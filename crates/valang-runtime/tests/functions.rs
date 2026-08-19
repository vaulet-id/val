//! A function passed by name.
//!
//! `receipts.map(double)` used to compile, run, and return the list unchanged:
//! the combinator looked for a lambda written in place and a name matched
//! nothing, so the mistake was silent in both directions.

use std::collections::BTreeMap;

use valang::capability::{Host, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;
use valang_runtime::value::Value;

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
  n: int default 0
}}

function double(x: int): int {{
  return x * 2
}}

function add(a: int, b: int): int {{
  return a + b
}}

@main
screen Home {{
  compute {{
{body}
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
                    Value::Int(i) => i.to_string(),
                    Value::List(items) => items
                        .iter()
                        .map(|i| match i {
                            Value::Int(n) => n.to_string(),
                            other => format!("{other:?}"),
                        })
                        .collect::<Vec<_>>()
                        .join(","),
                    other => format!("{other:?}"),
                });
            }
        }
    }
    out
}

#[test]
fn map_takes_a_function_by_name() {
    assert_eq!(drawn("    const out = [1, 2, 3].map(double)"), vec!["2,4,6"]);
}

#[test]
fn fold_takes_a_function_by_name() {
    assert_eq!(drawn("    const out = [1, 2, 3].fold(0, add)"), vec!["6"]);
}

/// And the written form still works, because it is what every example uses.
#[test]
fn a_function_written_in_place_still_works() {
    assert_eq!(drawn("    const out = [1, 2, 3].map { r -> r * 2 }"), vec!["2,4,6"]);
}
