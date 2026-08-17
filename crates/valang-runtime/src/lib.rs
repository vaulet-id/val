//! The VAL back end.
//!
//! Walks a typed AST and returns `(new state, output, effects)`. It never
//! performs an effect: it describes one and hands it to the host, which is the
//! only reason an execution record can be trusted.

pub mod canonical;
pub mod decode;
pub mod eval;
pub mod host;
pub mod merkle;
pub mod render;
pub mod value;

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use valang::ast::{Phase, Program, Stmt};

use canonical::{Canonical, DeterministicCbor};
use eval::{Eval, Trap};
use host::{Context, EffectRequest, Host, Limits, Verdict};
use merkle::{Hash, Leaf};
use value::Value;

pub type State = BTreeMap<String, Value>;

/// What the record says happened. Four outcomes, and the difference between the
/// middle two is the difference between a bug report and a sentence somebody
/// reads (§5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The host took the batch; the next state is committed.
    Committed,
    /// The host refused the batch; nothing commits, and nothing went wrong.
    Refused(String),
    /// `verify` failed, or a trust rule did not hold. An ordinary outcome.
    Failed(String),
    /// `require` failed, or arithmetic trapped. A defect in the application.
    Defect(String),
}

#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub app: String,
    pub version: String,
    pub action: String,
    pub code_hash: Hash,
    pub input_hash: Hash,
    pub previous_root: Hash,
    pub next_root: Hash,
    pub policies: Vec<String>,
    pub capabilities: Vec<String>,
    pub effects_requested: Vec<EffectRequest>,
    pub effects_executed: usize,
    pub context: Context,
    pub outcome: Outcome,
    /// Over `encode_record` of everything above. Absent only when a run ended
    /// before there was anything to attest to.
    pub signature: Vec<u8>,
    pub device_key: Vec<u8>,
}

/// The record's bytes, canonically. This is what the device signs and what a
/// verifier re-encodes and checks — so it carries the outcome too: "refused" is
/// part of what happened, and a record that only attested to successes would be
/// evidence of a different thing than it claims.
fn within(state: &State, enc: &DeterministicCbor, limits: Limits) -> Result<(), String> {
    fn walk(v: &Value, limits: Limits, path: &str) -> Result<(), String> {
        match v {
            Value::List(items) if items.len() > limits.max_list => Err(format!(
                "`{path}` would hold {} items and this host carries at most {}. Totality bounds how many steps a program takes and says nothing about how large a value becomes",
                items.len(),
                limits.max_list
            )),
            Value::Str(s) if s.len() > limits.max_string_bytes => Err(format!(
                "`{path}` would hold {} bytes of text and this host carries at most {}",
                s.len(),
                limits.max_string_bytes
            )),
            Value::List(items) => items.iter().try_for_each(|i| walk(i, limits, path)),
            Value::Map(m) => m.iter().try_for_each(|(k, v)| walk(v, limits, &format!("{path}.{k}"))),
            Value::Credential { claims, .. } => {
                claims.iter().try_for_each(|(k, v)| walk(v, limits, &format!("{path}.{k}")))
            }
            _ => Ok(()),
        }
    }

    for (k, v) in state {
        walk(v, limits, k)?;
    }
    let bytes = enc.encode(&Value::Map(state.clone()));
    if bytes.len() > limits.max_state_bytes {
        return Err(format!(
            "this state would be {} bytes and this host carries at most {}",
            bytes.len(),
            limits.max_state_bytes
        ));
    }
    Ok(())
}

pub fn encode_record(r: &ExecutionRecord) -> Vec<u8> {
    let mut m = BTreeMap::new();
    m.insert("app".into(), Value::Str(r.app.clone()));
    m.insert("version".into(), Value::Str(r.version.clone()));
    m.insert("action".into(), Value::Str(r.action.clone()));
    m.insert("code".into(), Value::Bytes(r.code_hash.to_vec()));
    m.insert("input".into(), Value::Bytes(r.input_hash.to_vec()));
    m.insert("previous_root".into(), Value::Bytes(r.previous_root.to_vec()));
    m.insert("next_root".into(), Value::Bytes(r.next_root.to_vec()));
    m.insert("policies".into(), Value::List(r.policies.iter().cloned().map(Value::Str).collect()));
    m.insert("capabilities".into(), Value::List(r.capabilities.iter().cloned().map(Value::Str).collect()));
    m.insert(
        "effects".into(),
        Value::List(
            r.effects_requested
                .iter()
                .map(|e| {
                    let mut em = BTreeMap::new();
                    em.insert("capability".to_string(), Value::Str(e.capability.clone()));
                    em.insert("payload".to_string(), e.payload.clone());
                    em.insert("reversible".to_string(), Value::Bool(e.reversible));
                    Value::Map(em)
                })
                .collect(),
        ),
    );
    m.insert("executed".into(), Value::Int(r.effects_executed as i64));
    m.insert("time".into(), Value::Int(r.context.time_now));
    m.insert("uuid".into(), Value::Str(r.context.random_uuid.clone()));
    m.insert(
        "outcome".into(),
        Value::Str(match &r.outcome {
            Outcome::Committed => "committed".into(),
            Outcome::Refused(w) => format!("refused: {w}"),
            Outcome::Failed(w) => format!("failed: {w}"),
            Outcome::Defect(w) => format!("defect: {w}"),
        }),
    );
    DeterministicCbor.encode(&Value::Map(m))
}

