//! What the runtime asks of a host.
//!
//! Small on purpose. Everything here is something the language cannot do and
//! must not fake: the clock, randomness, the credentials somebody holds, and
//! the decision about a batch of effects.

use std::collections::BTreeMap;

use crate::value::Value;

/// Requested, never performed. The runtime builds these and stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectRequest {
    pub capability: String,
    pub operation: String,
    pub payload: Value,
    /// Whether the host can walk this back if a later effect fails. A
    /// disclosure cannot, which is why the compiler orders them last.
    pub reversible: bool,
}

/// Recorded with everything else, because an action that cannot be replayed
/// cannot be proved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub time_now: i64,
    pub random_uuid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The whole batch was taken. The next state commits.
    Approved,
    /// The whole batch was refused. Nothing commits, and this is not an error.
    Refused(String),
}

pub trait Host {
    fn context(&self) -> Context;

    /// The credential the person chose, already checked against the policy the
    /// program named. The runtime does not verify signatures — it could not,
    /// and a language that pretended to would be the wrong place for it.
    fn credential(&self, ty: &str, policy: Option<&str>) -> Option<BTreeMap<String, Value>>;

    /// Asked once, with the whole batch, after the pure phases have run.
    fn decide(&self, effects: &[EffectRequest]) -> Verdict;
}
