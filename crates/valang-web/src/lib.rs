//! The compiler and the runtime, in a browser.
//!
//! The playground used to carry a parser and an evaluator written in
//! TypeScript. They were honest about being approximations and they were still
//! a second implementation of a language whose whole claim is that what ran can
//! be checked — two answers to "does this compile", and the one a reader saw was
//! not the one a host would run.
//!
//! **No `wasm-bindgen`.** Two exported functions and a length-prefixed string
//! is the whole interface; a binding generator here would be a build step and a
//! version to keep matched for something that fits on one page.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value as Json};
use valang::ast::{DataSource, Program};
use valang_runtime::fixture::Fixture;
use valang_runtime::render::{render as resolve_screen, Component};
use valang_runtime::merkle::hex;
use valang_runtime::value::Value;
use valang_runtime::{encode_record, run_action, Outcome};

// ------------------------------------------------------------------- memory

/// JavaScript asks for a buffer, writes into it, and hands back the pointer.
/// Anything it gets back is a length-prefixed UTF-8 string it must free.
#[no_mangle]
pub extern "C" fn val_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub extern "C" fn val_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe { drop(Vec::from_raw_parts(ptr, len, len)) }
    }
}

fn read(ptr: *const u8, len: usize) -> String {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(bytes).into_owned()
}

/// Four bytes of little-endian length, then the bytes. The caller reads the
/// prefix, then the string, then frees the whole thing.
fn write(s: String) -> *mut u8 {
    let bytes = s.into_bytes();
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&bytes);
    let ptr = out.as_mut_ptr();
    std::mem::forget(out);
    ptr
}

// ------------------------------------------------------------------ exports

/// Everything a reader can be shown about a source without running it:
/// diagnostics, the derived capability report, and the screens it declares.
#[no_mangle]
pub extern "C" fn val_analyse(ptr: *const u8, len: usize) -> *mut u8 {
    let input: Json = serde_json::from_str(&read(ptr, len)).unwrap_or(Json::Null);
    let source = input["source"].as_str().unwrap_or("");
    let bundle = text_bundle(&input["text"]);
    let locales: Vec<String> = input["locales"]
        .as_array()
        .map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    let hosts = hosts(&input["hosts"]);
    let packages = packages(&input["packages"]);
    let (program, diagnostics) = if bundle.is_empty() {
        valang::analyse_with_packages(source, None, &hosts, &packages)
    } else {
        valang::analyse_with_packages(source, Some((&bundle, &locales)), &hosts, &packages)
    };

    write(
        json!({
            "diagnostics": diagnostics.iter().map(|d| json!({
                "line": d.span.line,
                "column": d.span.col,
                "severity": match d.severity { valang::Severity::Error => "error", _ => "warning" },
                "message": d.message,
            })).collect::<Vec<_>>(),
            "report": report_json(&program),
            "screens": screens_json(&program),
            "actions": program.actions.iter().map(|a| a.name.clone()).collect::<Vec<_>>(),
        })
        .to_string(),
    )
}

/// One action, run against the wallet the caller supplies. What comes back is
/// the execution record and the trace behind it — which is what makes a press
/// in the preview something you can read afterwards rather than something that
/// happened.
#[no_mangle]
pub extern "C" fn val_run(ptr: *const u8, len: usize) -> *mut u8 {
    let input: Json = serde_json::from_str(&read(ptr, len)).unwrap_or(Json::Null);
    let source = input["source"].as_str().unwrap_or("");
    let action = input["action"].as_str().unwrap_or("");
    let wallet = input["wallet"].to_string();

    let (program, diagnostics) =
        valang::analyse_with_packages(source, None, &hosts(&input["hosts"]), &packages(&input["packages"]));
    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| format!("{}: {}", d.span.line, d.message))
        .collect();
    if !errors.is_empty() {
        return write(json!({ "wouldNotBuild": errors }).to_string());
    }

    let Ok(host) = Fixture::parse(&wallet) else {
        return write(json!({ "error": "the wallet is not valid JSON" }).to_string());
    };
    let before = valang_runtime::initial_state(&program, &host.state());
    // What the form held when it was submitted. The host collected it and
    // handed it over; nothing about it was ever application state.
    let submitted = json_state(&input["input"]);
    let run = run_action(&program, source, action, &before, &submitted, &host);
    let r = &run.record;

    write(
        json!({
            "action": r.action,
            "outcome": match &run.outcome {
                Outcome::Committed => json!({ "kind": "committed" }),
                Outcome::Refused(w) => json!({ "kind": "refused", "why": w }),
                Outcome::Failed(w) => json!({ "kind": "failed", "why": w }),
                Outcome::Defect(w) => json!({ "kind": "defect", "why": w }),
                Outcome::Declined(k) => json!({ "kind": "declined", "why": k }),
            },
            // The diff, which is the part somebody reads first — and the reason
            // an action is worth thinking of as a reducer.
            "changed": diff(&before, &run.next_state),
            "before": value_json(&Value::Map(before)),
            "after": value_json(&Value::Map(run.next_state.clone())),
            "effects": run.effects.iter().map(|e| json!({
                "capability": e.capability,
                "payload": value_json(&e.payload),
                "reversible": e.reversible,
            })).collect::<Vec<_>>(),
            // The token, which is all a publisher's server is handed.
            "token": valang_runtime::attestation::jwt(r),
            "deviceKey": hex_bytes(&r.device_key),
            "record": {
                "codeHash": hex(&r.code_hash),
                "inputHash": hex(&r.input_hash),
                "previousRoot": hex(&r.previous_root),
                "nextRoot": hex(&r.next_root),
                "executed": r.effects_executed,
                "time": r.context.time_now,
                "uuid": r.context.random_uuid,
                "bytes": encode_record(r).len(),
                "signature": hex_bytes(&r.signature),
            },
            "leaves": run.leaves.iter().map(|l| json!({
                "path": l.path,
                "value": value_json(&l.value),
                "hash": hex(&l.hash),
            })).collect::<Vec<_>>(),
        })
        .to_string(),
    )
}

