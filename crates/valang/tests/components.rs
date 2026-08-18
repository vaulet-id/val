//! A package's own components, expanded into the host's catalogue.
//!
//! What the assertions below pin is the shape a reader of the source would
//! predict: `MoneyCard(label: "balance", …)` is one `card`, and `TwoButtons` is
//! the three nodes its body lists. Nothing here reads a value back out of the
//! expander to compare it with itself.

const SRC: &str = r#"
app "example.components"
version 1

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
            valang::expand::CATALOGUE.contains(&node.kind.as_str()),
            "`{}` is not something the host ships",
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

#[test]
fn a_component_may_not_take_a_name_the_host_ships() {
    let src = SRC.replace("component MoneyCard(", "component card(");
    let msgs = errors(&src);
    assert!(
        msgs.iter().any(|m| m.contains("the host ships")),
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
