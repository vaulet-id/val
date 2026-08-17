//! Running the loyalty card, against numbers this crate did not produce.
//!
//! The expected values are pinned from the playground's TypeScript simulator,
//! which walked the same source with an independently written evaluator before
//! this one existed. Two implementations agreeing is worth something; this
//! crate agreeing with itself would not be.

use std::collections::BTreeMap;

use valang_runtime::canonical::{Canonical, DeterministicCbor};
use valang_runtime::host::{Context, EffectRequest, Host, Verdict};
use valang_runtime::value::Value;
use valang_runtime::{run_action, Outcome};

const LOYALTY: &str = include_str!("../../../examples/loyalty.val");

struct Wallet {
    approve: bool,
}

impl Host for Wallet {
    fn context(&self) -> Context {
        Context { time_now: 1_755_426_600_000, random_uuid: "0f2a-c71b".into() }
    }

    fn credential(&self, ty: &str, _policy: Option<&str>) -> Option<BTreeMap<String, Value>> {
        let mut c = BTreeMap::new();
        if ty == "NationalId" {
            c.insert("country".into(), Value::Str("TH".into()));
            c.insert("birthdate".into(), Value::Int(820_454_400_000));
            return Some(c);
        }
        if ty != "PurchaseReceipt" {
            return None;
        }
        c.insert("merchant".into(), Value::Str("Codefin Coffee".into()));
        c.insert("amount".into(), Value::Int(12_500)); // satang
        c.insert("purchased_at".into(), Value::Int(1_755_335_520_000));
        Some(c)
    }

    fn decide(&self, _effects: &[EffectRequest]) -> Verdict {
        if self.approve {
            Verdict::Approved
        } else {
            Verdict::Refused("the person said no".into())
        }
    }
    fn sign(&self, bytes: &[u8]) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        Sha256::digest(bytes).to_vec()
    }
    fn device_key(&self) -> Vec<u8> {
        b"test-device".to_vec()
    }
}

fn member(points: i64, tier: &str) -> Value {
    let mut m = BTreeMap::new();
    m.insert("member_id".into(), Value::Str("M-2891".into()));
    m.insert("points".into(), Value::Int(points));
    m.insert("tier".into(), Value::Enum("Tier".into(), tier.into()));
    Value::Map(m)
}

fn start() -> BTreeMap<String, Value> {
    let mut s = BTreeMap::new();
    s.insert("lifetimePoints".into(), Value::Int(1_240));
    s.insert("member".into(), member(1_240, "bronze"));
    s
}

#[test]
fn a_scan_earns_one_point_per_baht_and_commits() {
    let (program, diagnostics) = valang::analyse(LOYALTY);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let run = run_action(&program, LOYALTY, "ScanToEarn", &start(), &BTreeMap::new(), &Wallet { approve: true });
    assert_eq!(run.outcome, Outcome::Committed);

    // 12_500 satang / 100 = 125 points, on top of 1_240.
    assert_eq!(run.next_state["lifetimePoints"], Value::Int(1_365));
    let m = run.next_state["member"].field("points").cloned();
    assert_eq!(m, Some(Value::Int(1_365)));

    assert_eq!(run.effects.len(), 1);
    assert_eq!(run.effects[0].capability, "credential.issue");
    assert_ne!(run.record.previous_root, run.record.next_root);

    // What is in the credential, not only that one was asked for. The example
    // fills these from `next`, and checking the capability alone let a version
    // that issued three nulls pass.
    let Value::Credential { ty, claims, .. } = &run.effects[0].payload else {
        panic!("a credential was expected, found {}", run.effects[0].payload)
    };
    assert_eq!(ty, "LoyaltyMember");
    assert_eq!(claims["points"], Value::Int(1_365));
    assert_eq!(claims["member_id"], Value::Str("M-2891".into()));
    assert_eq!(claims["tier"], Value::Enum("Tier".into(), "bronze".into()));
}

