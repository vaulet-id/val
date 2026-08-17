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

use serde_json::Value as Json;
use sha2::{Digest, Sha256};

use crate::host::{Context, EffectRequest, Host, Verdict};
use crate::value::Value;

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

    fn credentials_of(&self, ty: &str, _policy: Option<&str>, limit: Option<i64>) -> Vec<BTreeMap<String, Value>> {
        let mut rows = self.rows(ty);
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
        // A hash, not a signature. A real device signs with a key in secure
        // hardware, and nothing here should be mistaken for that.
        Sha256::digest(bytes).to_vec()
    }

    fn device_key(&self) -> Vec<u8> {
        b"fixture-device".to_vec()
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
            None => Value::Str(s.clone()),
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
