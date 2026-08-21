//! A wallet holds the bytes, and nothing else.
//!
//! No source, no manifest, no compiler. This is the whole of what a phone does
//! with a Micro App somebody installed: read what it can do, show that to the
//! person, run it, and sign what happened. Every step here takes `&[u8]`, which
//! is the point — a step that needed a `Program` would be a step that needed a
//! front end.

use std::collections::BTreeMap;

use valang::capability::{Host as Registry, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::run_action_with;

const WALLET: &str = include_str!("../../../fixtures/wallet.json");

/// The bytes a publisher would ship. Built here because there is nowhere else
/// to get them from in a test; everything after this line pretends not to know
/// where they came from.
fn shipped(src: &str) -> Vec<u8> {
    let hosts =
        Hosts::of(vec![Registry::parse(include_str!("../../../hosts/core.json")).expect("core")]);
    let (program, _) = valang::analyse_fully(src, None, &hosts);
    valang_wasm::compile::compile_program(&program).expect("emits").bytes
}

#[test]
fn a_wallet_runs_what_it_was_handed() {
    let bytes = shipped(include_str!("../../../examples/loyalty.val"));

    // 1. What it can do, read off the import section.
    let wants = valang_wasm::wants_of(&bytes).expect("this host can describe it");
    assert!(wants.issues.contains("LoyaltyMember"), "{wants:?}");
    assert_eq!(wants.writes.len(), 3, "{wants:?}");

    // 2. Which application it is, read off the module's own metadata.
    let about = valang_wasm::compile::about_of(&bytes).expect("it says what it is");
    assert_eq!(about.app, "th.co.codefin.loyalty");
    assert_eq!(about.version, "1");
    let action = about.action("ScanToEarn").expect("it carries the action");
    assert_eq!(action.inputs.len(), 1, "one credential is asked for");
    assert_eq!(action.inputs[0].credential, "PurchaseReceipt");
    assert_eq!(action.inputs[0].policy.as_deref(), Some("ReceiptFromMerchant"));

    // 3. Run it. The record names the module, because the module is what ran.
    let module = valang_wasm::compile::Module {
        bytes: bytes.clone(),
        konsts: valang_wasm::konsts_of(&bytes).expect("its constants travel with it"),
        functions: Vec::new(),
    };
    let host = Fixture::parse(WALLET).expect("the wallet parses");
    // What this wallet holds, over what the application declared — read off the
    // module, because there is no program here to read a `default` from.
    let state = about.initial(&host.state());
    let code_hash: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(&bytes).into();
    let mut engine = valang_wasm::WasmEngine::new(&module);
    let run = run_action_with(
        &about,
        code_hash,
        "ScanToEarn",
        &state,
        &BTreeMap::new(),
        &host,
        &mut engine,
    );

    assert!(
        matches!(run.outcome, valang_runtime::Outcome::Committed),
        "the wallet's run did not commit: {:?}",
        run.outcome
    );
    assert_eq!(run.record.code_hash, code_hash, "the record names something else");
    assert_eq!(run.record.app, "th.co.codefin.loyalty");
    assert_eq!(run.effects.len(), 1, "one credential issued: {:?}", run.effects);
    assert!(!run.record.signature.is_empty(), "nothing signed it");
}

/// Bytes that are not a module, and bytes that are a module of somebody else's
/// language. Neither is run, and neither is described.
#[test]
fn bytes_that_are_not_this_are_refused() {
    assert!(valang_wasm::wants_of(b"not a module at all").is_err());
    assert!(valang_wasm::compile::about_of(b"not a module at all").is_none());
}

/// **Where it opens.** A wallet has no program to read `@main` off, and a host
/// with a screen's name written into it is a host that works for one
/// application.
#[test]
fn a_module_says_which_screen_it_opens_at() {
    let bytes = shipped(include_str!("../../../examples/portfolio.val"));
    let about = valang_wasm::compile::about_of(&bytes).expect("it says what it is");
    assert_eq!(about.opens, "Portfolio");
    assert!(about.screens.contains(&"Portfolio".to_string()), "{:?}", about.screens);
}