#[test]
fn a_refused_batch_commits_nothing() {
    let (program, _) = valang::analyse(LOYALTY);
    let before = start();
    let run = run_action(&program, LOYALTY, "ScanToEarn", &before, &BTreeMap::new(), &Wallet { approve: false });

    assert!(matches!(run.outcome, Outcome::Refused(_)));
    assert_eq!(run.next_state, before, "the state must not move when the batch did not");
    assert_eq!(run.record.previous_root, run.record.next_root);
    assert_eq!(run.record.effects_executed, 0);
    // The request is still in the record: what was asked for is part of what
    // happened, whether or not it was granted.
    assert_eq!(run.record.effects_requested.len(), 1);
}

#[test]
fn the_same_state_encodes_to_the_same_bytes_whatever_order_it_was_built_in() {
    let enc = DeterministicCbor;
    let mut a = BTreeMap::new();
    a.insert("b".to_string(), Value::Int(2));
    a.insert("a".to_string(), Value::Int(1));
    let mut b = BTreeMap::new();
    b.insert("a".to_string(), Value::Int(1));
    b.insert("b".to_string(), Value::Int(2));
    assert_eq!(enc.encode(&Value::Map(a)), enc.encode(&Value::Map(b)));
}

/// A proof hands the host a statement, never the answer. Evaluating the
/// predicate here and shipping the boolean would be a disclosure wearing the
/// word `prove` — the failure mode §5 says the compiler must refuse.
#[test]
fn a_proof_carries_the_statement_and_not_its_value() {
    const DOOR: &str = include_str!("../../../examples/door.val");
    let (program, diagnostics) = valang::analyse(DOOR);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let run = run_action(&program, DOOR, "EnterVenue", &BTreeMap::new(), &BTreeMap::new(), &Wallet { approve: true });
    assert_eq!(run.effects.len(), 1, "one batch, one disclosure");
    let e = &run.effects[0];
    assert_eq!(e.capability, "disclosure.present");
    assert!(!e.reversible, "nothing un-tells somebody a postcode");

    let shown = e.payload.to_string();
    assert!(shown.contains("statement:"), "the predicate travels: {shown}");
    assert!(
        !shown.contains("kind: \"prove\", of:"),
        "the answer must not: {shown}"
    );
}

/// The property the whole scheme rests on: one value, one encoding, and two
/// different values never the same bytes. An enum member used to encode as a
/// bare pair of strings, which made `Tier.gold` and the list `["Tier", "gold"]`
/// indistinguishable — and a hash over bytes stops being a hash over a value
/// the moment that is true.
#[test]
fn two_different_values_are_never_the_same_bytes() {
    use valang_runtime::decode::decode;
    let enc = DeterministicCbor;

    let member = Value::Enum("Tier".into(), "gold".into());
    let pair = Value::List(vec![Value::Str("Tier".into()), Value::Str("gold".into())]);
    assert_ne!(enc.encode(&member), enc.encode(&pair));

    for v in [member, pair, Value::Int(-1_000_000), Value::Str("แต้ม".into()), Value::Bool(true), Value::Null] {
        assert_eq!(decode(&enc.encode(&v)).unwrap(), v, "round trip: {v}");
    }
}

/// A strict decoder is what makes re-encoding sound. Without it a package could
/// arrive encoded some other legal way, and a verifier that re-encodes would be
/// checking a signature against its own idea of the file.
#[test]
fn a_non_canonical_encoding_is_refused() {
    use valang_runtime::decode::{decode, Malformed};

    // 10, written in two bytes instead of one. Legal CBOR, not this encoding.
    assert!(matches!(decode(&[0x18, 0x0a]), Err(Malformed::NotShortest { .. })));
    // A map whose keys are out of order.
    let mut unsorted = vec![0xa2];
    unsorted.extend_from_slice(&[0x61, b'b', 0x01]);
    unsorted.extend_from_slice(&[0x61, b'a', 0x02]);
    assert!(matches!(decode(&unsorted), Err(Malformed::KeysNotSorted { .. })));
    // Trailing bytes after a complete value.
    assert!(matches!(decode(&[0x01, 0x01]), Err(Malformed::Trailing { .. })));
}

