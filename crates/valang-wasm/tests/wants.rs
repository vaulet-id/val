//! What the module says it can do, against what the source says it does.
//!
//! These two are computed apart, for now: one walks the typed AST, the other
//! reads an import section out of compiled bytes. They are here to be compared
//! while that is still true — the plan is that the module becomes the only
//! answer and the walk goes away, and this is what says the module is ready to
//! be it.

use valang::capability::{Host, Hosts};

fn registries() -> Hosts {
    Hosts::of(vec![Host::parse(include_str!("../../../hosts/core.json")).expect("core parses")])
}

fn compiled(src: &str) -> (valang::ast::Program, Vec<u8>) {
    let (program, diagnostics) = valang::analyse_fully(src, None, &registries());
    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    assert!(errors.is_empty(), "the example does not compile: {errors:?}");

    let module = valang_wasm::compile::compile_program(&program)
        .unwrap_or_else(|missing| panic!("this back end does not emit: {missing:?}"));
    (program, module.bytes)
}

/// The one that matters, on the example the specification is written around.
#[test]
fn the_loyalty_example_says_the_same_thing_twice() {
    let (program, bytes) = compiled(include_str!("../../../examples/loyalty.val"));
    let wants = valang_wasm::wants_of(&bytes).expect("a module this host can describe");
    let said = valang::report::report(&program);

    assert_eq!(wants.writes, said.writes, "what it writes");
    assert_eq!(wants.issues, said.issues, "what it issues");
    assert_eq!(wants.discloses, said.discloses, "what it discloses");
    assert_eq!(wants.proves, said.proves, "what it proves");
    assert_eq!(wants.reads_as_lines(), said.reads, "what it reads");
}

/// And the one where they do not, which is the reason for doing this.
///
/// `door.val` proves somebody is over twenty without disclosing when they were
/// born. The walk over the source counts that as reading the birthdate, because
/// the claim is written in the predicate — so the sheet a person would have
/// been shown said "reads your birthdate" about an application that provably
/// cannot.
///
/// The module cannot say that. `prove` takes nothing: the statement is
/// evaluated by the host, which is the only party that can build the proof, so
/// there is no import for the birthdate and no way to reach it. **The module is
/// right and the walk is wrong**, and this is what the second route was for.
#[test]
fn proving_a_claim_is_not_reading_it() {
    let (program, bytes) = compiled(include_str!("../../../examples/door.val"));
    let wants = valang_wasm::wants_of(&bytes).expect("a module this host can describe");
    let said = valang::report::report(&program);

    assert_eq!(wants.discloses, said.discloses, "what it discloses");
    assert_eq!(wants.proves, said.proves, "what it proves");

    assert_eq!(
        wants.reads_as_lines(),
        ["NationalId.country under GovernmentIssued".to_string()].into_iter().collect(),
        "the module reads the country it discloses, and nothing else"
    );
    assert!(
        said.reads.iter().any(|r| r.contains("birthdate")),
        "the walk over the source has stopped claiming the birthdate is read — \
         delete this test with it"
    );
}
