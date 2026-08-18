//! The runner: somewhere a publisher's handler can execute.
//!
//! A handler is given an execution record and answers with one decision. It may
//! be written in TypeScript, Python, Go or Rust, and the contract does not
//! change between them — the record is a signed JWT with a published `vct`.
//!
//! **Verification happens here, once, in Rust.** The SDK a handler is given
//! returns the result rather than recomputing it, so there is one verifier
//! rather than four that agree until the day they do not.

mod lang;
mod sandbox;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{ConnectInfo, State};
use axum::{extract::Json as AxJson, http::StatusCode, routing::post, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunRequest {
    /// The handler and the modules beside it, as the author wrote them.
    files: Vec<File>,
    /// The entry point: `handler.ts`, `.py`, `.go` or `.rs`.
    entry: String,
    /// The execution record, as a compact JWT.
    token: String,
    /// The `.val` package this record claims to have run, so its hash can be
    /// compared with the one the record carries.
    source: String,
    /// The device key the record claims, hex.
    device_key: String,
    /// The last root this publisher saw from this holder, hex, if any.
    last_root: Option<String>,
}

#[derive(Deserialize)]
struct File {
    name: String,
    source: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum RunResponse {
    Decision(Value),
    Failed { kind: &'static str, error: String },
}

/// How many handlers may be running at once, and how many of those one caller
/// may hold.
///
/// The playground is public and a Rust handler costs a compile, so without a
/// cap a loop in somebody's tab is a free CPU faucet — and a queue of builds
/// starves the Python handler that would have answered in 30ms.
#[derive(Clone)]
struct Limits {
    /// Total in flight. Sized to the machine rather than to demand: eight
    /// concurrent cargo builds on four shared cores finish slower than four do.
    slots: Arc<tokio::sync::Semaphore>,
    /// In flight per caller. One, because a person presses one button.
    per_caller: Arc<Mutex<HashMap<String, usize>>>,
    each: usize,
}

impl Limits {
    fn from_env() -> Self {
        let total = std::env::var("RUNNER_SLOTS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
        let each =
            std::env::var("RUNNER_SLOTS_PER_CALLER").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
        Limits {
            slots: Arc::new(tokio::sync::Semaphore::new(total)),
            per_caller: Arc::new(Mutex::new(HashMap::new())),
            each,
        }
    }

    /// A slot, or nothing. Never a wait: a caller told to come back knows what
    /// happened, where one left hanging behind four cargo builds reads as a
    /// service that is broken.
    fn take(&self, caller: &str) -> Option<Slot> {
        let permit = self.slots.clone().try_acquire_owned().ok()?;
        {
            let mut held = self.per_caller.lock().unwrap();
            let n = held.entry(caller.to_string()).or_insert(0);
            if *n >= self.each {
                return None;
            }
            *n += 1;
        }
        Some(Slot { limits: self.clone(), caller: caller.to_string(), _permit: permit })
    }
}

/// Returns the slot when the request ends, however it ends.
struct Slot {
    limits: Limits,
    caller: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut held = self.limits.per_caller.lock().unwrap();
        if let Some(n) = held.get_mut(&self.caller) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                held.remove(&self.caller);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8787);

    let app = Router::new()
        .route("/v1/run", post(run))
        .route("/v1/languages", post(languages))
        .with_state(Limits::from_env())
        // The playground is served from another origin, and this service holds
        // nothing worth stealing: it is given a record and answers with a
        // decision, both of which the caller already had.
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await.expect("bind");
    eprintln!("valang-runner on :{port}");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .expect("serve");
}

async fn languages() -> AxJson<Value> {
    AxJson(json!({
        "languages": lang::LANGS.iter().map(|l| l.ext).collect::<Vec<_>>(),
    }))
}

async fn run(
    State(limits): State<Limits>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    AxJson(req): AxJson<RunRequest>,
) -> (StatusCode, AxJson<RunResponse>) {
    let Some(_slot) = limits.take(&peer.ip().to_string()) else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            AxJson(RunResponse::Failed {
                kind: "busy",
                error: "the runner is at capacity — try again".into(),
            }),
        );
    };

    let Some(lang) = lang::lang_of(&req.entry) else {
        return (
            StatusCode::BAD_REQUEST,
            AxJson(RunResponse::Failed {
                kind: "unknownLanguage",
                error: format!("no runner for {}", req.entry),
            }),
        );
    };

    // Verify before anything is executed. A handler that never runs cannot be
    // the thing that decides whether the record was good.
    let checked = verify(&req);

    let payload = json!({ "token": req.token, "checked": checked }).to_string();

    match sandbox::execute(lang, &req.files, &payload).await {
        Ok(out) => match serde_json::from_str::<Value>(out.trim()) {
            Ok(decision) => (StatusCode::OK, AxJson(RunResponse::Decision(decision))),
            Err(_) => (
                StatusCode::OK,
                AxJson(RunResponse::Failed {
                    kind: "threw",
                    error: format!("the handler printed something that is not a decision: {out}"),
                }),
            ),
        },
        Err(e) => (StatusCode::OK, AxJson(RunResponse::Failed { kind: "threw", error: e })),
    }
}

/// The one verifier. Its answer is what every language's SDK hands back from
/// `verify`, so a handler in Go and a handler in Python cannot disagree about
/// whether a record was good.
fn verify(req: &RunRequest) -> Value {
    let device_key = hex_bytes(&req.device_key);
    let last_root = req.last_root.as_deref().map(hex_bytes);
    let code = valang_verify::code_hash(&req.source);
    let never = |_: &str| false;

    let expect = valang_verify::Expectation {
        code_hash: &code,
        device_key: &device_key,
        last_root: last_root.as_deref(),
        spent: &never,
    };

    match valang_verify::verify(&req.token, &expect) {
        Ok(v) => json!({
            "ok": true,
            "record": {
                "app": v.record.app,
                "version": v.record.version,
                "action": v.record.action,
                "codeHash": v.record.code_hash,
                "previousRoot": v.record.previous_root,
                "nextRoot": v.record.next_root,
                "outcome": v.record.outcome,
            },
            "effects": v.effects.iter().map(|e| json!({
                "capability": e.capability,
                "payload": effect_payload(&e.payload),
                "reversible": e.reversible,
            })).collect::<Vec<_>>(),
        }),
        Err(r) => json!({ "ok": false, "refusal": { "kind": kind_of(&r), "why": format!("{r:?}") } }),
    }
}

fn kind_of(r: &valang_verify::Refusal) -> &'static str {
    use valang_verify::Refusal::*;
    match r {
        Malformed(_) => "malformed",
        Unsigned(_) => "unsigned",
        UnknownCode { .. } => "unknownCode",
        DidNotCommit(_) => "didNotCommit",
        NoSuchEffect(_) => "noSuchEffect",
        AlreadySpent(_) => "alreadySpent",
        RolledBack { .. } => "rolledBack",
    }
}

/// An effect payload, as JSON a handler in any language can walk.
fn effect_payload(v: &valang_runtime::value::Value) -> Value {
    use valang_runtime::value::Value as V;
    match v {
        V::Null => Value::Null,
        V::Bool(b) => json!(b),
        V::Int(i) => json!(i),
        V::Str(s) => json!(s),
        V::Bytes(b) => json!(hex_of(b)),
        V::List(xs) => Value::Array(xs.iter().map(effect_payload).collect()),
        V::Map(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), effect_payload(x))).collect()),
        V::Enum(ty, case) => json!(format!("{ty}.{case}")),
        V::Credential { ty, claims, .. } => json!({
            "credential": ty,
            "claims": claims.iter().map(|(k, x)| (k.clone(), effect_payload(x))).collect::<serde_json::Map<_, _>>(),
        }),
    }
}

fn hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .filter_map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

fn hex_of(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