/// Test vectors from RFC 8949 appendix A, which is the point: an encoder
/// checked against its own idea of the format checks nothing.
#[test]
fn the_encoding_is_the_one_in_the_rfc() {
    let enc = DeterministicCbor;
    for (value, bytes) in [
        (Value::Int(0), vec![0x00]),
        (Value::Int(1), vec![0x01]),
        (Value::Int(10), vec![0x0a]),
        (Value::Int(23), vec![0x17]),
        (Value::Int(24), vec![0x18, 0x18]),
        (Value::Int(100), vec![0x18, 0x64]),
        (Value::Int(1000), vec![0x19, 0x03, 0xe8]),
        (Value::Int(1_000_000), vec![0x1a, 0x00, 0x0f, 0x42, 0x40]),
        (Value::Int(-1), vec![0x20]),
        (Value::Int(-10), vec![0x29]),
        (Value::Int(-100), vec![0x38, 0x63]),
        (Value::Bool(false), vec![0xf4]),
        (Value::Bool(true), vec![0xf5]),
        (Value::Null, vec![0xf6]),
        (Value::Str(String::new()), vec![0x60]),
        (Value::Str("a".into()), vec![0x61, 0x61]),
        (Value::Str("IETF".into()), vec![0x64, 0x49, 0x45, 0x54, 0x46]),
        (Value::List(vec![]), vec![0x80]),
        (
            Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
            vec![0x83, 0x01, 0x02, 0x03],
        ),
    ] {
        assert_eq!(enc.encode(&value), bytes, "encoding {value}");
    }
}

/// The reason state is a tree. A verifier is handed one field and the root it
/// already had, and can check the field belongs to that state without being
/// shown any of the rest of it.
#[test]
fn one_field_can_be_proved_without_opening_the_others() {
    use valang_runtime::merkle::{leaves, prove, root, verify_inclusion};

    let (program, _) = valang::analyse(LOYALTY);
    let run = run_action(&program, LOYALTY, "ScanToEarn", &start(), &BTreeMap::new(), &Wallet { approve: true });

    let enc = DeterministicCbor;
    let ls = leaves(&run.next_state, &enc);
    let r = root(&ls);
    assert_eq!(r, run.record.next_root, "the record's root is this tree's root");

    for leaf in &ls {
        let p = prove(&ls, &leaf.path).expect("every leaf is provable");
        assert!(verify_inclusion(&p, &r, &enc), "{} should verify", leaf.path);
        // And it is about that field only: the proof carries no other value.
        assert_eq!(p.path, leaf.path);
        assert_eq!(p.value, leaf.value);
    }

    // A field claimed with a value it does not have must not verify.
    let mut lying = prove(&ls, "member.points").unwrap();
    lying.value = Value::Int(999_999);
    assert!(!verify_inclusion(&lying, &r, &enc));

    // Nor an honest proof against a different state's root: a proof is about
    // one state, which is what makes a remembered root worth remembering.
    let other = root(&leaves(&start(), &enc));
    let honest = prove(&ls, "member.points").unwrap();
    assert!(!verify_inclusion(&honest, &other, &enc));
}

/// A refused run is signed too. The record is evidence of what happened, and
/// "the host would not take this batch" is something that happened — a record
/// that only attested to successes would be evidence of a different thing than
/// it claims to be.
#[test]
fn the_record_is_signed_whichever_way_the_run_went() {
    use valang_runtime::encode_record;

    let (program, _) = valang::analyse(LOYALTY);
    for approve in [true, false] {
        let run = run_action(&program, LOYALTY, "ScanToEarn", &start(), &BTreeMap::new(), &Wallet { approve });
        assert!(!run.record.signature.is_empty(), "unsigned with approve={approve}");
        assert_eq!(run.record.device_key, b"test-device".to_vec());

        // The outcome is inside the signed bytes, so the two runs cannot be
        // swapped for one another.
        let bytes = encode_record(&run.record);
        assert!(!bytes.is_empty());
        let sig = Wallet { approve }.sign(&bytes);
        assert_eq!(sig, run.record.signature, "the signature is over these bytes");
    }

    let a = run_action(&program, LOYALTY, "ScanToEarn", &start(), &BTreeMap::new(), &Wallet { approve: true });
    let b = run_action(&program, LOYALTY, "ScanToEarn", &start(), &BTreeMap::new(), &Wallet { approve: false });
    assert_ne!(a.record.signature, b.record.signature);
}
