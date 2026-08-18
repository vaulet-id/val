//! The SDK a Rust handler is given.
//!
//! Verification already happened, in the runner, before this process started.
//! There is one verifier and every language talks to it the same way: the
//! runner hands the result in on stdin and this module shapes it.

use serde_json::{json, Value};
use std::io::Read;

pub struct Checked {
    pub ok: bool,
    pub refusal: Value,
    pub record: Value,
    pub effects: Vec<Value>,
}

pub type Decision = Value;

pub struct Sdk {
    checked: Value,
}

impl Sdk {
    pub fn verify(&self, _token: &str) -> Result<Checked, Value> {
        let ok = self.checked["ok"].as_bool().unwrap_or(false);
        if !ok {
            return Err(self.checked["refusal"].clone());
        }
        Ok(Checked {
            ok,
            refusal: self.checked["refusal"].clone(),
            record: self.checked["record"].clone(),
            effects: self.checked["effects"].as_array().cloned().unwrap_or_default(),
        })
    }

    pub fn issuance(&self, checked: &Checked, credential: &str) -> Option<Value> {
        checked.effects.iter().find_map(|e| {
            if e["capability"] != "credential.issue" {
                return None;
            }
            if e["payload"]["credential"] == credential {
                Some(e["payload"]["claims"].clone())
            } else {
                None
            }
        })
    }

    pub fn issue(&self, credential: &str, claims: Value) -> Decision {
        json!({ "kind": "issue", "credential": credential, "claims": claims })
    }

    pub fn accept(&self, note: &str) -> Decision {
        json!({ "kind": "accept", "note": note })
    }

    pub fn refuse(&self, refusal: Value) -> Decision {
        json!({ "kind": "refuse", "refusal": refusal })
    }
}

pub fn main_with(handle: fn(&str, &Sdk) -> Decision) {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw).expect("stdin");
    let payload: Value = serde_json::from_str(&raw).expect("json");
    let sdk = Sdk { checked: payload["checked"].clone() };
    let decision = handle(payload["token"].as_str().unwrap_or(""), &sdk);
    print!("{}", decision);
}
