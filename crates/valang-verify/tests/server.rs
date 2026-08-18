//! What a publisher's server does, end to end.
//!
//! The device runs an action and signs the record; the server checks it and
//! decides whether to sign a credential. Every refusal below is one a real
//! server has to make, and the point of each test is that the check is what
//! catches it — not the signature, not good faith.

use std::collections::BTreeMap;

use valang_runtime::fixture::Fixture;
use valang_runtime::host::Host;
use valang_runtime::value::Value;
use valang_runtime::{encode_record, run_action};
use valang_verify::*;

const LOYALTY: &str = include_str!("../../../examples/loyalty.val");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn host() -> Fixture {
    Fixture::parse(WALLET).expect("the fixture parses")
}

fn a_run() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let (program, diagnostics) = valang::analyse(LOYALTY);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let h = host();
    let run = run_action(&program, LOYALTY, "ScanToEarn", &h.state(), &BTreeMap::new(), &h);
    (
        encode_record(&run.record),
        run.record.signature.clone(),
        run.record.device_key.clone(),
        run.record.code_hash.to_vec(),
    )
}

fn never_spent(_: &str) -> bool {
    false
}

#[test]
fn a_good_record_yields_the_claims_the_server_will_sign() {
    let (bytes, sig, key, code) = a_run();
    let expect = Expectation {
        code_hash: &code,
        device_key: &key,
        last_root: None,
        spent: &never_spent,
    };

    let v = verify(&bytes, &sig, &expect).expect("verifies");
    assert_eq!(v.record.action, "ScanToEarn");
    assert_eq!(v.record.outcome, "committed");

    // What the server signs over: the claims the *record* shows being issued,
    // not whatever the client asked for. Signing the request instead would make
    // every check here decorative.
    let claims = issuance(&v, "LoyaltyMember").expect("this record issues one");
    assert_eq!(claims["points"], Value::Int(1_365));
    assert_eq!(claims["member_id"], Value::Str("M-2891".into()));
}

#[test]
fn a_record_signed_by_another_device_is_refused() {
    let (bytes, sig, _, code) = a_run();
    let somebody_else = [7u8; 32];
    let expect = Expectation { code_hash: &code, device_key: &somebody_else, last_root: None, spent: &never_spent };
    assert!(matches!(verify(&bytes, &sig, &expect), Err(Refusal::Unsigned(_))));
}

#[test]
fn a_record_from_code_this_publisher_did_not_publish_is_refused() {
    let (bytes, sig, key, _) = a_run();
    let other = code_hash("app \"somebody.else\"\nversion 1\n");
    let expect = Expectation { code_hash: &other, device_key: &key, last_root: None, spent: &never_spent };
    match verify(&bytes, &sig, &expect) {
        Err(Refusal::UnknownCode { .. }) => {}
        other => panic!("expected an unknown-code refusal, got {other:?}"),
    }
}

/// The signature is over the bytes, so changing one is caught before anything is
/// read out of it — which is the order that matters. A verifier that parsed
/// first would be making decisions about a record it had not authenticated.
#[test]
fn a_record_changed_after_signing_is_refused() {
    let (mut bytes, sig, key, code) = a_run();
    let at = bytes.len() / 2;
    bytes[at] ^= 0xff;
    let expect = Expectation { code_hash: &code, device_key: &key, last_root: None, spent: &never_spent };
    assert!(matches!(verify(&bytes, &sig, &expect), Err(Refusal::Unsigned(_))));
}

/// A run the host refused earned nothing, and the record says so. A server that
/// only looked at the effects would sign a credential for a batch nobody took.
#[test]
fn a_run_that_did_not_commit_is_refused() {
    let (program, _) = valang::analyse(LOYALTY);
    let h = host().refusing();
    let run = run_action(&program, LOYALTY, "ScanToEarn", &h.state(), &BTreeMap::new(), &h);

    let bytes = encode_record(&run.record);
    let expect = Expectation {
        code_hash: &run.record.code_hash.to_vec(),
        device_key: &run.record.device_key,
        last_root: None,
        spent: &never_spent,
    };
    match verify(&bytes, &run.record.signature, &expect) {
        Err(Refusal::DidNotCommit(why)) => assert!(why.starts_with("refused")),
        other => panic!("expected a did-not-commit refusal, got {other:?}"),
    }
}

/// The double-spend this system actually has: roll the state back and replay.
/// Remembering one hash per holder is the whole defence, and it is the server's
/// to remember — nobody else saw both records.
#[test]
fn a_record_reaching_behind_one_already_seen_is_refused() {
    let (bytes, sig, key, code) = a_run();
    let seen = [9u8; 32];
    let expect = Expectation { code_hash: &code, device_key: &key, last_root: Some(&seen), spent: &never_spent };
    match verify(&bytes, &sig, &expect) {
        Err(Refusal::RolledBack { .. }) => {}
        other => panic!("expected a rollback refusal, got {other:?}"),
    }
}

#[test]
fn a_nullifier_already_spent_is_refused() {
    let spent = |n: &str| n == "already-used";
    let expect = Expectation { code_hash: &[], device_key: &[], last_root: None, spent: &spent };
    assert!(check_spent("fresh", &expect).is_ok());
    assert!(matches!(check_spent("already-used", &expect), Err(Refusal::AlreadySpent(_))));
}
