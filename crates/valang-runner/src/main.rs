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

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8787);

    let app = Router::new()
        .route("/v1/run", post(run))
        .route("/v1/languages", post(languages))
        // The playground is served from another origin, and this service holds
        // nothing worth stealing: it is given a record and answers with a
        // decision, both of which the caller already had.
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await.expect("bind");
    eprintln!("valang-runner on :{port}");
    axum::serve(listener, app).await.expect("serve");
}

async fn languages() -> AxJson<Value> {
    AxJson(json!({
        "languages": lang::LANGS.iter().map(|l| l.ext).collect::<Vec<_>>(),
    }))
}

async fn run(AxJson(req): AxJson<RunRequest>) -> (StatusCode, AxJson<RunResponse>) {
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
