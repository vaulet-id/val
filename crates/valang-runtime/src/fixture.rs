//! A host built from `fixtures/wallet.json`.
//!
//! Three places used to invent this separately — the tests here, `valrun`, and
//! the preview — which meant three answers to "what is on this phone" and no
//! way to tell which one a bug was about. One file, and the same file the
//! playground lets somebody edit.
//!
//! It is a stub and it behaves like one: it approves every batch and signs with
//! a hash. What it is faithful about is *shape* — what a host is asked and what
//! it hands back.

use std::collections::BTreeMap;

use ed25519_dalek::{Signer, SigningKey};
use serde_json::Value as Json;

use crate::host::{Context, EffectRequest, Host, Verdict};
use crate::value::Value;

/// The device key this stub signs with.
///
/// A fixed seed, in the open, on purpose: it is a development key and anything
/// that looked like a secret here would eventually be treated as one. A real
/// device holds one in secure hardware and it never leaves.
const DEV_SEED: [u8; 32] = *b"val fixture device key, not real";

pub struct Fixture {
    json: Json,
    /// What the host will do with the batch. A stub that only ever says yes
    /// cannot be used to test what happens when somebody says no.
    pub approve: bool,
}

impl Fixture {
    pub fn parse(text: &str) -> Result<Fixture, String> {
        Ok(Fixture { json: serde_json::from_str(text).map_err(|e| e.to_string())?, approve: true })
    }

    pub fn refusing(mut self) -> Self {
        self.approve = false;
        self
    }

    /// The `state` a run starts from.
    pub fn state(&self) -> BTreeMap<String, Value> {
        match convert(&self.json["state"]) {
            Value::Map(m) => m,
            _ => BTreeMap::new(),
        }
    }

    fn rows(&self, ty: &str) -> Vec<BTreeMap<String, Value>> {
        let Some(rows) = self.json["credentials"][ty]["rows"].as_array() else { return Vec::new() };
        rows.iter()
            .filter_map(|r| match convert(r) {
                Value::Map(m) => Some(m),
                _ => None,
            })
            .collect()
    }
}

impl Host for Fixture {
    fn context(&self) -> Context {
        let time = self.json["context"]["time"].as_str().unwrap_or("");
        Context {
            time_now: parse_time(time),
            random_uuid: self.json["context"]["uuid"].as_str().unwrap_or("").to_string(),
        }
    }

    fn credential(&self, ty: &str, _policy: Option<&str>) -> Option<BTreeMap<String, Value>> {
        self.rows(ty).into_iter().next()
    }

    fn credentials_of(
        &self,
        ty: &str,
        _policy: Option<&str>,
        order: Option<(&str, bool)>,
        limit: Option<i64>,
    ) -> Vec<BTreeMap<String, Value>> {
        let mut rows = self.rows(ty);
        // Sorted before it is cut, or `order by … limit 5` would be five of
        // whatever came first and then sorted — which is a different five.
        if let Some((claim, descending)) = order {
            rows.sort_by(|a, b| {
                let (x, y) = (a.get(claim), b.get(claim));
                let ordering = match (x, y) {
                    (Some(Value::Int(x)), Some(Value::Int(y))) => x.cmp(y),
                    (Some(Value::Str(x)), Some(Value::Str(y))) => x.cmp(y),
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Equal,
                };
                if descending {
                    ordering.reverse()
                } else {
                    ordering
                }
            });
        }
        if let Some(n) = limit {
            rows.truncate(n.max(0) as usize);
        }
        rows
    }

    fn query(&self, audience: &str, _operation: &str) -> Vec<Value> {
        self.json["queries"][audience]
            .as_array()
            .map(|rows| rows.iter().map(convert).collect())
            .unwrap_or_default()
    }

    fn decide(&self, _effects: &[EffectRequest]) -> Verdict {
        if self.approve {
            Verdict::Approved
        } else {
            Verdict::Refused("the person said no".into())
        }
    }

    fn sign(&self, bytes: &[u8]) -> Vec<u8> {
        SigningKey::from_bytes(&DEV_SEED).sign(bytes).to_bytes().to_vec()
    }

    fn device_key(&self) -> Vec<u8> {
        SigningKey::from_bytes(&DEV_SEED).verifying_key().to_bytes().to_vec()
    }
}

/// JSON to a runtime value. Times are the interesting case: an ISO-8601 string
/// in the file becomes an integer here, because that is what comparing it to
/// `context.time.now` needs and a string would compare false and say nothing.
fn convert(j: &Json) -> Value {
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Int(n.as_i64().unwrap_or(0)),
        Json::String(s) => match parse_time_opt(s) {
            Some(ms) => Value::Int(ms),
            // `Tier.bronze` is an enum member, not the string "Tier.bronze".
            // JSON has no way to say so, and the run showed it: a state that
            // came in as a string and went out as a member reported every field
            // as changed, every time.
            None => match s.split_once('.') {
                Some((ty, member))
                    if ty.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                        && !member.contains('.')
                        && !member.is_empty() =>
                {
                    Value::Enum(ty.to_string(), member.to_string())
                }
                _ => Value::Str(s.clone()),
            },
        },
        Json::Array(items) => Value::List(items.iter().map(convert).collect()),
        Json::Object(map) => Value::Map(
            map.iter()
                .filter(|(k, _)| !k.starts_with('_'))
                .map(|(k, v)| (k.clone(), convert(v)))
                .collect(),
        ),
    }
}

fn parse_time(s: &str) -> i64 {
    parse_time_opt(s).unwrap_or(0)
}

/// `2026-08-17T10:30:00Z` to milliseconds. Written out rather than pulled in:
/// one format, fixed width, and a date library would be a dependency carried
/// for eleven characters.
fn parse_time_opt(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || *b.last()? != b'Z' {
        return None;
    }
    let num = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, sec) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);

    // Days since the epoch, by the civil-from-days algorithm — no leap-second
    // table, because neither has one and both are wrong about the same things.
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;

    Some(((days * 24 + h) * 60 + mi) * 60_000 + sec * 1_000)
}
