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

/// One step up the tree: the sibling, and which side it was on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub sibling: Hash,
    /// True when the sibling is to the right of the node being proved.
    pub sibling_on_right: bool,
}

/// An inclusion proof, which is the entire reason state is a tree and not a
/// single hash: one field can be shown without opening the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inclusion {
    pub path: String,
    pub value: Value,
    pub steps: Vec<Step>,
}

pub fn prove(leaves: &[Leaf], path: &str) -> Option<Inclusion> {
    let mut index = leaves.iter().position(|l| l.path == path)?;
    let leaf = &leaves[index];
    let mut level: Vec<Hash> = leaves.iter().map(|l| l.hash).collect();
    let mut steps = Vec::new();

    while level.len() > 1 {
        let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };
        // An odd node is paired with itself when the root is built, so its
        // sibling is itself. Stated here rather than left as an off-by-one for
        // somebody to find later with a proof that will not verify.
        let sibling = *level.get(sibling_index).unwrap_or(&level[index]);
        steps.push(Step { sibling, sibling_on_right: index % 2 == 0 });

        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let mut h = Sha256::new();
            h.update(pair[0]);
            h.update(pair.get(1).unwrap_or(&pair[0]));
            next.push(h.finalize().into());
        }
        level = next;
        index /= 2;
    }

    Some(Inclusion { path: leaf.path.clone(), value: leaf.value.clone(), steps })
}

/// Check a proof against a root, having been told nothing else. This is what a
/// verifier runs: it never sees the state, only the field it was shown and the
/// root it already had.
pub fn verify_inclusion(inclusion: &Inclusion, root: &Hash, enc: &dyn Canonical) -> bool {
    let mut h = Sha256::new();
    h.update(inclusion.path.as_bytes());
    h.update([0u8]);
    h.update(enc.encode(&inclusion.value));
    let mut current: Hash = h.finalize().into();

    for step in &inclusion.steps {
        let mut h = Sha256::new();
        if step.sibling_on_right {
            h.update(current);
            h.update(step.sibling);
        } else {
            h.update(step.sibling);
            h.update(current);
        }
        current = h.finalize().into();
    }
    current == *root
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