/// One screen, resolved with what a press handed it.
///
/// A parameterised screen cannot be resolved ahead of time — its content depends
/// on the row that opened it — so a host asks for it when it moves.
#[no_mangle]
pub extern "C" fn val_screen(ptr: *const u8, len: usize) -> *mut u8 {
    let input: Json = serde_json::from_str(&read(ptr, len)).unwrap_or(Json::Null);
    let source = input["source"].as_str().unwrap_or("");
    let name = input["screen"].as_str().unwrap_or("");
    let wallet = input["wallet"].to_string();

    let (program, _) =
        valang::analyse_with_packages(source, None, &hosts(&input["hosts"]), &packages(&input["packages"]));
    let Ok(host) = Fixture::parse(&wallet) else {
        return write(json!({ "error": "the wallet is not valid JSON" }).to_string());
    };
    let state = valang_runtime::initial_state(&program, &host.state());
    let args = json_state(&input["args"]);

    match valang_runtime::render::render_with(&program, name, &state, &args, &host) {
        Ok(s) => write(screen_json(&s).to_string()),
        Err(e) => write(json!({ "error": format!("{e:?}") }).to_string()),
    }
}

/// Every screen, resolved against the wallet: the data the host answered with,
/// the values the derived block computed, and a tree whose slots are values
/// rather than the expressions that produced them.
///
/// The renderer used to resolve these itself, in Dart, which put `limit`,
/// `order by` and `verified with` in a second language — and then in whatever
/// the next renderer is written in. What is left over there is drawing and
/// formatting, which is what a toolkit is for.
#[no_mangle]
pub extern "C" fn val_render(ptr: *const u8, len: usize) -> *mut u8 {
    let input: Json = serde_json::from_str(&read(ptr, len)).unwrap_or(Json::Null);
    let source = input["source"].as_str().unwrap_or("");
    let wallet = input["wallet"].to_string();

    let (program, _) =
        valang::analyse_with_packages(source, None, &hosts(&input["hosts"]), &packages(&input["packages"]));
    let Ok(host) = Fixture::parse(&wallet) else {
        return write(json!({ "screens": [] }).to_string());
    };
    let state = valang_runtime::initial_state(&program, &host.state());

    let screens: Vec<Json> = program
        .screens
        .iter()
        .map(|s| match resolve_screen(&program, &s.name, &state, &host) {
            Ok(resolved) => screen_json(&resolved),
            // Said rather than dropped. A screen that threw used to vanish, and
            // the next one took its place — which looks exactly like a screen
            // somebody forgot to write.
            Err(e) => json!({
                "name": s.name,
                "start": s.is_main(),
                "error": format!("{e:?}"),
                "data": Vec::<Json>::new(),
                "derived": Json::Object(Map::new()),
                "tree": Vec::<Json>::new(),
            }),
        })
        .collect();

    write(json!({ "screens": screens }).to_string())
}

/// A resolved screen, as the renderer receives it. One shape, whether a host
/// asked for every screen or for one it is moving to.
fn screen_json(s: &valang_runtime::render::Screen) -> Json {
    json!({
        "name": s.name,
        "title": s.title.as_ref().map(component_json),
        "start": s.start,
        "data": s.data.iter().map(|d| json!({
            "name": d.name,
            "grade": d.grade,
            "of": d.of,
            "policy": d.policy,
            "rows": d.rows,
        })).collect::<Vec<_>>(),
        "derived": Json::Object(s.derived.iter().map(|(k, v)| (k.clone(), value_json(v))).collect::<Map<_, _>>()),
        "tree": s.tree.iter().map(component_json).collect::<Vec<_>>(),
    })
}

