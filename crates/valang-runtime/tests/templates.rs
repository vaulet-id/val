//! `` `you have ${state.points} points` ``
//!
//! Sugar for `phrase`, and deliberately nothing more: the words and the values
//! reach the host separately, so the host formats the number — Thai digits, the
//! thousands separator, the currency position — for every application at once.
//! A template that joined them here would format them in the application,
//! differently in each one.

use std::collections::BTreeMap;

use valang::capability::{Host, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;
use valang_runtime::value::Value;

const CORE: &str = include_str!("../../../hosts/core.json");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

/// Every argument of the first drawn node, so a test can see the words and the
/// values as the host receives them.
fn args(body: &str) -> BTreeMap<String, String> {
    let src = format!(
        r#"
app "x.y"
version "1.0.0"

capabilities {{
}}

state {{
  points: int default 1240
  name: string default "Mark"
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

    screen.tree[0].children[0]
        .args
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                match v {
                    Value::Str(s) => s.clone(),
                    Value::Int(i) => i.to_string(),
                    other => format!("{other:?}"),
                },
            )
        })
        .collect()
}

#[test]
fn the_words_keep_the_slot_and_the_value_travels_beside_it() {
    let a = args("    text(`you have ${state.points} points`)");
    assert_eq!(a.get("text").map(String::as_str), Some("you have {points} points"));
    assert_eq!(a.get("points").map(String::as_str), Some("1240"));
}

/// The last segment of the path, because a bundle for a second language is read
/// by somebody who was not here.
#[test]
fn a_slot_is_named_after_what_it_holds() {
    let a = args("    card(`hello ${state.name}, ${state.points} points`)");
    assert_eq!(a.get("text").map(String::as_str), Some("hello {name}, {points} points"));
    assert_eq!(a.get("name").map(String::as_str), Some("Mark"));
}

/// Two slots with one name would be one slot.
#[test]
fn a_name_used_twice_falls_back_to_its_position() {
    let a = args("    card(`${state.points} of ${state.points}`)");
    assert_eq!(a.get("text").map(String::as_str), Some("{points} of {v1}"));
    assert_eq!(a.get("v1").map(String::as_str), Some("1240"));
}

#[test]
fn an_expression_means_in_a_string_what_it_means_outside_one() {
    let a = args("    text(`total ${state.points + 10}`)");
    assert_eq!(a.get("text").map(String::as_str), Some("total {v0}"));
    assert_eq!(a.get("v0").map(String::as_str), Some("1250"));
}

#[test]
fn a_template_with_no_slots_is_just_words() {
    let a = args("    text(`nothing to fill`)");
    assert_eq!(a.get("text").map(String::as_str), Some("nothing to fill"));
    assert_eq!(a.len(), 1, "{a:?}");
}

/// A brace inside a string is not a brace. `${ f("}") }` ended the
/// interpolation at the wrong place, and the rest of the template was read as
/// source until the file ran out.
#[test]
fn a_brace_inside_a_string_does_not_close_the_interpolation() {
    let a = args("    text(`a ${ \"}\" } b`)");
    assert_eq!(a.get("text").map(String::as_str), Some("a {v0} b"));
    assert_eq!(a.get("v0").map(String::as_str), Some("}"));
}
