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
use valang_runtime::attestation::{b64_decode, VCT};
use valang_runtime::value::Value;

/// A record, as it arrived. Decoded rather than trusted: every field below is
/// read out of the bytes the device signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub vct: String,
    pub app: String,
    pub version: String,
    pub action: String,
    /// Hex, as the token carries them. A verifier compares strings rather than
    /// re-encoding, because the token is the thing that was signed.
    pub code_hash: String,
    pub input_hash: String,
    pub previous_root: String,
    pub next_root: String,
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

/// Verify a record token and hand back what is true.
///
/// The token is a JWT and the first half of this is ordinary JWS verification —
/// which is the point of having made it one. What is left is the part no
/// standard covers, because nobody has needed it before: whether the code that
/// ran is the code somebody published, and whether the state went backwards.
pub fn verify(token: &str, expect: &Expectation) -> Result<Verified, Refusal> {
    let mut parts = token.split('.');
    let (h, p, sig) = match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s), None) => (h, p, s),
        _ => return Err(Refusal::Malformed("a record is a three-part JWT".into())),
    };

    // 1. The signature over the signing input, before anything is read out of
    //    the payload. A verifier that parsed first would be deciding about a
    //    record it had not authenticated.
    let key: [u8; 32] = expect
        .device_key
        .try_into()
        .map_err(|_| Refusal::Unsigned("the device key is not a key".into()))?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| Refusal::Unsigned("malformed device key".into()))?;
    let raw = b64_decode(sig).ok_or_else(|| Refusal::Unsigned("the signature is not base64url".into()))?;
    let signature =
        Signature::from_slice(&raw).map_err(|_| Refusal::Unsigned("malformed signature".into()))?;
    key.verify(format!("{h}.{p}").as_bytes(), &signature)
        .map_err(|_| Refusal::Unsigned("the signature is not over these bytes".into()))?;

    // 2. Now it is worth reading.
    let claims = b64_decode(p).ok_or_else(|| Refusal::Malformed("the payload is not base64url".into()))?;
    let claims = String::from_utf8(claims).map_err(|_| Refusal::Malformed("the payload is not UTF-8".into()))?;
    let record = parse(&claims)?;

    if record.vct != VCT {
        return Err(Refusal::Malformed(format!("this is a `{}`, not an execution record", record.vct)));
    }

    // 3. The code that ran is the code this publisher published.
    if record.code_hash != hex(expect.code_hash) {
        return Err(Refusal::UnknownCode { expected: hex(expect.code_hash), found: record.code_hash });
    }

    // 4. It committed. A refused or failed run earned nothing.
    if record.outcome != "committed" {
        return Err(Refusal::DidNotCommit(record.outcome.clone()));
    }

    // 5. The state did not go backwards.
    if let Some(seen) = expect.last_root {
        let seen = hex(seen);
        if record.previous_root != seen && record.next_root != seen {
            return Err(Refusal::RolledBack { seen, offered: record.previous_root.clone() });
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

/// The hash of one source, for a package that is one file.
pub fn code_hash(source: &str) -> Vec<u8> {
    Sha256::digest(source.as_bytes()).to_vec()
}

/// The hash of a package's sources, for a server that holds the package rather
/// than a hash somebody told it.
///
/// **There has to be one answer to this.** A package is several files, and the
/// wallet that runs it, the publisher's server that checks the record, and the
/// tool that signed the package each have to turn those files into the same
/// bytes — or every record is refused as code nobody published, and the reason
/// is a join somebody wrote twice.
///
/// Sorted by path, and each file contributes its path and its length as well as
/// its text: joined with a newline alone, a line moved from one file to another
/// would leave the hash unchanged, and the package that ran would not be the
/// package that was read.
pub fn code_hash_of(sources: &BTreeMap<String, String>) -> Vec<u8> {
    let mut h = Sha256::new();
    for (path, text) in sources {
        h.update((path.len() as u64).to_le_bytes());
        h.update(path.as_bytes());
        h.update((text.len() as u64).to_le_bytes());
        h.update(text.as_bytes());
    }
    h.finalize().to_vec()
}

/// A JSON reader for a payload whose shape is fixed and known.
///
/// Deliberately small and deliberately strict about nothing: the signature has
/// already been checked, so this is reading bytes that are known to be the ones
/// that were signed. A field that is missing reads as empty and the checks above
/// then refuse it, which is the same answer with a better sentence.
fn parse(json: &str) -> Result<Record, Refusal> {
    let str_at = |key: &str| -> String {
        let needle = format!("\"{key}\":\"");
        let Some(at) = json.find(&needle) else { return String::new() };
        let rest = &json[at + needle.len()..];
        let end = rest.find('"').unwrap_or(0);
        rest[..end].to_string()
    };
    let int_at = |key: &str| -> i64 {
        let needle = format!("\"{key}\":");
        let Some(at) = json.find(&needle) else { return 0 };
        let rest = &json[at + needle.len()..];
        let end = rest.find(|c: char| !c.is_ascii_digit() && c != '-').unwrap_or(rest.len());
        rest[..end].parse().unwrap_or(0)
    };

    // The effects, read as the objects they are. A publisher signs over these,
    // so they are the one part worth reading carefully.
    let mut effects = Vec::new();
    if let Some(at) = json.find("\"effects\":[") {
        let rest = &json[at + 11..];
        let mut depth = 0;
        let mut start = 0;
        for (i, c) in rest.char_indices() {
            match c {
                '{' => {
                    if depth == 0 {
                        start = i;
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        effects.push(effect(&rest[start..=i]));
                    }
                }
                ']' if depth == 0 => break,
                _ => {}
            }
        }
    }

    Ok(Record {
        vct: str_at("vct"),
        app: str_at("app"),
        version: str_at("version"),
        action: str_at("action"),
        code_hash: str_at("code_hash"),
        input_hash: str_at("input_hash"),
        previous_root: str_at("previous_root"),
        next_root: str_at("next_root"),
        policies: Vec::new(),
        capabilities: Vec::new(),
        effects,
        executed: int_at("executed"),
        time: int_at("time"),
        uuid: str_at("uuid"),
        outcome: str_at("outcome"),
    })
}

fn effect(json: &str) -> Effect {
    let capability = {
        let needle = "\"capability\":\"";
        json.find(needle)
            .map(|at| {
                let rest = &json[at + needle.len()..];
                rest[..rest.find('"').unwrap_or(0)].to_string()
            })
            .unwrap_or_default()
    };
    let credential = {
        let needle = "\"credential\":\"";
        json.find(needle).map(|at| {
            let rest = &json[at + needle.len()..];
            rest[..rest.find('"').unwrap_or(0)].to_string()
        })
    };
    let claims = credential.as_ref().and_then(|_| {
        let at = json.find("\"claims\":")?;
        Some(json[at + 9..].trim_end_matches('}').to_string())
    });

    Effect {
        capability,
        payload: match (credential, claims) {
            (Some(ty), Some(claims)) => Value::Credential {
                ty,
                claims: parse_claims(&claims),
                verified: None,
            },
            _ => Value::Null,
        },
        reversible: json.contains("\"reversible\":true"),
    }
}

/// The claims of an issuance: a flat object of strings and numbers, which is
/// what a credential's claims are.
fn parse_claims(json: &str) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let body = json.trim().trim_start_matches('{').trim_end_matches('}');
    let mut rest = body;
    while let Some(at) = rest.find('"') {
        let after = &rest[at + 1..];
        let Some(end) = after.find('"') else { break };
        let key = &after[..end];
        let Some(colon) = after[end..].find(':') else { break };
        let value_at = end + colon + 1;
        let value = after[value_at..].trim_start();
        if let Some(stripped) = value.strip_prefix('"') {
            let end = stripped.find('"').unwrap_or(0);
            out.insert(key.to_string(), Value::Str(stripped[..end].to_string()));
            rest = &stripped[end + 1..];
        } else {
            let end = value.find(|c: char| c == ',' || c == '}').unwrap_or(value.len());
            let text = value[..end].trim();
            out.insert(
                key.to_string(),
                text.parse::<i64>().map(Value::Int).unwrap_or_else(|_| Value::Str(text.to_string())),
            );
            rest = &value[end..];
        }
    }
    out
}
