//! The state root.
//!
//! Leaves are the `(path, value)` pairs `update` patches, canonically encoded
//! and sorted by path, so a single field can be proved without opening the rest
//! (§7). A list is one leaf per element, which reveals its length and is the
//! trade that section takes deliberately.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::canonical::Canonical;
use crate::value::Value;

pub type Hash = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leaf {
    pub path: String,
    pub value: Value,
    pub hash: Hash,
}

pub fn hex(h: &Hash) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn leaves(state: &BTreeMap<String, Value>, enc: &dyn Canonical) -> Vec<Leaf> {
    let mut out = Vec::new();
    for (k, v) in state {
        walk(k.clone(), v, enc, &mut out);
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn walk(path: String, v: &Value, enc: &dyn Canonical, out: &mut Vec<Leaf>) {
    match v {
        Value::Map(m) => {
            for (k, inner) in m {
                walk(format!("{path}.{k}"), inner, enc, out);
            }
        }
        Value::List(items) => {
            for (i, inner) in items.iter().enumerate() {
                walk(format!("{path}[{i}]"), inner, enc, out);
            }
        }
        leaf => {
            let mut h = Sha256::new();
            // The path is inside the hash. Without it two fields holding the
            // same value would produce interchangeable proofs.
            h.update(path.as_bytes());
            h.update([0u8]);
            h.update(enc.encode(leaf));
            out.push(Leaf { path, value: leaf.clone(), hash: h.finalize().into() });
        }
    }
}

pub fn root(leaves: &[Leaf]) -> Hash {
    if leaves.is_empty() {
        return Sha256::digest(b"").into();
    }
    let mut level: Vec<Hash> = leaves.iter().map(|l| l.hash).collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let mut h = Sha256::new();
            h.update(pair[0]);
            // An odd node is paired with itself. Simple, and the alternative —
            // promoting it — makes two different trees share a root.
            h.update(pair.get(1).unwrap_or(&pair[0]));
            next.push(h.finalize().into());
        }
        level = next;
    }
    level[0]
}
