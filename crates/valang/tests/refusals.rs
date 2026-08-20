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

