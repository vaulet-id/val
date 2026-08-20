//! The whole path a wallet takes: bytes arrive, the host admits them, the
//! program runs, and a record is signed.
//!
//! This is what "installing a Micro App" is. It is written as one test on
//! purpose — the steps are only worth anything together, and each of them
//! existed separately for a while without anything proving they composed.

use std::collections::BTreeMap;

use valang_package::*;
use valang_runtime::fixture::Fixture;

const LOYALTY: &str = include_str!("../../../examples/loyalty.val");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn registries() -> valang::capability::Hosts {
    valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")])
}

/// This host: it draws with the core catalogue and admits the `val` tier.
struct Wallet;

impl HostPolicy for Wallet {
    fn registries(&self) -> valang::capability::Hosts {
        registries()
    }
}

fn manifest() -> Manifest {
    Manifest {
        app: "th.co.codefin.loyalty".into(),
        version: "1".into(),
        kind: "val".into(),
        publisher: "did:web:codefin.io".into(),
        catalogue: "1".into(),
        locales: vec!["th".into(), "en".into()],
    }
}

fn text() -> BTreeMap<String, BTreeMap<String, String>> {
    let entry = |th: &str, en: &str| {
        BTreeMap::from([("th".to_string(), th.to_string()), ("en".to_string(), en.to_string())])
    };
    BTreeMap::from([
        ("balance".to_string(), entry("แต้ม {points}", "{points} points")),
        (
            "tooSmallToEarn".to_string(),
            entry("ยอดต่ำกว่า 20 บาท ยังไม่ได้แต้ม", "Purchases under ฿20 do not earn points"),
        ),
    ])
}

fn packaged() -> Vec<u8> {
    let key = keygen();
    let sources = BTreeMap::from([("loyalty.val".to_string(), LOYALTY.to_string())]);
    let pkg = build(manifest(), sources, text(), &registries(), Some(&key)).expect("builds");
    encode(&pkg)
}

#[test]
fn a_package_arrives_as_bytes_and_an_action_runs() {
    let bytes = packaged();

    // What the wallet is handed. Nothing before this point is trusted.
    let pkg = read(&bytes).expect("the bytes are a package");
    let installed = install_with(&pkg, &Wallet).expect("this host admits it");
    let code = installed.code.expect("a `val` package carries code");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&code.program, &host.state());
    let run = valang_runtime::run_action(
        &code.program,
        &code.source,
        "ScanToEarn",
        &state,
        &BTreeMap::new(),
        &host,
    );

    assert!(
        matches!(run.outcome, valang_runtime::Outcome::Committed),
        "the action did not commit: {:?}",
        run.outcome
    );
    assert_eq!(run.record.app, "th.co.codefin.loyalty");
    assert_ne!(run.record.previous_root, run.record.next_root, "the state did not move");
    assert!(!run.record.signature.is_empty(), "nothing signed the record");
}

/// The property the two-compile version could not have: **what was checked is
/// what runs.** The record hashes the source the program was compiled from, and
/// that is the text the package carried — so a verifier holding the package and
/// holding the record can say they are the same code, which is the whole claim.
#[test]
fn the_record_names_the_code_that_was_admitted() {
    let bytes = packaged();
    let pkg = read(&bytes).expect("the bytes are a package");
    let installed = install_with(&pkg, &Wallet).expect("admitted");
    let code = installed.code.expect("carries code");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&code.program, &host.state());
    let run = valang_runtime::run_action(
        &code.program,
        &code.source,
        "ScanToEarn",
        &state,
        &BTreeMap::new(),
        &host,
    );

    // Computed here from the package's own text, by a different route than the
    // runtime took to fill the record in.
    let expected: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(code.source.as_bytes()).into();
    assert_eq!(run.record.code_hash, expected, "the record names other code than the package carried");
}

/// A source changed after signing is refused, and the program never exists —
/// there is nothing to accidentally run.
#[test]
fn a_modified_package_never_produces_a_program() {
    let key = keygen();
    let sources = BTreeMap::from([("loyalty.val".to_string(), LOYALTY.to_string())]);
    let mut pkg = build(manifest(), sources, text(), &registries(), Some(&key)).expect("builds");
    pkg.sources.insert("loyalty.val".to_string(), LOYALTY.replace("version 1", "version 2"));

    assert!(matches!(install_with(&pkg, &Wallet), Err(Refusal::Modified(_))));
}
