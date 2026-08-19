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

/// The syntax carries arguments even though the only directive today takes
/// none, so the first one that needs an argument is a row in a table rather
/// than a second shape bolted on beside this one.
#[test]
fn a_directive_that_takes_nothing_is_given_nothing() {
    let e = errors(&program("  card(\"ok\")").replace("@main", "@main(true)"));
    assert!(e.iter().any(|m| m.contains("`@main` marks a declaration and takes nothing")), "{e:?}");
}

#[test]
fn an_unknown_directive_says_which_ones_exist() {
    let e = errors(&program("  card(\"ok\")").replace("@main", "@sheet"));
    assert!(e.iter().any(|m| m.contains("is not a directive this language has")), "{e:?}");
    assert!(e.iter().any(|m| m.contains("`@main`")), "{e:?}");
}

#[test]
fn a_directive_marks_a_screen_and_nothing_else() {
    let src = program("  card(\"ok\")").replace("action Earn", "@main\naction Earn");
    let e = errors(&src);
    assert!(e.iter().any(|m| m.contains("`@main` marks a screen, and `action` is not one")), "{e:?}");
}

/// A parameter inside the `else` half of a component's body is a parameter.
///
/// Substitution walked children and carried the other branch through unchanged,
/// so the name stayed the parameter's own and resolved to nothing — a component
/// that drew an empty card exactly when the condition was false.
#[test]
fn a_parameter_is_substituted_in_both_branches() {
    let src = format!(
        r#"
app "example.branches"
version 1

capabilities {{
}}

state {{
  points: int default 0
}}

component Either(shown: string, hidden: string) {{
  column {{
    if (state.points > 0) {{
      card(shown)
    }} else {{
      card(hidden)
    }}
  }}
}}

@main
screen Home {{
  column {{
    Either(shown: "yes", hidden: "no")
  }}
}}
"#
    );
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let (program, d) = valang::analyse_fully(&src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let mut found = String::new();
    fn walk(nodes: &[valang::ast::UiNode], out: &mut String) {
        for n in nodes {
            for a in &n.args {
                if let valang::ast::Expr::Str { value, .. } = &a.value {
                    out.push_str(value);
                }
            }
            walk(&n.children, out);
            walk(&n.otherwise, out);
        }
    }
    walk(&program.screens[0].tree, &mut found);
    assert!(found.contains("yes") && found.contains("no"), "one branch kept the parameter: {found}");
}

/// The siblings of the substitution bug, each the same mistake in a different
/// walker: a phrase in the branch that is not taken, and a cycle hidden there.
#[test]
fn a_phrase_in_the_other_branch_is_flattened() {
    let src = r#"
app "example.branches"
version 1

capabilities {
}

state {
  points: int default 0
}

@main
screen Home {
  column {
    if (state.points > 0) {
      card(phrase("You have {n}", n: state.points))
    } else {
      card(phrase("Nothing yet, {who}", who: "friend"))
    }
  }
}
"#;
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let (program, d) = valang::analyse_fully(src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    // Flattened means the `phrase` call is gone and its slot is recorded.
    let branch = &program.screens[0].tree[0].children[0].otherwise[0];
    assert_eq!(branch.slots, vec!["who".to_string()], "the other branch kept its phrase call");
}

#[test]
fn a_cycle_through_the_other_branch_is_still_a_cycle() {
    let src = r#"
app "example.branches"
version 1

capabilities {
}

state {
  points: int default 0
}

component Loop(text: string) {
  column {
    if (state.points > 0) {
      card(text)
    } else {
      Loop(text: text)
    }
  }
}

@main
screen Home {
  column {
    Loop(text: "x")
  }
}
"#;
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let (_, d) = valang::analyse_fully(src, None, &hosts);
    let msgs: Vec<String> = d.into_iter().map(|x| x.message).collect();
    assert!(msgs.iter().any(|m| m.contains("`Loop` uses itself")), "{msgs:?}");
}

/// A parser loop whose body can consume nothing is a hang, not a message — and
/// in the editor a hang is a tab that stops responding. `List(int)`, the wrong
/// bracket, spun forever in the type-argument loop and again in the parameter
/// list.
///
/// The assertion is that it returns at all: what it says is a second question.
#[test]
fn a_type_written_with_the_wrong_bracket_reports_rather_than_spinning() {
    let src = "app \"x.y\"\nversion 1\n\ncapabilities {\n}\n\nfunction f(rows: List(int)): int {\n  return 1\n}\n";
    let e = errors(src);
    assert!(!e.is_empty(), "a malformed type said nothing at all");
}

#[test]
fn a_parameter_that_is_neither_a_name_nor_a_type_reports() {
    let src = "app \"x.y\"\nversion 1\n\ncapabilities {\n}\n\nfunction f(: , 9): int {\n  return 1\n}\n";
    let e = errors(src);
    assert!(!e.is_empty(), "a malformed parameter list said nothing at all");
}
