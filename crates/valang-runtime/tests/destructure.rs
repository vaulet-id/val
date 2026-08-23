//! `const { merchant, amount } = row`
//!
//! One statement rather than one binding per field, so the right-hand side is
//! read once — a record here can be a credential the host had to be asked for.

use std::collections::BTreeMap;

use valang::capability::{Host, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;
use valang_runtime::value::Value;

const CORE: &str = include_str!("../../../hosts/core.json");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn drawn(compute: &str, draws: &str) -> Vec<String> {
    let src = format!(
        r#"
app "x.y"
version "1.0.0"

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
{draws}
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
    for child in &screen.tree[0].children {
        for v in child.args.values() {
            out.push(match v {
                Value::Str(s) => s.clone(),
                Value::Int(i) => i.to_string(),
                other => format!("{other:?}"),
            });
        }
    }
    out
}

#[test]
fn the_fields_come_out_under_their_own_names() {
    assert_eq!(
        drawn(
            "    const { merchant, amount } = { merchant: \"Codefin\", amount: 120 }",
            "    text(merchant)\n    section(amount)"
        ),
        vec!["Codefin", "120"]
    );
}

#[test]
fn a_field_the_record_does_not_have_is_nothing() {
    assert_eq!(
        drawn(
            "    const { missing } = { merchant: \"Codefin\" }\n    const out = missing ?: \"none\"",
            "    text(out)"
        ),
        vec!["none"]
    );
}

/// A parameter with a default is one the call site may leave out, and what it
/// then means is written once where the component is rather than once per call.
#[test]
fn a_parameter_left_out_takes_its_default() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

state {
  n: int default 0
}

component Badge(label: string, tone: string default "neutral") {
  card {
    text: label
    section(tone)
  }
}

@main
screen Home {
  column {
    Badge(label: "one")
    Badge(label: "two", tone: "loud")
  }
}
"#;
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let (program, d) = valang::analyse_fully(src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());
    let screen = render(&program, "Home", &state, &host).expect("Home resolves");

    let mut out = Vec::new();
    for card in &screen.tree[0].children {
        for child in &card.children {
            for v in child.args.values() {
                if let Value::Str(s) = v {
                    out.push(s.clone());
                }
            }
        }
    }
    assert_eq!(out, vec!["neutral", "loud"]);
}
