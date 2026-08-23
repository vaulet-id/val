//! `if` in a screen's tree.
//!
//! The branch is chosen while the screen is being resolved, so what a host
//! receives is a tree with no condition left in it — a host that never
//! implements `if` still draws these screens correctly, which is the whole
//! reason the choice is made here.

use std::collections::BTreeMap;

use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;

const WALLET: &str = include_str!("../../../fixtures/wallet.json");

const SRC: &str = r#"
app "example.branches"
version "1.0.0"

capabilities {
}

state {
  points: int default 0
}

action Earn {
  update {
    points: state.points + 10
  }
}

@main

screen Home {
  column {
    if (state.points > 0) {
      card("A member")
    } else {
      card("Not a member")
      button("Join") { onTap: Earn }
    }
    button("Earn") { onTap: Earn }
  }
}
"#;

/// The tree the renderer produced, as `kind(text)` per line, so a test reads
/// like the screen it is about.
fn shape(c: &valang_runtime::render::Component, out: &mut Vec<String>) {
    let text = match c.args.values().find_map(|v| match v {
        valang_runtime::value::Value::Str(s) => Some(s.clone()),
        _ => None,
    }) {
        Some(s) => s,
        None => String::new(),
    };
    out.push(if text.is_empty() { c.kind.clone() } else { format!("{}({text})", c.kind) });
    for child in &c.children {
        shape(child, out);
    }
}

fn drawn(points: i64) -> Vec<String> {
    let (program, d) = valang::analyse(SRC);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");
    let host = Fixture::parse(WALLET).expect("the wallet parses");

    let mut state = BTreeMap::new();
    state.insert("points".to_string(), valang_runtime::value::Value::Int(points));

    let screen = render(&program, "Home", &state, &host).expect("Home resolves");
    let mut out = Vec::new();
    for node in &screen.tree {
        shape(node, &mut out);
    }
    out
}

#[test]
fn the_branch_that_is_taken_is_the_one_drawn() {
    assert_eq!(
        drawn(10),
        vec!["column", "card(A member)", "button(Earn)"],
    );
}

#[test]
fn the_other_branch_may_hold_more_than_one_node() {
    assert_eq!(
        drawn(0),
        vec!["column", "card(Not a member)", "button(Join)", "button(Earn)"],
    );
}

/// A host is never told a condition existed. If `if` reached a renderer, every
/// host would have to implement it — and the ones that did not would draw an
/// empty box where a card belongs.
#[test]
fn no_condition_survives_into_the_tree() {
    for points in [0, 10] {
        assert!(!drawn(points).iter().any(|k| k.starts_with("if")), "an `if` reached the host");
    }
}
