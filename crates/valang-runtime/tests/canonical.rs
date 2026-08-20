//! The encoding every hash is computed over.
//!
//! A state root, a code hash and an input hash are all this encoding, so a
//! value that encodes two ways or two values that encode alike are wrong in the
//! one place nothing downstream can notice: two records would disagree about
//! what happened, or agree about what did not.

use std::collections::BTreeMap;

use valang_runtime::canonical::{Canonical, DeterministicCbor};
use valang_runtime::decode::decode;
use valang_runtime::value::Value;

fn map(pairs: &[(&str, Value)]) -> Value {
    Value::Map(pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

/// Every shape, including the ones whose head changes size.
fn every_shape() -> Vec<Value> {
    vec![
        Value::Null,
        Value::Bool(true),
        Value::Bool(false),
        Value::Int(0),
        Value::Int(-1),
        Value::Int(23),
        Value::Int(24),
        Value::Int(255),
        Value::Int(256),
        Value::Int(65_536),
        Value::Int(i64::MAX),
        Value::Int(i64::MIN),
        Value::Str(String::new()),
        Value::Str("a".into()),
        Value::Str("ยาวพอที่จะเปลี่ยนหัวของมัน เพราะมันยาวเกินยี่สิบสามไบต์".into()),
        Value::Bytes(Vec::new()),
        Value::Bytes(vec![0, 1, 255]),
        Value::List(Vec::new()),
        Value::List(vec![Value::Int(1), Value::Str("two".into())]),
        Value::Enum("Tier".into(), "gold".into()),
        map(&[]),
        map(&[("b", Value::Int(2)), ("a", Value::Int(1))]),
        map(&[("nested", map(&[("deep", Value::List(vec![Value::Null]))]))]),
        Value::Credential {
            ty: "PurchaseReceipt".into(),
            claims: [("amount".to_string(), Value::Int(120))].into_iter().collect(),
            verified: None,
        },
        Value::Credential {
            ty: "PurchaseReceipt".into(),
            claims: [("amount".to_string(), Value::Int(120))].into_iter().collect(),
            verified: Some("ReceiptFromMerchant".into()),
        },
    ]
}

/// What was signed has to be what a verifier reads back.
#[test]
fn encoding_and_decoding_are_inverses() {
    let enc = DeterministicCbor;
    for v in every_shape() {
        let bytes = enc.encode(&v);
        let back = decode(&bytes).unwrap_or_else(|e| panic!("{v:?} did not decode: {e:?}"));
        assert_eq!(back, v, "{v:?} came back as {back:?}");
    }
}

/// Two values that hash alike is the one thing a canonical encoding may never
/// allow. The pairs below are the ones a shape-based encoding would confuse.
#[test]
fn no_two_values_encode_alike() {
    let enc = DeterministicCbor;
    let mut seen: BTreeMap<Vec<u8>, Value> = BTreeMap::new();
    let mut all = every_shape();
    all.push(Value::Str("Tier.gold".into()));
    all.push(Value::List(vec![Value::Str("Tier".into()), Value::Str("gold".into())]));
    all.push(map(&[("type", Value::Str("PurchaseReceipt".into()))]));
    all.push(Value::Str("1".into()));
    all.push(Value::Int(1));
    all.push(Value::Bytes(b"1".to_vec()));

    for v in all {
        let bytes = enc.encode(&v);
        if let Some(other) = seen.get(&bytes) {
            assert_eq!(other, &v, "{v:?} and {other:?} encode to the same bytes");
        }
        seen.insert(bytes, v);
    }
}

/// One value has one encoding, whatever order its map was built in.
#[test]
fn one_value_has_one_encoding() {
    let enc = DeterministicCbor;

    let mut forwards = BTreeMap::new();
    forwards.insert("a".to_string(), Value::Int(1));
    forwards.insert("zzzzzzzzzzzzzzzzzzzzzzzzzzzz".to_string(), Value::Int(2));
    let mut backwards = BTreeMap::new();
    backwards.insert("zzzzzzzzzzzzzzzzzzzzzzzzzzzz".to_string(), Value::Int(2));
    backwards.insert("a".to_string(), Value::Int(1));

    assert_eq!(enc.encode(&Value::Map(forwards)), enc.encode(&Value::Map(backwards)));
}

/// Map keys are ordered by their encoded bytes, which is length first — the
/// rule differs from Rust's string ordering exactly where a key is long enough
/// to change its head, and that is where a second implementation would diverge.
#[test]
fn map_keys_are_ordered_by_their_encoded_bytes() {
    let enc = DeterministicCbor;
    // "b" is one byte and sorts after "aaaaaaaaaaaaaaaaaaaaaaaaaaaa" as a
    // string, and before it once the length prefix is in front.
    let long = "aaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let bytes = enc.encode(&map(&[(long, Value::Int(1)), ("b", Value::Int(2))]));

    let first_key_starts_at = 1; // after the map head
    assert_eq!(
        bytes[first_key_starts_at], 0x61,
        "the one-byte key did not come first: keys are not ordered by encoded bytes"
    );
}
