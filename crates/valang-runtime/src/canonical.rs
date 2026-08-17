//! The canonical encoding.
//!
//! State, input and code hashes need one, and it must be the same one
//! everywhere: a second canonicalisation is a second thing to get subtly wrong
//! (§7). This crate carries a reference implementation of deterministic CBOR —
//! RFC 8949 §4.2.1 — behind a trait, so a host that already has one supplies
//! it instead of the language carrying a rival.

use crate::value::Value;

pub trait Canonical {
    fn encode(&self, v: &Value) -> Vec<u8>;
}

/// RFC 8949 §4.2.1: shortest-form argument encoding, and map keys sorted by
/// their encoded bytes. Both rules exist so that one value has one encoding.
pub struct DeterministicCbor;

impl Canonical for DeterministicCbor {
    fn encode(&self, v: &Value) -> Vec<u8> {
        let mut out = Vec::new();
        write(v, &mut out);
        out
    }
}

/// A private-use CBOR tag. Nothing else in this encoding is tagged, so a tag is
/// unambiguous where a shape is not.
pub const TAG_ENUM: u64 = 40_001;

fn head(major: u8, arg: u64, out: &mut Vec<u8>) {
    // Shortest form. A length of 10 encoded in eight bytes would be legal CBOR
    // and a different byte string for the same value, which is the whole thing
    // determinism forbids.
    let m = major << 5;
    match arg {
        0..=23 => out.push(m | arg as u8),
        24..=0xff => {
            out.push(m | 24);
            out.push(arg as u8);
        }
        0x100..=0xffff => {
            out.push(m | 25);
            out.extend_from_slice(&(arg as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(m | 26);
            out.extend_from_slice(&(arg as u32).to_be_bytes());
        }
        _ => {
            out.push(m | 27);
            out.extend_from_slice(&arg.to_be_bytes());
        }
    }
}

fn write(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Null => out.push(0xf6),
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Int(i) if *i >= 0 => head(0, *i as u64, out),
        Value::Int(i) => head(1, (-1 - *i) as u64, out),
        Value::Bytes(b) => {
            head(2, b.len() as u64, out);
            out.extend_from_slice(b);
        }
        Value::Str(s) => {
            head(3, s.len() as u64, out);
            out.extend_from_slice(s.as_bytes());
        }
        Value::List(items) => {
            head(4, items.len() as u64, out);
            for i in items {
                write(i, out);
            }
        }
        Value::Map(m) => {
            let mut entries: Vec<(Vec<u8>, &Value)> = m
                .iter()
                .map(|(k, v)| {
                    let mut kb = Vec::new();
                    write(&Value::Str(k.clone()), &mut kb);
                    (kb, v)
                })
                .collect();
            // By encoded bytes, not by the Rust string ordering — they differ
            // once a key is long enough to change the length prefix.
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            head(5, entries.len() as u64, out);
            for (k, v) in entries {
                out.extend_from_slice(&k);
                write(v, out);
            }
        }
        // Tagged, not a bare pair. Encoded as an array of two strings, an enum
        // member and a list of two strings — `["th", "en"]`, say — are the same
        // bytes, which is the one thing a canonical encoding may never allow:
        // two different values that hash alike.
        Value::Enum(e, m) => {
            head(6, TAG_ENUM, out);
            head(4, 2, out);
            write(&Value::Str(e.clone()), out);
            write(&Value::Str(m.clone()), out);
        }
        Value::Credential { ty, claims, verified } => {
            let mut m = std::collections::BTreeMap::new();
            m.insert("type".to_string(), Value::Str(ty.clone()));
            m.insert("claims".to_string(), Value::Map(claims.clone()));
            if let Some(p) = verified {
                m.insert("verified".to_string(), Value::Str(p.clone()));
            }
            write(&Value::Map(m), out);
        }
    }
}
