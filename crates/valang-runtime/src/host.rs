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

/// What the host will carry. Totality bounds the number of steps a program
/// takes and says nothing about how large a value may become — a `fold` whose
/// accumulator grows is finite in steps and unbounded in memory. So the bounds
/// live here, where they can be honest, rather than being implied by a word
/// that does not cover them (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_list: usize,
    pub max_string_bytes: usize,
    /// The whole of a state, canonically encoded.
    pub max_state_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        // Numbers a phone can hold without thinking about it. A host with a
        // reason picks its own; a host with no reason should not have to.
        Limits { max_list: 4_096, max_string_bytes: 64 * 1024, max_state_bytes: 1024 * 1024 }
    }
}

pub trait Host {
    fn context(&self) -> Context;

    fn limits(&self) -> Limits {
        Limits::default()
    }

    /// The credential the person chose, already checked against the policy the
    /// program named. The runtime does not verify signatures — it could not,
    /// and a language that pretended to would be the wrong place for it.
    fn credential(&self, ty: &str, policy: Option<&str>) -> Option<BTreeMap<String, Value>>;

    /// Asked once, with the whole batch, after the pure phases have run.
    fn decide(&self, effects: &[EffectRequest]) -> Verdict;

    /// The credentials a screen declared, already checked against the policy it
    /// named. A screen declares its data and the host resolves it *before*
    /// drawing — which is why there is no half-drawn screen and no prompt
    /// arriving mid-scroll.
    ///
    /// `order` is the claim to sort on and whether it descends, as the screen
    /// wrote it. The host does the sorting because the host holds the rows —
    /// and until this was passed, a screen asking for its receipts newest first
    /// got whatever order the wallet happened to answer in.
    fn credentials_of(
        &self,
        _ty: &str,
        _policy: Option<&str>,
        _order: Option<(&str, bool)>,
        _limit: Option<i64>,
    ) -> Vec<BTreeMap<String, Value>> {
        Vec::new()
    }

    /// A query answer. The host performed the presentation and holds the token;
    /// the application never sees it, and never learns why a query failed.
    fn query(&self, _audience: &str, _operation: &str) -> Vec<Value> {
        Vec::new()
    }

    /// Sign the execution record. The key stays here: the evaluator has no
    /// business holding one, and a runtime that could sign could sign a record
    /// of a run that did not happen.
    fn sign(&self, bytes: &[u8]) -> Vec<u8>;

    /// The public half, so a verifier can check the signature without asking
    /// the device for anything.
    fn device_key(&self) -> Vec<u8>;
}