pub struct Run {
    pub outcome: Outcome,
    pub next_state: State,
    pub effects: Vec<EffectRequest>,
    pub record: ExecutionRecord,
    pub leaves: Vec<Leaf>,
}

/// Run one action. `source` is hashed as the code identity, because the artifact
/// is the source (§1) and hashing anything else would be hashing a thing nobody
/// read.
pub fn run_action(
    program: &Program,
    source: &str,
    action_name: &str,
    state: &State,
    input: &State,
    host: &dyn Host,
) -> Run {
    let enc = DeterministicCbor;
    let context = host.context();

    let action = program.actions.iter().find(|a| a.name == action_name);
    let previous = merkle::leaves(state, &enc);
    let previous_root = merkle::root(&previous);

    let mut record = ExecutionRecord {
        app: program.app.clone().unwrap_or_default(),
        version: program.version.clone().unwrap_or_default(),
        action: action_name.to_string(),
        code_hash: Sha256::digest(source.as_bytes()).into(),
        input_hash: Sha256::digest(enc.encode(&Value::Map(input.clone()))).into(),
        previous_root,
        next_root: previous_root,
        policies: Vec::new(),
        capabilities: program.capabilities.iter().map(|c| c.name.clone()).collect(),
        effects_requested: Vec::new(),
        effects_executed: 0,
        context: context.clone(),
        outcome: Outcome::Defect("no such action".into()),
        signature: Vec::new(),
        device_key: host.device_key(),
    };

    let Some(action) = action else {
        record.signature = host.sign(&encode_record(&record));
        return Run { outcome: record.outcome.clone(), next_state: state.clone(), effects: Vec::new(), record, leaves: previous };
    };

    let mut ev = Eval::new(program, context);
    for (k, v) in input {
        ev.bind(k, v.clone());
    }

    // The host hands over the credentials the program's `input` names, already
    // checked against the policy that will be named in `verify`.
    for block in &action.phases {
        if block.phase != Phase::Input {
            continue;
        }
        for s in &block.stmts {
            if let Stmt::Binding { name, ty, .. } = s {
                if ty.name == "Credential" {
                    let cred_ty = ty.args.first().map(|a| a.name.clone()).unwrap_or_default();
                    let policy = program.trusts.iter().find(|t| t.subject_type == cred_ty).map(|t| t.name.clone());
                    if let Some(claims) = host.credential(&cred_ty, policy.as_deref()) {
                        ev.bind(name, Value::Credential { ty: cred_ty, claims, verified: None });
                    }
                }
            }
        }
    }

    let mut next = state.clone();
    for block in &action.phases {
        // `update` produced the next state and `execute` reads it as `next`.
        // Without this binding every `next.…` in an issued claim is null, and
        // the credential goes out empty — which a test that only checks which
        // capability was requested will not notice.
        if block.phase == Phase::Execute {
            ev.bind("next", Value::Map(next.clone()));
        }
        for s in &block.stmts {
            if let Err(trap) = ev.stmt(s, block.phase, &mut next) {
                record.outcome = match trap {
                    Trap::Failed(m) => Outcome::Failed(m),
                    Trap::Defect(m) => Outcome::Defect(m),
                    Trap::DivideByZero => Outcome::Defect("division by zero traps, as overflow does".into()),
                    Trap::Overflow(what) => Outcome::Defect(format!("integer overflow in {what} traps: a wrong number the record would then faithfully prove is worse than a failure")),
                    Trap::Unsupported(m) => Outcome::Defect(m),
                };
                record.effects_requested = ev.effects.clone();
                record.signature = host.sign(&encode_record(&record));
                return Run { outcome: record.outcome.clone(), next_state: state.clone(), effects: ev.effects, record, leaves: previous };
            }
        }
    }

    // Irreversible last, ordered here so the author does not have to know.
    let mut effects = ev.effects.clone();
    effects.sort_by_key(|e| !e.reversible);

    record.policies = program.trusts.iter().map(|t| t.name.clone()).collect();
    record.effects_requested = effects.clone();

    match host.decide(&effects) {
        Verdict::Refused(why) => {
            record.outcome = Outcome::Refused(why.clone());
            record.signature = host.sign(&encode_record(&record));
            Run { outcome: Outcome::Refused(why), next_state: state.clone(), effects, record, leaves: previous }
        }
        Verdict::Approved => {
            // Checked before the state is committed rather than while it is
            // being built: a limit that stops an action halfway leaves a state
            // that no phase produced.
            if let Err(why) = within(&next, &enc, host.limits()) {
                record.outcome = Outcome::Defect(why.clone());
                record.signature = host.sign(&encode_record(&record));
                return Run { outcome: Outcome::Defect(why), next_state: state.clone(), effects, record, leaves: previous };
            }
            let leaves = merkle::leaves(&next, &enc);
            record.next_root = merkle::root(&leaves);
            record.effects_executed = effects.len();
            record.outcome = Outcome::Committed;
            record.signature = host.sign(&encode_record(&record));
            Run { outcome: Outcome::Committed, next_state: next, effects, record, leaves }
        }
    }
}
