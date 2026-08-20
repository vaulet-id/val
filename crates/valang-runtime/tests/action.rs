//! How an action runs.
//!
//! The phases are one step: what `update` reads, what a lambda's row is, and
//! what a record is when the same run happens twice.

use std::collections::BTreeMap;

use valang::capability::{Host, Hosts};
use valang_runtime::value::Value;

const CORE: &str = include_str!("../../../hosts/core.json");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn program(body: &str) -> String {
    format!(
        "app \"x.y\"\nversion 1\n\ncapabilities {{\n}}\n\nstate {{\n  n: int default 0\n}}\n\n{body}\n\n@main\nscreen Home {{\n  column {{\n    section(\"x\")\n  }}\n}}\n"
    )
}

/// A lambda parameter that shadows a binding, read after the lambda.
#[test]
fn a_lambda_parameter_is_the_lambdas() {
    let src = program(
        "action Go {\n  compute {\n    const r = 7\n    const doubled = [1, 2].map { r -> r * 2 }\n    const after = r\n  }\n\n  update {\n    n: after\n  }\n}",
    );
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let (program, d) = valang::analyse_fully(&src, None, &hosts);
    assert!(
        d.iter().all(|x| x.severity != valang::diag::Severity::Error),
        "{:?}",
        d.iter().map(|x| &x.message).collect::<Vec<_>>()
    );

    // `after` must be 7, not the row.
    let host = valang_runtime::fixture::Fixture::parse(WALLET).unwrap();
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());
    let run = valang_runtime::run_action(&program, &src, "Go", &state, &BTreeMap::new(), &host);
    let after = run.next_state.get("n").cloned();
    assert_eq!(after, Some(Value::Int(7)), "the lambda's row escaped it");
}


/// Two runs of one action produce the same bytes.
#[test]
fn the_same_run_is_the_same_record() {
    let src = program("action Go {\n  update {\n    n: state.n + 1\n  }\n}");
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let (program, _) = valang::analyse_fully(&src, None, &hosts);
    let host = valang_runtime::fixture::Fixture::parse(WALLET).unwrap();
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());

    let a = valang_runtime::run_action(&program, &src, "Go", &state, &BTreeMap::new(), &host);
    let b = valang_runtime::run_action(&program, &src, "Go", &state, &BTreeMap::new(), &host);
    assert_eq!(
        valang_runtime::encode_record(&a.record),
        valang_runtime::encode_record(&b.record),
        "one run twice gave two records"
    );
}

