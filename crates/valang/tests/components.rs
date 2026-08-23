//! A package's own components, expanded into the host's catalogue.
//!
//! What the assertions below pin is the shape a reader of the source would
//! predict: `MoneyCard(label: "balance", …)` is one `card`, and `TwoButtons` is
//! the three nodes its body lists. Nothing here reads a value back out of the
//! expander to compare it with itself.

const SRC: &str = r#"
app "example.components"
version "1.0.0"

capabilities {
}

type CardStyle {
  emphasis: string?
  icon:     string?
}

state {
  presses: int default 0
}

action Press {
  update {
    presses: 1
  }
}

component MoneyCard(label: string, amount: int, style: CardStyle) {
  card(text: label, amount: amount, ...style)
}

component TwoButtons(left: string, right: string) {
  row(text: "pair")
  button(text: "left", emphasis: secondary, onTap: Press)
  button(text: "right", emphasis: primary, onTap: Press)
}

screen Home {
  column {
    MoneyCard(label: "balance", amount: state.presses, style: { emphasis: "primary", icon: "wallet" })
    TwoButtons(left: "a", right: "b")
  }
}
"#;

fn errors(src: &str) -> Vec<String> {
    let (_, d) = valang::analyse(src);
    d.iter()
        .filter(|x| x.severity == valang::diag::Severity::Error)
        .map(|x| x.message.clone())
        .collect()
}

#[test]
fn a_screen_holds_only_catalogue_nodes_after_expansion() {
    let (program, d) = valang::analyse(SRC);
    assert!(errors(SRC).is_empty(), "{d:?}");

    let home = &program.screens[0];
    let column = &home.tree[0];
    assert_eq!(column.kind, "column");

    let kinds: Vec<&str> = column.children.iter().map(|c| c.kind.as_str()).collect();
    // One card from MoneyCard, then the three nodes TwoButtons lists.
    assert_eq!(kinds, ["card", "row", "button", "button"]);

    for node in &column.children {
        assert!(
            valang::expand::is_catalogue_name(&node.kind),
            "`{}` is not a catalogue name",
            node.kind
        );
    }
}

/// The call site's value reaches the node, so `text: label` is the key the
/// caller named rather than the parameter's own name.
#[test]
fn a_parameter_is_replaced_by_what_the_call_site_handed_it() {
    let (program, _) = valang::analyse(SRC);
    let card = &program.screens[0].tree[0].children[0];
    let text = card.args.iter().find(|a| a.name.as_deref() == Some("text")).expect("a text arg");
    match &text.value {
        valang::ast::Expr::Str { value, .. } => assert_eq!(value, "balance"),
        other => panic!("expected the caller's key, got {other:?}"),
    }
}

/// A lowercase name is a catalogue's name, whatever catalogue the host
/// published, so the collision cannot happen rather than being caught per host.
#[test]
fn a_component_may_not_take_a_lowercase_name() {
    let src = SRC.replace("component MoneyCard(", "component card(");
    let msgs = errors(&src);
    assert!(
        msgs.iter().any(|m| m.contains("capitalised")),
        "expected a clash to be reported, got {msgs:?}"
    );
}

#[test]
fn a_component_is_capitalised() {
    let src = SRC.replace("component TwoButtons(", "component twoButtons(")
        .replace("TwoButtons(left:", "twoButtons(left:");
    let msgs = errors(&src);
    assert!(
        msgs.iter().any(|m| m.contains("capitalised")),
        "expected the case rule to be reported, got {msgs:?}"
    );
}

/// Totality is the one promise here that cannot bend, and a cycle would expand
/// until the machine stopped. It is reported before anything is expanded.
#[test]
fn a_component_may_not_use_itself() {
    let src = SRC.replace(
        r#"component TwoButtons(left: string, right: string) {
  row(text: "pair")"#,
        r#"component TwoButtons(left: string, right: string) {
  TwoButtons(left: left, right: right)
  row(text: "pair")"#,
    );
    let msgs = errors(&src);
    assert!(
        msgs.iter().any(|m| m.contains("uses itself")),
        "expected the cycle to be reported, got {msgs:?}"
    );
}

#[test]
fn a_missing_argument_is_reported_at_the_call_site() {
    let src = SRC.replace("TwoButtons(left: \"a\", right: \"b\")", "TwoButtons(left: \"a\")");
    let msgs = errors(&src);
    assert!(
        msgs.iter().any(|m| m.contains("needs `right`")),
        "expected the missing argument to be named, got {msgs:?}"
    );
}

