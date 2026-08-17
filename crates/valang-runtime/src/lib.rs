//! The VAL back end.
//!
//! Walks a typed AST and returns `(new state, output, effects)`. It never
//! performs an effect: it describes one and hands it to the host, which is the
//! only reason an execution record can be trusted.

pub mod canonical;
pub mod eval;
pub mod host;
pub mod merkle;
pub mod value;

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use valang::ast::{Phase, Program, Stmt};

use canonical::{Canonical, DeterministicCbor};
use eval::{Eval, Trap};
use host::{Context, EffectRequest, Host, Verdict};
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
    };

    let Some(action) = action else {
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
            Run { outcome: Outcome::Refused(why), next_state: state.clone(), effects, record, leaves: previous }
        }
        Verdict::Approved => {
            let leaves = merkle::leaves(&next, &enc);
            record.next_root = merkle::root(&leaves);
            record.effects_executed = effects.len();
            record.outcome = Outcome::Committed;
            Run { outcome: Outcome::Committed, next_state: next, effects, record, leaves }
        }
    }
}
