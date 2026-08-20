//! What may be written, and where.

use valang::capability::{Host, Hosts};

const CORE: &str = include_str!("../../../hosts/core.json");

fn errors(body: &str) -> Vec<String> {
    let src = format!(
        r#"
app "x.y"
version 1

capabilities {{
}}

state {{
  n: int default 0
}}

action Go {{
{body}

  update {{
    n: 1
  }}
}}

@main
screen Home {{
  column {{
    button("go") {{ onTap: Go }}
  }}
}}
"#
    );
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    valang::analyse_fully(&src, None, &hosts)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::diag::Severity::Error)
        .map(|d| d.message)
        .collect()
}

#[test]
fn a_const_is_what_it_was_defined_as() {
    let e = errors("  compute {\n    const fixed = 1\n    fixed = 2\n  }");
    assert!(e.iter().any(|m| m.contains("`fixed` is a `const`")), "{e:?}");
}

#[test]
fn a_name_that_was_never_declared_is_not_written() {
    let e = errors("  compute {\n    never = 3\n  }");
    assert!(e.iter().any(|m| m.contains("`never` was never declared")), "{e:?}");
}

#[test]
fn a_name_keeps_the_type_it_was_declared_with() {
    let e = errors("  compute {\n    let s = \"a\"\n    s = 9\n  }");
    assert!(e.iter().any(|m| m.contains("`s` is string, and this is int")), "{e:?}");
}

/// A phase is what it is: `require` and `verify` hold conditions, `update` is a
/// patch, `execute` is a batch of effects. What is left is the two places whose
/// purpose is working a value out.
#[test]
fn a_phase_that_is_not_for_working_something_out_writes_nothing() {
    let e = errors("  require {\n    let x = 1\n    x = 2\n  }");
    assert!(e.iter().any(|m| m.contains("`require` is not where a value is worked out")), "{e:?}");
}

#[test]
fn there_is_no_var() {
    let e = errors("  compute {\n    var x = 1\n  }");
    assert!(e.iter().any(|m| m.contains("a variable is `let`; there is no `var`")), "{e:?}");
}