fn component_json(c: &Component) -> Json {
    json!({
        "kind": c.kind,
        "args": Json::Object(c.args.iter().map(|(k, v)| (k.clone(), value_json(v))).collect::<Map<_, _>>()),
        "children": c.children.iter().map(component_json).collect::<Vec<_>>(),
    })
}

/// The check a publisher's server runs, in the browser — the same crate a Go or
/// a Python SDK will bind to, so the editor is not demonstrating a second
/// implementation of the thing it is teaching.
#[no_mangle]
pub extern "C" fn val_verify(ptr: *const u8, len: usize) -> *mut u8 {
    let input: Json = serde_json::from_str(&read(ptr, len)).unwrap_or(Json::Null);
    let token = input["token"].as_str().unwrap_or("");
    let source = input["source"].as_str().unwrap_or("");

    let device_key = hex_bytes_of(input["deviceKey"].as_str().unwrap_or(""));
    let last_root = input["lastRoot"].as_str().map(hex_bytes_of);
    let code = valang_verify::code_hash(source);
    let never = |_: &str| false;

    let expect = valang_verify::Expectation {
        code_hash: &code,
        device_key: &device_key,
        last_root: last_root.as_deref(),
        spent: &never,
    };

    match valang_verify::verify(token, &expect) {
        Ok(v) => write(
            json!({
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
                    "payload": value_json(&e.payload),
                    "reversible": e.reversible,
                })).collect::<Vec<_>>(),
            })
            .to_string(),
        ),
        Err(r) => write(json!({ "ok": false, "refusal": describe(&r) }).to_string()),
    }
}

fn describe(r: &valang_verify::Refusal) -> Json {
    use valang_verify::Refusal::*;
    match r {
        Malformed(w) => json!({ "kind": "malformed", "why": w }),
        Unsigned(w) => json!({ "kind": "unsigned", "why": w }),
        UnknownCode { expected, found } => json!({
            "kind": "unknownCode",
            "why": format!("this record is from code {}, and this publisher published {}", &found[..8.min(found.len())], &expected[..8.min(expected.len())]),
        }),
        DidNotCommit(w) => json!({ "kind": "didNotCommit", "why": format!("the run {w}, so nothing was earned") }),
        NoSuchEffect(w) => json!({ "kind": "noSuchEffect", "why": w }),
        AlreadySpent(w) => json!({ "kind": "alreadySpent", "why": w }),
        RolledBack { seen, offered } => json!({
            "kind": "rolledBack",
            "why": format!("this reaches back to {}, and {} was already seen", &offered[..8.min(offered.len())], &seen[..8.min(seen.len())]),
        }),
    }
}

fn hex_bytes_of(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .filter_map(|i| u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect()
}

// -------------------------------------------------------------------- shapes

fn hex_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A JSON object as the values an action is given.
fn json_state(j: &Json) -> BTreeMap<String, valang_runtime::value::Value> {
    let mut out = BTreeMap::new();
    if let Some(map) = j.as_object() {
        for (k, v) in map {
            let value = match v {
                Json::String(s) => valang_runtime::value::Value::Str(s.clone()),
                Json::Bool(b) => valang_runtime::value::Value::Bool(*b),
                Json::Number(n) => n
                    .as_i64()
                    .map(valang_runtime::value::Value::Int)
                    .unwrap_or(valang_runtime::value::Value::Null),
                _ => valang_runtime::value::Value::Null,
            };
            out.insert(k.clone(), value);
        }
    }
    out
}

/// What the caller says it provides. A host hands this in; the language carries
/// no list of what anybody can do or draw.
/// The other packages this build can reach — every other project the editor has
/// open, so importing across two of them is the same thing here as importing a
/// published package.
///
/// Parsed rather than checked: whether that package builds is its own build's
/// answer, and its unrelated mistakes have no business appearing in this one's
/// panel. What is taken from it is checked on the way in.
fn packages(j: &Json) -> valang::expand::Packages {
    let mut loaded = Vec::new();
    if let Some(list) = j.as_array() {
        for entry in list {
            if let Some(source) = entry.as_str() {
                loaded.push(valang::parse::parse(source).0);
            }
        }
    }
    valang::expand::Packages::of(loaded)
}

fn hosts(j: &Json) -> valang::capability::Hosts {
    let mut loaded = Vec::new();
    if let Some(list) = j.as_array() {
        for entry in list {
            let source = match entry {
                Json::String(s) => s.clone(),
                other => other.to_string(),
            };
            if let Ok(h) = valang::capability::Host::parse(&source) {
                loaded.push(h);
            }
        }
    }
    valang::capability::Hosts::of(loaded)
}

fn text_bundle(j: &Json) -> valang::TextBundle {
    let mut out = valang::TextBundle::new();
    if let Some(map) = j.as_object() {
        for (key, per_locale) in map {
            let mut inner = BTreeMap::new();
            if let Some(l) = per_locale.as_object() {
                for (locale, template) in l {
                    if let Some(t) = template.as_str() {
                        inner.insert(locale.clone(), t.to_string());
                    }
                }
            }
            out.insert(key.clone(), inner);
        }
    }
    out
}

fn value_json(v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::Bool(b) => json!(b),
        Value::Int(i) => json!(i),
        Value::Str(s) => json!(s),
        Value::Bytes(b) => json!(hex_bytes(b)),
        Value::List(items) => Json::Array(items.iter().map(value_json).collect()),
        Value::Map(m) => Json::Object(m.iter().map(|(k, v)| (k.clone(), value_json(v))).collect()),
        Value::Enum(e, member) => json!(format!("{e}.{member}")),
        Value::Credential { ty, claims, verified } => json!({
            "credential": ty,
            "verified": verified,
            "claims": Json::Object(claims.iter().map(|(k, v)| (k.clone(), value_json(v))).collect::<Map<_, _>>()),
        }),
    }
}

