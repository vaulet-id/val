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
use valang_runtime::attestation::jwt;
use valang_runtime::run_action;
use valang_verify::*;

const LOYALTY: &str = include_str!("../../../examples/loyalty.val");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn host() -> Fixture {
    Fixture::parse(WALLET).expect("the fixture parses")
}

/// The token, the device key, and the code hash — which is all a publisher's
/// server is handed.
fn a_run() -> (String, Vec<u8>, Vec<u8>) {
    let (program, diagnostics) = valang::analyse(LOYALTY);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let h = host();
    let run = run_action(&program, LOYALTY, "ScanToEarn", &h.state(), &BTreeMap::new(), &h);
    (jwt(&run.record), run.record.device_key.clone(), run.record.code_hash.to_vec())
}

fn never_spent(_: &str) -> bool {
    false
}

#[test]
fn a_good_record_yields_the_claims_the_server_will_sign() {
    let (token, key, code) = a_run();
    let expect = Expectation { code_hash: &code, device_key: &key, last_root: None, spent: &never_spent };

    // Three parts and a dot between them: a publisher with an ordinary JWT
    // library gets this far without any of our code.
    assert_eq!(token.split('.').count(), 3);

    let v = verify(&token, &expect).expect("verifies");
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
    let (token, _, code) = a_run();
    let somebody_else = [7u8; 32];
    let expect = Expectation { code_hash: &code, device_key: &somebody_else, last_root: None, spent: &never_spent };
    assert!(matches!(verify(&token, &expect), Err(Refusal::Unsigned(_))));
}

#[test]
fn a_record_from_code_this_publisher_did_not_publish_is_refused() {
    let (token, key, _) = a_run();
    let other = code_hash("app \"somebody.else\"\nversion 1\n");
    let expect = Expectation { code_hash: &other, device_key: &key, last_root: None, spent: &never_spent };
    match verify(&token, &expect) {
        Err(Refusal::UnknownCode { .. }) => {}
        other => panic!("expected an unknown-code refusal, got {other:?}"),
    }
}

/// The signature is over the bytes, so changing one is caught before anything is
/// read out of it — which is the order that matters. A verifier that parsed
/// first would be making decisions about a record it had not authenticated.
#[test]
fn a_record_changed_after_signing_is_refused() {
    let (token, key, code) = a_run();
    // Change one character of the payload and re-assemble.
    let mut parts: Vec<String> = token.split('.').map(str::to_string).collect();
    parts[1] = parts[1].replacen('a', "b", 1);
    let tampered = parts.join(".");
    let expect = Expectation { code_hash: &code, device_key: &key, last_root: None, spent: &never_spent };
    assert!(matches!(verify(&tampered, &expect), Err(Refusal::Unsigned(_))));
}

/// A run the host refused earned nothing, and the record says so. A server that
/// only looked at the effects would sign a credential for a batch nobody took.
#[test]
fn a_run_that_did_not_commit_is_refused() {
    let (program, _) = valang::analyse(LOYALTY);
    let h = host().refusing();
    let run = run_action(&program, LOYALTY, "ScanToEarn", &h.state(), &BTreeMap::new(), &h);

    let expect = Expectation {
        code_hash: &run.record.code_hash.to_vec(),
        device_key: &run.record.device_key,
        last_root: None,
        spent: &never_spent,
    };
    match verify(&jwt(&run.record), &expect) {
        Err(Refusal::DidNotCommit(why)) => assert!(why.starts_with("refused")),
        other => panic!("expected a did-not-commit refusal, got {other:?}"),
    }
}

/// The double-spend this system actually has: roll the state back and replay.
/// Remembering one hash per holder is the whole defence, and it is the server's
/// to remember — nobody else saw both records.
#[test]
fn a_record_reaching_behind_one_already_seen_is_refused() {
    let (token, key, code) = a_run();
    let seen = [9u8; 32];
    let expect = Expectation { code_hash: &code, device_key: &key, last_root: Some(&seen), spent: &never_spent };
    match verify(&token, &expect) {
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

/// Base64url with bits left over.
///
/// 64 bytes of signature is 86 base64url characters, which is 516 bits: the
/// last character carries four bits nobody reads. The decoder threw them away,
/// so sixteen different last characters decoded to the same signature and one
/// record verified under sixteen token strings — a server that remembers what
/// it has seen by the token it was handed would have seen sixteen.
#[test]
fn a_token_with_bits_left_over_is_not_the_same_token() {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let (token, key, code) = a_run();
    let expect =
        Expectation { code_hash: &code, device_key: &key, last_root: None, spent: &never_spent };
    verify(&token, &expect).expect("the record it was made from verifies");

    let parts: Vec<&str> = token.split('.').collect();
    let sig = parts[2];
    let last = sig.as_bytes()[sig.len() - 1];
    let value = ALPHABET.iter().position(|c| *c == last).expect("base64url") as u8;

    // Another character whose top two bits are the same: the four bits below
    // them are discarded, so it decodes to the same 64 bytes.
    let other = ALPHABET[((value & 0b11_0000) | ((value.wrapping_add(1)) & 0b00_1111)) as usize];
    let mut bytes = sig.as_bytes().to_vec();
    *bytes.last_mut().unwrap() = other;
    let twin = format!("{}.{}.{}", parts[0], parts[1], String::from_utf8(bytes).unwrap());
    assert_ne!(twin, token, "the test did not change the token");

    assert!(
        verify(&twin, &expect).is_err(),
        "one record verified under two token strings: the bits nobody reads were not checked"
    );
}