#[test]
fn an_argument_the_component_does_not_take_is_reported() {
    let src = SRC.replace(
        "TwoButtons(left: \"a\", right: \"b\")",
        "TwoButtons(left: \"a\", right: \"b\", middle: \"c\")",
    );
    let msgs = errors(&src);
    assert!(
        msgs.iter().any(|m| m.contains("has no `middle`")),
        "expected the extra argument to be named, got {msgs:?}"
    );
}

/// A spread becomes one named argument per field of the record's declared type,
/// so a reader can say what the call passes by reading `CardStyle`.
#[test]
fn a_spread_becomes_one_argument_per_field() {
    let (program, _) = valang::analyse(SRC);
    let card = &program.screens[0].tree[0].children[0];
    let names: Vec<&str> = card.args.iter().filter_map(|a| a.name.as_deref()).collect();
    assert_eq!(names, ["text", "amount", "emphasis", "icon"]);
    assert!(card.args.iter().all(|a| !a.spread), "a spread survived expansion");
}

/// Only a record this package declared. A list spread into an argument list is
/// the reading this symbol has in the languages the expression layer borrows
/// from, and it is not what this means.
#[test]
fn only_a_declared_record_may_be_spread() {
    let src = SRC.replace(
        "component MoneyCard(label: string, amount: int, style: CardStyle) {",
        "component MoneyCard(label: string, amount: int, style: int) {",
    );
    let msgs = errors(&src);
    assert!(
        msgs.iter().any(|m| m.contains("only a record")),
        "expected the spread to be refused, got {msgs:?}"
    );
}

/// A rest parameter would leave the component unable to say what it accepts,
/// which is the one thing its declaration is for.
#[test]
fn a_spread_must_name_a_parameter() {
    let src = SRC.replace("card(text: label, amount: amount, ...style)", "card(...whatever)");
    let msgs = errors(&src);
    assert!(
        msgs.iter().any(|m| m.contains("is not a parameter")),
        "expected an unknown name to be reported, got {msgs:?}"
    );
}

/// A parameter is a parameter wherever it appears in an expression.
///
/// Substitution enumerated a handful of expression kinds and swallowed the
/// rest, so `note exists` inside a component kept the parameter's own name and
/// evaluated to nothing: a condition that was false for a value that was there.
/// One case per variant that was missing.
#[test]
fn a_parameter_is_substituted_in_every_kind_of_expression() {
    for (label, expr) in [
        ("exists", "note exists"),
        ("unary", "!(note exists)"),
        ("list", "[note, note] exists"),
        ("switch", "(switch (note) { default => true }) exists"),
        ("ternary", "(note exists ? note : note) exists"),
    ] {
        let src = format!(
            r#"
app "example.substitute"
version "1.0.0"

capabilities {{
}}

component C(note: string?) {{
  column {{
    if ({expr}) {{
      text(note)
    }}
  }}
}}

@main
screen Home {{
  column {{
    C(note: "here")
  }}
}}
"#
        );
        let (program, d) = valang::analyse(&src);
        assert!(
            d.iter().all(|x| x.severity != valang::diag::Severity::Error),
            "{label}: {d:?}"
        );

        // The condition must mention the value, not the parameter's name.
        let mut names = Vec::new();
        fn walk(nodes: &[valang::ast::UiNode], out: &mut Vec<String>) {
            for n in nodes {
                for a in &n.args {
                    a.value.walk(&mut |e| {
                        if let valang::ast::Expr::Ident { name, .. } = e {
                            out.push(name.clone());
                        }
                    });
                }
                walk(&n.children, out);
                walk(&n.otherwise, out);
            }
        }
        walk(&program.screens[0].tree, &mut names);
        assert!(!names.contains(&"note".to_string()), "{label}: `note` survived: {names:?}");
    }
}

/// Until this, nothing in a screen's tree was typed at all: a condition could be
/// a number and a component could be handed anything.
#[test]
fn a_condition_is_true_or_false() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

state {
  points: int default 0
}

@main
screen Home {
  column {
    if (state.points) {
      card("a")
    }
  }
}
"#;
    let msgs: Vec<String> = valang::analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(msgs.iter().any(|m| m.contains("a condition is true or false")), "{msgs:?}");
}

#[test]
fn a_component_is_handed_what_it_declared() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

state {
  points: int default 0
}

component C(label: string) {
  text(label)
}

@main
screen Home {
  column {
    C(label: state.points)
  }
}
"#;
    let msgs: Vec<String> = valang::analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(
        msgs.iter().any(|m| m.contains("`C` takes `label` as string, and this is int")),
        "{msgs:?}"
    );
}

