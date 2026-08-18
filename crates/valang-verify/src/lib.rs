//! Verifying an execution record.
//!
//! **One implementation.** A publisher's server decides whether to sign a
//! credential, and that decision rests entirely on this check — so if every
//! language SDK reimplemented it, the SDKs would disagree, and the disagreement
//! would be about whether somebody's points were real.
//!
//! So this crate compiles to Wasm and the SDKs are bindings over it. A Go
//! publisher and a Python one run the same bytes, and a bug found by either is
//! fixed for both.
//!
//! What this does **not** do is decide. It answers what is true about a record;
//! whether that is grounds to issue something is the publisher's business, and
//! theirs alone — it is their key.

use std::collections::BTreeMap;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use valang_runtime::canonical::{Canonical, DeterministicCbor};
use valang_runtime::decode::decode;
use valang_runtime::value::Value;

/// A record, as it arrived. Decoded rather than trusted: every field below is
/// read out of the bytes the device signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub app: String,
    pub version: String,
    pub action: String,
    pub code_hash: Vec<u8>,
    pub input_hash: Vec<u8>,
    pub previous_root: Vec<u8>,
    pub next_root: Vec<u8>,
    pub policies: Vec<String>,
    pub capabilities: Vec<String>,
    pub effects: Vec<Effect>,
    pub executed: i64,
    pub time: i64,
    pub uuid: String,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    pub capability: String,
    pub payload: Value,
    pub reversible: bool,
}

/// What a server is asked to believe, and what it should refuse to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Malformed(String),
    /// The device's signature is not over these bytes.
    Unsigned(String),
    /// The code that ran is not the code this publisher published.
    UnknownCode { expected: String, found: String },
    /// The run did not commit, so there is nothing to have earned.
    DidNotCommit(String),
    /// The action asked for something this record does not show it doing.
    NoSuchEffect(String),
    /// The same credential, twice.
    AlreadySpent(String),
    /// The state went backwards: this record reaches behind one already seen.
    RolledBack { seen: String, offered: String },
}

/// What the caller knows that the record cannot tell it.
pub struct Expectation<'a> {
    /// The code hash of the version this publisher published. A record from any
    /// other code is a record from a program nobody reviewed.
    pub code_hash: &'a [u8],
    /// The device key the record claims. A server that takes this from the
    /// record itself has checked that the record signed itself.
    pub device_key: &'a [u8],
    /// The last `next_root` this server saw from this holder, if any. Rolling
    /// back and replaying is the double-spend this system actually has, and
    /// remembering one hash is the whole defence.
    pub last_root: Option<&'a [u8]>,
    /// Nullifiers already spent. Not credential identifiers — a list of those
    /// links every presentation to the one before it.
    pub spent: &'a dyn Fn(&str) -> bool,
}

#[derive(Debug)]
pub struct Verified {
    pub record: Record,
    /// The effects the record shows being requested. A server signs against
    /// these, never against what the client asked it to sign.
    pub effects: Vec<Effect>,
}

/// Decode, check, and hand back what is true. The order matters: nothing is read
/// out of a record before the signature over it has been checked.
pub fn verify(bytes: &[u8], signature: &[u8], expect: &Expectation) -> Result<Verified, Refusal> {
    // 1. The signature, first, over the bytes as they arrived.
    let key: [u8; 32] = expect
        .device_key
        .try_into()
        .map_err(|_| Refusal::Unsigned("the device key is not a key".into()))?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| Refusal::Unsigned("malformed device key".into()))?;
    let sig = Signature::from_slice(signature).map_err(|_| Refusal::Unsigned("malformed signature".into()))?;
    key.verify(bytes, &sig)
        .map_err(|_| Refusal::Unsigned("the signature is not over these bytes".into()))?;

    // 2. Only now is it worth reading.
    let record = parse(bytes)?;

    // 3. The code that ran is the code this publisher published.
    if record.code_hash != expect.code_hash {
        return Err(Refusal::UnknownCode {
            expected: hex(expect.code_hash),
            found: hex(&record.code_hash),
        });
    }

    // 4. It committed. A refused or failed run earned nothing.
    if record.outcome != "committed" {
        return Err(Refusal::DidNotCommit(record.outcome.clone()));
    }

    // 5. The state did not go backwards.
    if let Some(seen) = expect.last_root {
        if record.previous_root != seen && record.next_root != seen {
            return Err(Refusal::RolledBack { seen: hex(seen), offered: hex(&record.previous_root) });
        }
    }

    let effects = record.effects.clone();
    Ok(Verified { record, effects })
}