/// Field by field, by path, so the panel can show what moved rather than two
/// blobs to compare by eye.
fn diff(before: &BTreeMap<String, Value>, after: &BTreeMap<String, Value>) -> Json {
    fn walk(prefix: &str, a: &Value, b: &Value, out: &mut Vec<Json>) {
        match (a, b) {
            (Value::Map(x), Value::Map(y)) => {
                let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
                keys.sort();
                keys.dedup();
                for k in keys {
                    let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                    walk(
                        &path,
                        x.get(k).unwrap_or(&Value::Null),
                        y.get(k).unwrap_or(&Value::Null),
                        out,
                    );
                }
            }
            _ if a != b => out.push(json!({ "path": prefix, "from": value_json(a), "to": value_json(b) })),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk("", &Value::Map(before.clone()), &Value::Map(after.clone()), &mut out);
    Json::Array(out)
}

fn report_json(p: &Program) -> Json {
    let r = valang::report::report(p);
    let list = |s: &std::collections::BTreeSet<String>| Json::Array(s.iter().map(|x| json!(x)).collect());
    json!({
        "app": r.app,
        "version": r.version,
        "reads": list(&r.reads),
        "discloses": list(&r.discloses),
        "proves": list(&r.proves),
        "issues": list(&r.issues),
        "audiences": list(&r.audiences),
        "payments": list(&r.payments),
        "writes": list(&r.writes),
        "irreversible": r.irreversible,
    })
}

fn screens_json(p: &Program) -> Json {
    Json::Array(
        p.screens
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "data": s.data.iter().map(|d| match &d.source {
                        DataSource::Credentials { ty, policy, .. } => json!({
                            "name": d.name, "source": "credentials", "type": ty, "policy": policy,
                        }),
                        DataSource::Query { audience } => json!({
                            "name": d.name, "source": "query", "audience": audience,
                        }),
                        DataSource::Unknown => json!({ "name": d.name, "source": "unknown" }),
                    }).collect::<Vec<_>>(),
                    "tree": s.tree.iter().map(ui_json).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

fn ui_json(n: &valang::ast::UiNode) -> Json {
    let mut args = Map::new();
    for (i, a) in n.args.iter().enumerate() {
        let key = a.name.clone().unwrap_or_else(|| i.to_string());
        // An expression is handed over as the text somebody wrote: the host
        // resolves it against the wallet, because the wallet is the host's.
        args.insert(key, json!(render(&a.value)));
    }
    json!({
        "kind": n.kind,
        "args": Json::Object(args),
        "lambda": n.lambda,
        "children": n.children.iter().map(ui_json).collect::<Vec<_>>(),
        // The other half of an `if`. A screen is listed here before it is
        // resolved, so both branches are still in it.
        "otherwise": n.otherwise.iter().map(ui_json).collect::<Vec<_>>(),
    })
}

fn render(e: &valang::ast::Expr) -> String {
    use valang::ast::Expr::*;
    match e {
        Num { value, .. } => value.to_string(),
        Str { value, .. } => value.clone(),
        Bool { value, .. } => value.to_string(),
        Binary { op, lhs, rhs, .. } => format!("{} {op} {}", render(lhs), render(rhs)),
        Call { callee, args, .. } => format!(
            "{}({})",
            render(callee),
            args.iter().map(|a| render(&a.value)).collect::<Vec<_>>().join(", ")
        ),
        other => other.path().unwrap_or_else(|| "…".into()),
    }
}
