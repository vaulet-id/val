//! `for (x in xs)` and the ranges it runs over.
//!
//! Repetition without the wallet's list around it: `list(rows) { … }` draws the
//! wallet's separators and its empty state, and until this existed that was the
//! only way to say a thing twice.

use std::collections::BTreeMap;

use valang::capability::{Host, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;

const CORE: &str = include_str!("../../../hosts/core.json");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn drawn(body: &str) -> Vec<String> {
    let src = format!(
        r#"
app "x.y"
version 1

capabilities {{
}}

state {{
  n: int default 3
}}

@main
screen Home {{
  column {{
{body}
  }}
}}
"#
    );
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let (program, d) = valang::analyse_fully(&src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());
    let screen = render(&program, "Home", &state, &host).expect("Home resolves");

    let mut out = Vec::new();
    fn walk(c: &valang_runtime::render::Component, out: &mut Vec<String>) {
        let text = c.args.values().find_map(|v| match v {
            valang_runtime::value::Value::Str(s) => Some(s.clone()),
            valang_runtime::value::Value::Int(i) => Some(i.to_string()),
            _ => None,
        });
        out.push(match text {
            Some(t) => format!("{}({t})", c.kind),
            None => c.kind.clone(),
        });
        for child in &c.children {
            walk(child, out);
        }
    }
    for node in &screen.tree {
        walk(node, &mut out);
    }
    out
}

/// Both ends included, as the punctuation says in every language that spells a
/// range this way.
#[test]
fn a_range_includes_both_ends() {
    assert_eq!(drawn("    for (i in 1...3) {\n      text(i)\n    }"),
        vec!["column", "text(1)", "text(2)", "text(3)"]);
}

#[test]
fn a_range_that_runs_backwards_is_empty() {
    assert_eq!(drawn("    for (i in 3...1) {\n      text(i)\n    }"), vec!["column"]);
}

/// Spliced where the loop was written, so what a host receives is a tree with
/// no loop in it — and a host needs no loop of its own.
#[test]
fn nothing_is_drawn_around_the_body() {
    let out = drawn("    for (i in 1...2) {\n      text(i)\n      section(i)\n    }");
    assert_eq!(out, vec!["column", "text(1)", "section(1)", "text(2)", "section(2)"]);
    assert!(!out.iter().any(|k| k.starts_with("for")), "a loop reached the host");
}

#[test]
fn a_range_reads_the_state_it_runs_to() {
    assert_eq!(drawn("    for (i in 1...state.n) {\n      text(i)\n    }"),
        vec!["column", "text(1)", "text(2)", "text(3)"]);
}
