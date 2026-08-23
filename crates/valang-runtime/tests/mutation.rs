//! `let` and writing to it.
//!
//! A `const` is a definition: what the name means does not depend on how far
//! down the block a reader has got. A `let` is a variable, and exists because
//! most people arrive already knowing what one is.

use std::collections::BTreeMap;

use valang::capability::{Host, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;
use valang_runtime::value::Value;

const CORE: &str = include_str!("../../../hosts/core.json");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn drawn(points: i64, decls: &str, compute: &str) -> String {
    let src = format!(
        r#"
app "x.y"
version "1.0.0"

capabilities {{
}}

state {{
  points: int default 0
}}

{decls}

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
    let mut state = BTreeMap::new();
    state.insert("points".to_string(), Value::Int(points));
    let screen = render(&program, "Home", &state, &host).expect("Home resolves");

    screen.tree[0].children[0]
        .args
        .values()
        .find_map(|v| match v {
            Value::Str(s) => Some(s.clone()),
            Value::Int(i) => Some(i.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

const LABEL: &str = r#"
function label(points: int): string {
  let out = "bronze"
  if (points >= 100) {
    out = "silver"
  }
  if (points >= 1000) {
    out = "gold"
  }
  return out
}
"#;

#[test]
fn a_branch_writes_the_name_the_block_declared() {
    assert_eq!(drawn(50, LABEL, "    const out = label(state.points)"), "bronze");
    assert_eq!(drawn(500, LABEL, "    const out = label(state.points)"), "silver");
    assert_eq!(drawn(5000, LABEL, "    const out = label(state.points)"), "gold");
}

/// The trap this shape has in every language that gets it wrong: an assignment
/// inside a branch that quietly makes a second name, so the value outside the
/// branch is the old one.
#[test]
fn a_write_inside_a_branch_is_the_same_name_outside_it() {
    let out = drawn(
        1,
        "",
        "    let n = 1\n    if (state.points > 0) {\n      n = 2\n    }\n    const out = n",
    );
    assert_eq!(out, "2");
}

#[test]
fn a_name_written_twice_keeps_the_last_value() {
    assert_eq!(drawn(0, "", "    let n = 1\n    n = 2\n    n = 3\n    const out = n"), "3");
}
