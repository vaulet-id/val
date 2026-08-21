//! One action, two engines, and everything about the run compared.
//!
//! The tree-walking evaluator is what a build with a compiler in it uses; the
//! module is what a phone uses, because a phone has no compiler. They are two
//! implementations of one language, so the only thing worth asserting is that
//! they agree — and about the whole of a run, not about the answer: the state
//! that came out, what the host was asked to do, the roots, and the outcome.

use std::collections::BTreeMap;

use valang::capability::{Host as Registry, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::{run_action, run_action_with, Run};

const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn registries() -> Hosts {
    Hosts::of(vec![Registry::parse(include_str!("../../../hosts/core.json")).expect("core parses")])
}

/// Both runs of one action: the walk, and the module.
fn both(src: &str, action: &str) -> (Run, Run) {
    let (program, diagnostics) = valang::analyse_fully(src, None, &registries());
    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    assert!(errors.is_empty(), "the example does not compile: {errors:?}");

    let module = valang_wasm::compile::compile_program(&program)
        .unwrap_or_else(|missing| panic!("not emitted: {missing:?}"));

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &host.state());
    let input = BTreeMap::new();

    let walked = run_action(&program, src, action, &state, &input, &host);
    let mut engine = valang_wasm::WasmEngine::new(&module);
    let ran = run_action_with(&program, src, action, &state, &input, &host, &mut engine);
    (walked, ran)
}

fn agree(walked: &Run, ran: &Run, about: &str) {
    assert_eq!(format!("{:?}", ran.outcome), format!("{:?}", walked.outcome), "the outcome of {about}");
    assert_eq!(ran.next_state, walked.next_state, "the state {about} produced");
    assert_eq!(ran.record.next_root, walked.record.next_root, "the root after {about}");
    assert_eq!(ran.effects.len(), walked.effects.len(), "how much {about} asked for");
    for (a, b) in ran.effects.iter().zip(&walked.effects) {
        assert_eq!(a.capability, b.capability, "which capability {about} asked for");
        assert_eq!(a.payload, b.payload, "what {about} asked to have done");
        assert_eq!(a.reversible, b.reversible, "whether it can be taken back");
    }
}

/// The example the specification is written around: a receipt read, points
/// computed, three fields written, a membership issued.
#[test]
fn the_two_engines_agree_about_earning_points() {
    let (walked, ran) = both(include_str!("../../../examples/loyalty.val"), "ScanToEarn");
    agree(&walked, &ran, "ScanToEarn");
    assert!(
        matches!(ran.outcome, valang_runtime::Outcome::Committed),
        "the module's run did not commit: {:?}",
        ran.outcome
    );
}

/// The one where an effect goes out. `present { disclose …; prove … }` is one
/// request carrying both lines — which is why the module builds the parts and
/// hands them over together rather than sending each as it goes.
#[test]
fn the_two_engines_agree_about_a_disclosure() {
    let (walked, ran) = both(include_str!("../../../examples/door.val"), "EnterVenue");
    agree(&walked, &ran, "EnterVenue");
    assert_eq!(ran.effects.len(), 1, "one `present` is one request: {:?}", ran.effects);
}
