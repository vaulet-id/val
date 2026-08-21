//! The path a wallet walks: bytes arrive, the wallet admits them, an action
//! runs, and a record is signed.
//!
//! **Nothing here compiles anything.** The package carries a module the
//! publisher built and signed; the wallet checks the bytes, reads what they can
//! do off them, and runs them. A step that needed a source would be a step that
//! needed a compiler on the phone.

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

/// The publisher's build, which is the only place a compiler runs.
fn published() -> Vec<u8> {
    let key = keygen();
    let sources = BTreeMap::from([("loyalty.val".to_string(), LOYALTY.to_string())]);
    let pkg = build(manifest(), sources, text(), &registries(), Some(&key)).expect("builds");
    encode(&pkg)
}

#[test]
fn a_wallet_installs_a_package_and_runs_it() {
    let bytes = published();

    // What the wallet is handed. Nothing before this point is trusted.
    let pkg = read(&bytes).expect("the bytes are a package");
    assert!(!pkg.module.is_empty(), "a package carries the module that runs");

    let installed = install_with(&pkg, &Wallet).expect("this host admits it");
    let code = installed.code.expect("a `val` package carries a module");

    // What the person is shown, derived from the module the wallet holds.
    let sheet = valang_wasm::compile::report_of_module(&code.module).expect("it says what it does");
    assert!(sheet.issues.contains("LoyaltyMember"), "{sheet}");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let module = valang_wasm::compile::Module {
        konsts: valang_wasm::konsts_of(&code.module).expect("its constants travel with it"),
        bytes: code.module.clone(),
        functions: Vec::new(),
    };
    let code_hash: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(&code.module).into();
    let mut engine = valang_wasm::WasmEngine::new(&module);
    let run = valang_runtime::run_action_with(
        &code.about,
        code_hash,
        "ScanToEarn",
        &code.about.initial(&host.state()),
        &BTreeMap::new(),
        &host,
        &mut engine,
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

/// **The record names the module.** Somebody holding the package and the record
/// can say the two are the same thing, without holding any source and without
/// having compiled anything.
#[test]
fn the_record_names_the_module_the_package_carried() {
    let bytes = published();
    let pkg = read(&bytes).expect("the bytes are a package");
    let code = install_with(&pkg, &Wallet).expect("admitted").code.expect("carries a module");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let module = valang_wasm::compile::Module {
        konsts: valang_wasm::konsts_of(&code.module).expect("constants"),
        bytes: code.module.clone(),
        functions: Vec::new(),
    };
    let code_hash: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(&pkg.module).into();
    let mut engine = valang_wasm::WasmEngine::new(&module);
    let run = valang_runtime::run_action_with(
        &code.about,
        code_hash,
        "ScanToEarn",
        &code.about.initial(&host.state()),
        &BTreeMap::new(),
        &host,
        &mut engine,
    );

    assert_eq!(run.record.code_hash, code_hash);
    assert_eq!(
        valang_package::hex_of(&pkg.module),
        pkg.integrity,
        "and the package says the same about those bytes"
    );
}

/// A module changed after signing is refused, and nothing is handed back to
/// run. This is the whole of what the signature is for once no source travels.
#[test]
fn a_modified_module_is_refused() {
    let key = keygen();
    let sources = BTreeMap::from([("loyalty.val".to_string(), LOYALTY.to_string())]);
    let mut pkg = build(manifest(), sources, text(), &registries(), Some(&key)).expect("builds");

    // One byte, somewhere nobody would look.
    let last = pkg.module.len() - 1;
    pkg.module[last] ^= 0xff;

    assert!(matches!(install_with(&pkg, &Wallet), Err(Refusal::Modified(_))));
}

/// And a module whose bytes still hash right but were signed by somebody else.
#[test]
fn a_package_signed_by_somebody_else_is_refused() {
    let sources = BTreeMap::from([("loyalty.val".to_string(), LOYALTY.to_string())]);
    let mut pkg =
        build(manifest(), sources, text(), &registries(), Some(&keygen())).expect("builds");
    pkg.public_key = Some(keygen().verifying_key().to_bytes().to_vec());

    assert!(matches!(install_with(&pkg, &Wallet), Err(Refusal::Unsigned(_))));
}
