//! What a run starts from.
//!
//! The expected values are read off the source below, which is what a person
//! reading `default 0` would predict. They are not taken from what the
//! evaluator produces for it.

use std::collections::BTreeMap;

use valang_runtime::{initial_state, value::Value, State};

const SRC: &str = r#"
app "example.state"
version "1.0.0"

capabilities {
}

state {
  taps: int default 0
  label: text default "unnamed"
  awake: bool default true
  note: text?
  seen: int
}

screen Home {
  column {
  }
}
"#;

fn program() -> valang::ast::Program {
    let (p, d) = valang::analyse(SRC);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");
    p
}

#[test]
fn a_fresh_install_starts_at_the_declared_defaults() {
    let s = initial_state(&program(), &State::new());

    assert_eq!(s.get("taps"), Some(&Value::Int(0)));
    assert_eq!(s.get("label"), Some(&Value::Str("unnamed".into())));
    assert_eq!(s.get("awake"), Some(&Value::Bool(true)));
}

/// A field with no default and no value is null rather than absent, so reading
/// it is an ordinary null rather than an unknown name.
#[test]
fn a_field_with_no_default_is_null() {
    let s = initial_state(&program(), &State::new());
    assert_eq!(s.get("seen"), Some(&Value::Null));
}

/// An optional field the wallet has never held is not there at all — which is
/// what `?` says.
#[test]
fn an_optional_field_is_left_out() {
    let s = initial_state(&program(), &State::new());
    assert_eq!(s.get("note"), None);
}

#[test]
fn what_the_wallet_holds_wins_over_the_default() {
    let mut held = State::new();
    held.insert("taps".into(), Value::Int(7));
    let s = initial_state(&program(), &held);
    assert_eq!(s.get("taps"), Some(&Value::Int(7)));
}

/// The wallet may be holding another application's fields — it holds one map
/// per install, and a playground holds one map for whatever is open. Fields
/// this program never declared are not its state.
#[test]
fn fields_this_program_never_declared_are_dropped() {
    let mut held = State::new();
    held.insert("points".into(), Value::Int(120));
    let s: BTreeMap<String, Value> = initial_state(&program(), &held);
    assert_eq!(s.get("points"), None);
}