/// A screen with parameters is one too — it is reached by a press that hands it
/// values, and those are the screen's declared parameters.
#[test]
fn a_screen_is_moved_to_with_what_it_declared() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

state {
  points: int default 0
}

@main
screen Home {
  column {
    tile(text: "go", onTap: Detail(id: state.points))
  }
}

screen Detail(id: string) {
  column {
    text(id)
  }
}
"#;
    let msgs: Vec<String> = valang::analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(msgs.iter().any(|m| m.contains("`Detail` takes `id` as string")), "{msgs:?}");
}

/// A combinator given no function at all used to return the list unchanged.
#[test]
fn a_combinator_is_given_a_function() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

state {
  n: int default 0
}

function f(xs: List<int>): List<int> {
  return xs.map(nothing)
}
"#;
    let msgs: Vec<String> = valang::analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(msgs.iter().any(|m| m.contains("`map` is given a function")), "{msgs:?}");
}

#[test]
fn a_named_function_takes_what_the_combinator_hands_over() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

state {
  n: int default 0
}

function add(a: int, b: int): int {
  return a + b
}

function f(xs: List<int>): List<int> {
  return xs.map(add)
}
"#;
    let msgs: Vec<String> = valang::analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(
        msgs.iter().any(|m| m.contains("`map` hands over 1 value(s), and `add` takes 2")),
        "{msgs:?}"
    );
}

/// A component takes text, and text with values in it is a phrase — which had
/// never worked, because a component's arguments are typed before expansion and
/// that is the one place a phrase is still a call.
#[test]
fn a_component_may_be_handed_a_phrase() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

state {
  points: int default 1
}

component Badge(label: string) {
  card(label)
}

@main
screen Home {
  column {
    Badge(label: phrase("row {n}", n: state.points))
    Badge(label: `row ${state.points}`)
  }
}
"#;
    let msgs: Vec<String> = valang::analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(msgs.is_empty(), "{msgs:?}");
}

/// `state.member?.name` is the author saying it may not be there, which is what
/// the narrowing rule asks them to say. Refusing it as well left no way to say
/// it at all outside `require`.
#[test]
fn optional_access_is_how_an_optional_is_read_outside_require() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

type Member { name: string }

state {
  member: Member?
}

action Go {
  compute {
    const who = state.member?.name ?: "guest"
  }

  update {
    member: { name: who }
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Go }
  }
}
"#;
    let msgs: Vec<String> = valang::analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(!msgs.iter().any(|m| m.contains("may not exist")), "{msgs:?}");
}

/// And a plain read of one is still refused, which is the rule's whole point.
#[test]
fn a_plain_read_of_an_optional_is_still_narrowed_first() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

type Member { name: string }

state {
  member: Member?
}

action Go {
  compute {
    const who = state.member.name
  }

  update {
    member: { name: who }
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Go }
  }
}
"#;
    let msgs: Vec<String> = valang::analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(msgs.iter().any(|m| m.contains("may not exist")), "{msgs:?}");
}

/// A bare name in a tree is bound, or a word the host has, or the name of an
/// action or screen. A fourth thing was drawn as itself: `text(pointss)` put the
/// word `pointss` on the screen and nobody was told.
#[test]
fn a_misspelt_name_in_a_tree_is_not_a_word() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

state {
  points: int default 0
}

@main
screen Home {
  column {
    text(pointss)
  }
}
"#;
    let hosts = valang::capability::Hosts::of(vec![
        valang::capability::Host::parse(include_str!("../../../hosts/core.json")).unwrap(),
    ]);
    let msgs: Vec<String> = valang::analyse_fully(src, None, &hosts)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(msgs.iter().any(|m| m.contains("`pointss` is neither")), "{msgs:?}");
}

/// And the three kinds that are not mistakes still are not: a word from an open
/// vocabulary the application invented, a field name a form introduces, and an
/// action a press names.
#[test]
fn the_names_a_tree_is_allowed_to_carry() {
    let src = r#"
app "x.y"
version "1.0.0"

capabilities {
}

state {
  points: int default 0
}

action Go {
  update {
    points: 1
  }
}

@main
screen Home {
  column {
    card(text: "hi", color: brandPink, style: title)
    checkbox("Remind me") { into: remind }
    button("go") { onTap: Go }
  }
}
"#;
    let hosts = valang::capability::Hosts::of(vec![
        valang::capability::Host::parse(include_str!("../../../hosts/core.json")).unwrap(),
    ]);
    let msgs: Vec<String> = valang::analyse_fully(src, None, &hosts)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(msgs.is_empty(), "{msgs:?}");
}
