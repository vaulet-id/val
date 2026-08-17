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
