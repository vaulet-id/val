//! Shapes the compiler has to refuse.

use valang::capability::{Host, Hosts};

const CORE: &str = include_str!("../../../hosts/core.json");

fn errors(src: &str) -> Vec<String> {
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    valang::analyse_fully(src, None, &hosts)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::diag::Severity::Error)
        .map(|d| d.message)
        .collect()
}

fn program(body: &str) -> String {
    format!(
        "app \"x.y\"\nversion 1\n\ncapabilities {{\n}}\n\nstate {{\n  n: int default 0\n}}\n\n{body}\n\n@main\nscreen Home {{\n  column {{\n    section(\"x\")\n  }}\n}}\n"
    )
}

/// Two functions that call each other. One that calls itself is refused; a
/// circle of two is the same program with a step in it.
#[test]
fn mutual_recursion_is_recursion() {
    let e = errors(&program(
        "function even(n: int): bool {\n  return n == 0 ? true : odd(n - 1)\n}\n\nfunction odd(n: int): bool {\n  return n == 0 ? false : even(n - 1)\n}",
    ));
    assert!(!e.is_empty(), "two functions calling each other built clean");
}

/// A patch to a path the state does not have.
#[test]
fn a_patch_names_a_field_that_exists() {
    let e = errors(&program(
        "action Go {\n  update {\n    nope: 1\n  }\n}",
    ));
    assert!(e.iter().any(|m| m.contains("nope")), "a patch named a field nothing declares: {e:?}");
}


/// A rule that reads a phase's statements has to read the ones inside a branch.
/// Three of them stopped at the first `if`, so writing the mistake in a branch
/// was how to get it past them.
#[test]
fn a_branch_is_not_a_place_to_hide_a_mistake() {
    // A patch with a list index in it.
    let indexed = errors(&program(
        "action Go {\n  update {\n    if (state.n > 0) {\n      rows[3].used: true\n    }\n  }\n}",
    ));
    assert!(
        indexed.iter().any(|m| m.contains("list index")),
        "an indexed patch inside a branch: {indexed:?}"
    );

    // A record literal where a path belongs.
    let record = errors(&program(
        "action Go {\n  update {\n    if (state.n > 0) {\n      n: { a: 1 }\n    }\n  }\n}",
    ));
    assert!(
        record.iter().any(|m| m.contains("takes paths, not record literals")),
        "a record literal inside a branch: {record:?}"
    );
}