/// The one thing a caller has to do itself, because only it knows what its own
/// scheme's nullifier is derived from.
pub fn check_spent(nullifier: &str, expect: &Expectation) -> Result<(), Refusal> {
    if (expect.spent)(nullifier) {
        return Err(Refusal::AlreadySpent(nullifier.to_string()));
    }
    Ok(())
}

/// The claims of an issuance the record shows being requested — which is what a
/// server signs over. Signing what the client asked for instead would make every
/// check above decorative.
pub fn issuance<'a>(v: &'a Verified, credential: &str) -> Result<&'a BTreeMap<String, Value>, Refusal> {
    for e in &v.effects {
        if e.capability != "credential.issue" {
            continue;
        }
        if let Value::Credential { ty, claims, .. } = &e.payload {
            if ty == credential {
                return Ok(claims);
            }
        }
    }
    Err(Refusal::NoSuchEffect(format!("this record does not issue a `{credential}`")))
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The hash of a package's sources, for a server that holds the package rather
/// than a hash somebody told it.
pub fn code_hash(source: &str) -> Vec<u8> {
    Sha256::digest(source.as_bytes()).to_vec()
}

fn parse(bytes: &[u8]) -> Result<Record, Refusal> {
    let Value::Map(m) = decode(bytes).map_err(|e| Refusal::Malformed(format!("{e:?}")))? else {
        return Err(Refusal::Malformed("a record is a map".into()));
    };
    let s = |k: &str| match m.get(k) {
        Some(Value::Str(v)) => v.clone(),
        _ => String::new(),
    };
    let b = |k: &str| match m.get(k) {
        Some(Value::Bytes(v)) => v.clone(),
        _ => Vec::new(),
    };
    let i = |k: &str| match m.get(k) {
        Some(Value::Int(v)) => *v,
        _ => 0,
    };
    let list = |k: &str| match m.get(k) {
        Some(Value::List(v)) => v
            .iter()
            .filter_map(|x| match x {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };

    let effects = match m.get("effects") {
        Some(Value::List(items)) => items
            .iter()
            .filter_map(|e| {
                let Value::Map(e) = e else { return None };
                Some(Effect {
                    capability: match e.get("capability") {
                        Some(Value::Str(c)) => c.clone(),
                        _ => String::new(),
                    },
                    payload: e.get("payload").cloned().unwrap_or(Value::Null),
                    reversible: matches!(e.get("reversible"), Some(Value::Bool(true))),
                })
            })
            .collect(),
        _ => Vec::new(),
    };

    Ok(Record {
        app: s("app"),
        version: s("version"),
        action: s("action"),
        code_hash: b("code"),
        input_hash: b("input"),
        previous_root: b("previous_root"),
        next_root: b("next_root"),
        policies: list("policies"),
        capabilities: list("capabilities"),
        effects,
        executed: i("executed"),
        time: i("time"),
        uuid: s("uuid"),
        outcome: s("outcome"),
    })
}

/// Re-encoding, for a caller that wants to check the bytes it was handed are the
/// canonical encoding of what it parsed. The decoder is strict, so this can only
/// differ if something upstream is not using this encoding at all.
pub fn reencode(r: &Record) -> Vec<u8> {
    let mut m = BTreeMap::new();
    m.insert("action".to_string(), Value::Str(r.action.clone()));
    m.insert("app".to_string(), Value::Str(r.app.clone()));
    DeterministicCbor.encode(&Value::Map(m))
}
