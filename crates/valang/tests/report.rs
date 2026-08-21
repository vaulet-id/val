//! The report must not understate the application.
//!
//! The consent sheet a person approves is a rendering of it. A capability that
//! runs and does not appear is the one failure this whole design exists to
//! prevent, so the places a use can hide are worth naming.

use valang::capability::{Host, Hosts};

/// The report as a wallet derives it: from the compiled module, whose import
/// section is the whole of what it can reach. These tests are about a use
/// hiding somewhere, and a branch nothing walked into is exactly where one
/// would — so they are worth as much against the module as they were against
/// the walk that used to answer this.
fn report(p: &valang::ast::Program) -> valang::report::Report {
    valang_wasm::report_of(p).expect("the back end emits this")
}

const CORE: &str = include_str!("../../../hosts/core.json");

fn built(src: &str) -> (valang::ast::Program, Vec<String>) {
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let (program, d) = valang::analyse_fully(src, None, &hosts);
    let errors = d
        .into_iter()
        .filter(|x| x.severity == valang::diag::Severity::Error)
        .map(|x| x.message)
        .collect();
    (program, errors)
}

const BRANCHED: &str = r#"
app "x.y"
version 1

capabilities {
  credential.issue(Card)
  credential.issue(Other)
}

credential Card {
  who: string
}

credential Other {
  who: string
}

state {
  n: int default 0
}

action Go {
  update {
    n: 1
  }

  execute {
    if (state.n > 0) {
      credential.issue(Card { who: "a" })
    } else {
      credential.issue(Other { who: "b" })
    }
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Go }
  }
}
"#;

/// An effect in the branch that is not taken is still something this
/// application does. Which branch runs depends on state the report does not
/// have, and a person consents before either.
#[test]
fn an_effect_in_either_branch_is_in_the_report() {
    let (program, errors) = built(BRANCHED);
    assert!(errors.is_empty(), "{errors:?}");
    let r = report(&program);
    let issues: Vec<&String> = r.issues.iter().collect();
    assert!(issues.iter().any(|i| i.contains("Card")), "the taken branch is missing: {issues:?}");
    assert!(
        issues.iter().any(|i| i.contains("Other")),
        "the branch that is not taken is missing from the report: {issues:?}"
    );
}

/// And a capability declared but only used in a branch is not reported as
/// unused — which would push an author to remove it and then fail to build.
#[test]
fn a_capability_used_only_in_a_branch_is_used() {
    let (_, errors) = built(BRANCHED);
    assert!(
        !errors.iter().any(|m| m.contains("never used")),
        "a capability used in one branch was called unused: {errors:?}"
    );
}

/// The baseline: an effect written straight into `execute`.
#[test]
fn an_effect_not_in_a_branch_is_in_the_report() {
    let src = BRANCHED.replace(
        "    if (state.n > 0) {\n      credential.issue(Card { who: \"a\" })\n    } else {\n      credential.issue(Other { who: \"b\" })\n    }",
        "    credential.issue(Card { who: \"a\" })\n    credential.issue(Other { who: \"b\" })",
    );
    let (program, errors) = built(&src);
    assert!(errors.is_empty(), "{errors:?}");
    let r = report(&program);
    assert!(!r.issues.is_empty(), "an effect written straight into `execute` is not in the report");
}

/// A field written in one branch of an `update` is a field this application
/// writes. The same flat loop missed it.
#[test]
fn a_field_written_in_a_branch_is_in_the_report() {
    let src = r#"
app "x.y"
version 1

capabilities {
}

state {
  a: int default 0
  b: int default 0
}

action Go {
  update {
    if (state.a > 0) {
      b: 1
    }
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Go }
  }
}
"#;
    let (program, errors) = built(src);
    assert!(errors.is_empty(), "{errors:?}");
    let r = report(&program);
    assert!(
        r.writes.iter().any(|w| w == "b"),
        "a field written in a branch is not in the report: {:?}",
        r.writes
    );
}

/// A credential verified in a branch is a credential this application reads,
/// and the policy it was read under is part of the sentence.
#[test]
fn a_credential_verified_in_a_branch_is_in_the_report() {
    let src = r#"
app "x.y"
version 1

capabilities {
  credential.read(Receipt)
}

credential Receipt {
  amount: int
}

trust FromShop(r: Receipt) {
  anchor: "shop.example"
}

state {
  n: int default 0
}

action Go {
  input {
    r: Credential<Receipt>
  }

  verify {
    if (state.n > 0) {
      const checked = r with FromShop
    }
  }

  update {
    n: 1
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Go }
  }
}
"#;
    let (program, errors) = built(src);
    assert!(errors.is_empty(), "{errors:?}");
    let r = report(&program);
    // Checked in a branch, and not read: nothing in this program reads a claim
    // off it, and the sheet says so on its own line rather than calling it a
    // read. Both lines matter to whoever is deciding.
    assert!(
        r.checks.iter().any(|x| x.contains("Receipt")),
        "a credential verified in a branch is in neither line: reads {:?} checks {:?}",
        r.reads,
        r.checks
    );
    assert!(r.reads.is_empty(), "and it reads nothing: {:?}", r.reads);
}
