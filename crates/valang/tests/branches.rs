//! `if` in a screen's tree, as the front end sees it.

use valang::capability::{Host, Hosts};

const CORE: &str = include_str!("../../../hosts/core.json");

fn errors(src: &str) -> Vec<String> {
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let (_, d) = valang::analyse_fully(src, None, &hosts);
    d.into_iter()
        .filter(|x| x.severity == valang::diag::Severity::Error)
        .map(|x| x.message)
        .collect()
}

fn program(tree: &str) -> String {
    format!(
        r#"
app "example.branches"
version 1

capabilities {{
}}

state {{
  points: int default 0
}}

action Earn {{
  update {{
    points: 1
  }}
}}

@main

screen Home {{
{tree}
}}
"#
    )
}

#[test]
fn both_halves_are_checked() {
    // The branch nobody took today is somebody's screen tomorrow. Checking only
    // the taken one would make a build's success depend on a value the compiler
    // does not have.
    let e = errors(&program(
        "  if (state.points > 0) {\n    card(\"ok\")\n  } else {\n    nonesuch(\"x\")\n  }",
    ));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("`nonesuch` is not something this host provides"), "{e:?}");
}

#[test]
fn a_press_inside_a_branch_still_names_something_real() {
    let e = errors(&program(
        "  if (state.points > 0) {\n    button(\"Go\") { onTap: Nowhere }\n  }",
    ));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("neither an action nor a screen"), "{e:?}");
}

#[test]
fn else_is_optional() {
    assert!(errors(&program("  if (state.points > 0) {\n    card(\"ok\")\n  }")).is_empty());
}

/// A node that follows an `if` with no `else` is a sibling, not a branch. The
/// parser has to look past a newline for `else` and put it back when there is
/// none, and getting that wrong swallows the next node silently.
#[test]
fn a_node_after_a_branchless_if_is_not_swallowed() {
    let src = program("  if (state.points > 0) {\n    card(\"ok\")\n  }\n  nonesuch(\"x\")");
    let e = errors(&src);
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("`nonesuch`"), "{e:?}");
}