/// A binding may take a state field's name. `state.n` is always written out, so
/// a bare `n` is never the state — and `update { n: n }` writes the binding.
#[test]
fn a_binding_that_shares_a_name_with_state_is_still_the_binding() {
    let src = program(
        "action Go {\n  compute {\n    const n = 5\n  }\n\n  update {\n    n: n\n  }\n}",
    );
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let (compiled, d) = valang::analyse_fully(&src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = valang_runtime::fixture::Fixture::parse(WALLET).unwrap();
    let state = valang_runtime::initial_state(&compiled, &BTreeMap::new());
    let run = valang_runtime::run_action(&compiled, &src, "Go", &state, &BTreeMap::new(), &host);
    assert_eq!(run.next_state.get("n"), Some(&Value::Int(5)));
}

/// `update` is a patch, not a sequence of assignments: every line reads the
/// state the action started with. A swap is the case that tells the two apart —
/// read as assignments, both fields end up holding the same value.
#[test]
fn an_update_is_one_patch_and_not_a_sequence() {
    let src = "app \"x.y\"\nversion 1\n\ncapabilities {\n}\n\nstate {\n  a: int default 1\n  b: int default 2\n}\n\naction Swap {\n  update {\n    a: state.b\n    b: state.a\n  }\n}\n\n@main\nscreen Home {\n  column {\n    button(\"go\") { onTap: Swap }\n  }\n}\n";
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let (compiled, d) = valang::analyse_fully(src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = valang_runtime::fixture::Fixture::parse(WALLET).unwrap();
    let mut state = BTreeMap::new();
    state.insert("a".to_string(), Value::Int(1));
    state.insert("b".to_string(), Value::Int(2));
    let run = valang_runtime::run_action(&compiled, src, "Swap", &state, &BTreeMap::new(), &host);
    assert_eq!(run.next_state.get("a"), Some(&Value::Int(2)), "a took the old b");
    assert_eq!(run.next_state.get("b"), Some(&Value::Int(1)), "b took the old a");
}

/// A state that differs anywhere has a different root, and the same state has
/// the same one however its map was built.
#[test]
fn the_state_root_is_of_the_state_and_nothing_else() {
    let src = program("action Go {\n  update {\n    n: state.n + 1\n  }\n}");
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let (compiled, _) = valang::analyse_fully(&src, None, &hosts);
    let host = valang_runtime::fixture::Fixture::parse(WALLET).unwrap();

    let root_of = |n: i64| {
        let mut state = BTreeMap::new();
        state.insert("n".to_string(), Value::Int(n));
        let run =
            valang_runtime::run_action(&compiled, &src, "Go", &state, &BTreeMap::new(), &host);
        run.record.previous_root
    };

    assert_ne!(root_of(1), root_of(2), "two different states shared a root");
    assert_eq!(root_of(7), root_of(7), "one state gave two roots");
}

/// Totality bounds how many steps a program takes and says nothing about how
/// large a value becomes. The host carries the second bound, and an action that
/// would put more in state than it carries does not commit.
#[test]
fn a_state_larger_than_the_host_carries_does_not_commit() {
    let src = "app \"x.y\"\nversion 1\n\ncapabilities {\n}\n\nstate {\n  rows: List<int> default []\n}\n\naction Fill {\n  compute {\n    const many = (1...9000).map { r -> r }\n  }\n\n  update {\n    rows: many\n  }\n}\n\n@main\nscreen Home {\n  column {\n    button(\"go\") { onTap: Fill }\n  }\n}\n";
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let (compiled, d) = valang::analyse_fully(src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = valang_runtime::fixture::Fixture::parse(WALLET).unwrap();
    let mut state = BTreeMap::new();
    state.insert("rows".to_string(), Value::List(Vec::new()));
    let run = valang_runtime::run_action(&compiled, src, "Fill", &state, &BTreeMap::new(), &host);
    assert!(
        !matches!(run.outcome, valang_runtime::Outcome::Committed),
        "a state of nine thousand rows committed against a host carrying four thousand: {:?}",
        run.outcome
    );
}

/// An action commits or it does not. A trap partway through the patch must not
/// leave the lines before it applied.
#[test]
fn a_trap_partway_through_an_update_leaves_no_half_state() {
    let src = "app \"x.y\"\nversion 1\n\ncapabilities {\n}\n\nstate {\n  a: int default 1\n  b: int default 1\n}\n\naction Go {\n  update {\n    a: 7\n    b: state.b * 9223372036854775807 * 2\n  }\n}\n\n@main\nscreen Home {\n  column {\n    button(\"go\") { onTap: Go }\n  }\n}\n";
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let (compiled, d) = valang::analyse_fully(src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = valang_runtime::fixture::Fixture::parse(WALLET).unwrap();
    let mut state = BTreeMap::new();
    state.insert("a".to_string(), Value::Int(1));
    state.insert("b".to_string(), Value::Int(1));
    let run = valang_runtime::run_action(&compiled, src, "Go", &state, &BTreeMap::new(), &host);

    assert!(!matches!(run.outcome, valang_runtime::Outcome::Committed), "{:?}", run.outcome);
    assert_eq!(
        run.next_state.get("a"),
        Some(&Value::Int(1)),
        "the line before the trap was applied and the action did not commit"
    );
}

/// The batch is offered once and the person may say no. Nothing commits then —
/// and "nothing" has to include the state the pure phases worked out, or a
/// refusal would leave behind exactly the change it refused.
#[test]
fn a_batch_the_person_refuses_commits_no_state() {
    let src = "app \"x.y\"\nversion 1\n\ncapabilities {\n  credential.issue(Card)\n}\n\ncredential Card {\n  who: string\n}\n\nstate {\n  n: int default 1\n}\n\naction Go {\n  update {\n    n: 9\n  }\n\n  execute {\n    credential.issue(Card { who: \"me\" })\n  }\n}\n\n@main\nscreen Home {\n  column {\n    button(\"go\") { onTap: Go }\n  }\n}\n";
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let (compiled, d) = valang::analyse_fully(src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let mut host = valang_runtime::fixture::Fixture::parse(WALLET).unwrap();
    host.approve = false;

    let mut state = BTreeMap::new();
    state.insert("n".to_string(), Value::Int(1));
    let run = valang_runtime::run_action(&compiled, src, "Go", &state, &BTreeMap::new(), &host);

    assert!(!matches!(run.outcome, valang_runtime::Outcome::Committed), "{:?}", run.outcome);
    assert_eq!(
        run.next_state.get("n"),
        Some(&Value::Int(1)),
        "the person refused the batch and the state changed anyway"
    );
}
