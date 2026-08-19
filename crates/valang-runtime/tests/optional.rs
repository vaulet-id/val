//! `a?.b` and `a ?: b`.
//!
//! The check somebody forgets is the one that matters, so it is written as
//! punctuation rather than as a condition the author repeats.

use std::collections::BTreeMap;

use valang::capability::{Host, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;
use valang_runtime::value::Value;

const CORE: &str = include_str!("../../../hosts/core.json");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn drawn(compute: &str) -> Vec<String> {
    let src = format!(
        r#"
app "x.y"
version 1

capabilities {{
}}

state {{
  n: int default 0
}}

@main
screen Home {{
  compute {{
{compute}
  }}

  column {{
    text(out)
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
    for node in &screen.tree {
        for child in &node.children {
            for v in child.args.values() {
                out.push(match v {
                    Value::Str(s) => s.clone(),
                    Value::Null => "null".to_string(),
                    other => format!("{other:?}"),
                });
            }
        }
    }
    out
}

#[test]
fn a_path_that_is_there_reads_through() {
    assert_eq!(drawn("    const out = { a: { b: \"x\" } }.a?.b"), vec!["x"]);
}

/// The point of it: the whole path is nothing, rather than a failure partway
/// down it. Asked through `exists`, because a null drawn on a screen is a word
/// the host looks up rather than a value — which would hide the difference
/// between nothing and a trap.
#[test]
fn a_path_that_stops_is_nothing_rather_than_a_failure() {
    assert_eq!(
        drawn("    const out = { a: null }.a?.b exists ? \"there\" : \"nothing\""),
        vec!["nothing"]
    );
}

/// And a path that is not optional still fails, because somebody wrote it
/// believing what it reaches for is there.
#[test]
fn a_plain_path_through_nothing_still_fails() {
    let src = r#"
app "x.y"
version 1

capabilities {
}

state {
  n: int default 0
}

@main
screen Home {
  compute {
    const out = { a: null }.a.b
  }

  column {
    text(out)
  }
}
"#;
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let (program, d) = valang::analyse_fully(src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");
    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());
    assert!(render(&program, "Home", &state, &host).is_err(), "a path through nothing succeeded");
}

#[test]
fn elvis_takes_the_left_side_when_it_is_there() {
    assert_eq!(drawn("    const out = \"here\" ?: \"none\""), vec!["here"]);
}

#[test]
fn elvis_takes_the_right_side_when_it_is_not() {
    assert_eq!(drawn("    const out = { a: null }.a?.b ?: \"none\""), vec!["none"]);
}
