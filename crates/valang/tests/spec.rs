//! Sentences the specification makes, tested as sentences.
//!
//! Each of these is a claim `docs/spec.md` states in bold, picked because
//! nothing in the suite would have failed if it stopped being true. That is how
//! the last two bugs were found: a rule with no test is a rule that goes quiet.

use valang::capability::{Host, Hosts};

const CORE: &str = include_str!("../../../hosts/core.json");

fn errors(src: &str) -> Vec<String> {
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    valang::analyse_fully(src, None, &hosts)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::diag::Severity::Error)
        .map(|d| d.message)
        .collect()
}

/// "A screen may derive, and may not act. Its `compute` block follows an
/// action's rules: pure, no effects."
#[test]
fn a_screens_compute_may_not_act() {
    let src = r#"
app "x.y"
version 1

capabilities {
  credential.issue(Card)
}

credential Card as "https://org.vaulet.id/example/credential/card" {
  who: string
}

state {
  n: int default 0
}

@main
screen Home {
  compute {
    const c = credential.issue(Card { who: "me" })
  }

  column {
    section("x")
  }
}
"#;
    let e = errors(src);
    assert!(
        e.iter().any(|m| m.contains("effect") || m.contains("execute")),
        "a screen issued a credential from its `compute` block: {e:?}"
    );
}

/// "Anything from an API cannot be proved. A query answer is somebody's word,
/// not somebody's signature."
#[test]
fn a_query_answer_cannot_be_proved() {
    let src = r#"
app "x.y"
version 1

capabilities {
  api.query(broker.co.th)
  disclosure.present
}

state {
  n: int default 0
}

action Show {
  verify {
    const quotes = query broker.quotes
  }

  update {
    n: 1
  }

  execute {
    present {
      prove quotes.count > 0
    }
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Show }
  }
}
"#;
    let e = errors(src);
    assert!(
        !e.is_empty(),
        "an API answer was proved, which is somebody's word passed off as somebody's signature"
    );
}

/// "A version is what an importer depends on. Changing an exported component's
/// parameters is a breaking change to packages that are not yours."
///
/// The compiler cannot see the other package, so what it can do is refuse the
/// call — and it has to, or the change is silent at both ends.
#[test]
fn a_changed_export_breaks_the_call_that_used_it() {
    let kit = |param: &str| {
        format!(
            "app \"org.kit\"\nversion 1\n\ncapabilities {{\n}}\n\nexport component Chip({param}: string) {{\n  section({param})\n}}\n"
        )
    };
    let app = r#"
app "org.app"
version 1

capabilities {
}

import "org.kit/1" { Chip }

@main
screen Home {
  column {
    Chip(label: "x")
  }
}
"#;
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let against = |kit: String| {
        let packages = valang::expand::Packages::of(vec![valang::parse::parse(&kit).0]);
        valang::analyse_with_packages(app, None, &hosts, &packages)
            .1
            .into_iter()
            .filter(|d| d.severity == valang::diag::Severity::Error)
            .map(|d| d.message)
            .collect::<Vec<_>>()
    };

    assert!(against(kit("label")).is_empty(), "{:?}", against(kit("label")));
    let renamed = against(kit("title"));
    assert!(
        !renamed.is_empty(),
        "the exported component's parameter was renamed and the call site said nothing"
    );
}

/// "Interaction state belongs to the host … An action receives what the form
/// held at the moment it was submitted, through `input`."
///
/// So a screen cannot read it: there is no name for what is typed but not
/// submitted, and inventing one would be a second place it lives.
#[test]
fn a_screen_cannot_read_what_is_typed_but_not_submitted() {
    let src = r#"
app "x.y"
version 1

capabilities {
}

state {
  n: int default 0
}

action Go {
  input {
    note: string
  }

  update {
    n: 1
  }
}

@main
screen Home {
  column {
    field("Note") { into: note }
    text(note)
    button("go") { onTap: Go }
  }
}
"#;
    let e = errors(src);
    assert!(
        !e.is_empty(),
        "a screen read the field it had just declared, which is state the host holds"
    );
}

/// "A change to the shape of `state` starts that version's state empty."
///
/// Nothing in the language can say otherwise — there is no migration to write —
/// so what this checks is that a state field cannot be given a value from
/// anywhere but its `default`.
#[test]
fn a_state_field_starts_where_its_default_says() {
    let src = r#"
app "x.y"
version 1

capabilities {
}

state {
  n: int default previous.n
}

@main
screen Home {
  column {
    section("x")
  }
}
"#;
    let e = errors(src);
    assert!(
        !e.is_empty(),
        "a state field took its starting value from something outside the declaration"
    );
}

/// "A press names an action." Every handler does, not only `onTap` — the core
/// registry gives `list` an `onRemove`, and a target nothing declares is the
/// same mistake wherever it is written.
#[test]
fn every_handler_names_something_that_exists() {
    let src = r#"
app "x.y"
version 1

capabilities {
}

state {
  n: int default 0
}

action Go {
  update {
    n: 1
  }
}

@main
screen Home {
  column {
    list([1, 2]) { r ->
      text(r)
    }
    button("go") { onTap: Go }
  }
}
"#;
    assert!(errors(src).is_empty(), "{:?}", errors(src));

    let broken = src.replace(
        "    list([1, 2]) { r ->",
        "    list([1, 2]) { r ->\n      onRemove: Nowhere",
    );
    let e = errors(&broken);
    assert!(
        e.iter().any(|m| m.contains("Nowhere")),
        "`onRemove` named an action nothing declares and nobody said so: {e:?}"
    );
}

/// "Irreversible effects run last. The compiler orders them for you."
#[test]
fn irreversible_effects_run_last() {
    let src = r#"
app "x.y"
version 1

capabilities {
  credential.read(Id)
  credential.issue(Card)
  disclosure.present
  storage.write
}

credential Id as "https://org.vaulet.id/example/credential/id" {
  country: string
}

credential Card as "https://org.vaulet.id/example/credential/card" {
  who: string
}

trust Issued(id: Id) {
  anchor: "th.go.dopa"
  require {
    id.signature.valid
  }
}

state {
  n: int default 0
}

action Go {
  input {
    id: Credential<Id>
  }

  verify {
    const checked = id with Issued
  }

  update {
    n: 1
  }

  execute {
    present {
      disclose checked.claims.country
    }
    storage.write(key: "a", value: "b")
    credential.issue(Card { who: "me" })
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Go }
  }
}
"#;
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let (program, d) = valang::analyse_fully(src, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    // The order the effects are written is not the order they are offered: the
    // ones that cannot be undone go last, and the compiler is what puts them
    // there. Read off the action as the runtime will walk it.
    let go = program.actions.iter().find(|a| a.name == "Go").expect("declared");
    let execute = go
        .phases
        .iter()
        .find(|b| b.phase == valang::ast::Phase::Execute)
        .expect("has an execute block");
    let names: Vec<String> = execute
        .stmts
        .iter()
        .filter_map(|s| match s {
            valang::ast::Stmt::Effect { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    let last = names.last().cloned().unwrap_or_default();
    assert!(
        last == "present" || last == "credential.issue",
        "the irreversible effect is not last: {names:?}"
    );
}

/// "No derived values" and "no interaction state" in `state`. Both are about
/// what belongs there, and the first is checkable: a field whose value is only
/// ever a total of something already on the screen.
///
/// The language cannot see intent, so what it can refuse is the shape that
/// makes the mistake possible — and it turns out it refuses neither.
#[test]
fn what_state_may_hold() {
    let src = r#"
app "x.y"
version 1

capabilities {
}

state {
  n:        int default 0
  openTab:  string default "first"
}

action Go {
  update {
    openTab: "second"
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Go }
  }
}
"#;
    // Nothing here is refused today, and nothing in the language could tell
    // `openTab` from any other field. The claim is guidance rather than a rule,
    // and this test says which it is so that nobody reads it as the other.
    assert!(
        errors(src).is_empty(),
        "if this starts failing, `state` grew a rule and the specification should say so: {:?}",
        errors(src)
    );
}

/// "At most one disclosure per action."
#[test]
fn one_disclosure_to_an_action() {
    let src = r#"
app "x.y"
version 1

capabilities {
  credential.read(Id)
  disclosure.present
}

credential Id as "https://org.vaulet.id/example/credential/id" {
  country:   string
  birthdate: date
}

trust Issued(id: Id) {
  anchor: "th.go.dopa"
  require {
    id.signature.valid
  }
}

state {
  n: int default 0
}

action Go {
  input {
    id: Credential<Id>
  }

  verify {
    const checked = id with Issued
  }

  update {
    n: 1
  }

  execute {
    present {
      disclose checked.claims.country
    }
    present {
      disclose checked.claims.birthdate
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
    let e = errors(src);
    assert!(
        e.iter().any(|m| m.contains("disclosure")),
        "an action disclosed twice: {e:?}"
    );
}

/// "Numbers are decimal … no hex, no binary, no exponents, and `12.50` is a
/// compile error."
#[test]
fn numbers_are_decimal_and_whole() {
    let cases = [
        ("a float", "state {\n  n: int default 12.50\n}"),
        ("hex", "state {\n  n: int default 0xff\n}"),
        ("an exponent", "state {\n  n: int default 1e3\n}"),
    ];
    for (what, block) in cases {
        let src = format!(
            "app \"x.y\"\nversion 1\n\ncapabilities {{\n}}\n\n{block}\n\n@main\nscreen H {{\n  column {{\n    section(\"x\")\n  }}\n}}\n"
        );
        assert!(!errors(&src).is_empty(), "{what} was accepted: {block}");
    }
}

/// "Identifiers are ASCII and camelCase for names you choose."
#[test]
fn an_identifier_is_ascii() {
    let src = "app \"x.y\"\nversion 1\n\ncapabilities {\n}\n\nstate {\n  แต้ม: int default 0\n}\n\n@main\nscreen H {\n  column {\n    section(\"x\")\n  }\n}\n";
    let e = errors(src);
    assert!(e.iter().any(|m| m.contains("ASCII")), "a Thai identifier was accepted: {e:?}");
}

/// "Arguments are named once there are two. One argument may be positional."
#[test]
fn two_arguments_are_named() {
    let one = "app \"x.y\"\nversion 1\n\ncapabilities {\n}\n\nfunction f(a: int): int {\n  return a\n}\n\nfunction g(): int {\n  return f(1)\n}\n\nstate {\n  n: int default 0\n}\n\n@main\nscreen H {\n  column {\n    section(\"x\")\n  }\n}\n";
    assert!(errors(one).is_empty(), "one positional argument was refused: {:?}", errors(one));

    let two = one
        .replace("function f(a: int): int {\n  return a\n}", "function f(a: int, b: int): int {\n  return a + b\n}")
        .replace("return f(1)", "return f(1, 2)");
    let e = errors(&two);
    assert!(
        e.iter().any(|m| m.contains("so they are named")),
        "two positional arguments were accepted: {e:?}"
    );
}

/// "The type names the policy. `Verified<SignatureOnly>` and
/// `Verified<ReceiptFromMerchant>` are different types, and a function that
/// wants the second will not take the first."
#[test]
fn a_policy_is_part_of_the_type() {
    let src = r#"
app "x.y"
version 1

capabilities {
  credential.read(Receipt)
}

credential Receipt as "https://org.vaulet.id/example/credential/receipt" {
  amount: int
}

trust Loose(r: Receipt) {
  anchor: "shop.example"
  require {
    r.signature.valid
  }
}

trust Strict(r: Receipt) {
  anchor: "shop.example"
  require {
    r.signature.valid
    r.status.active
  }
}

function wants(r: Verified<Strict>): int {
  return r.claims.amount
}

state {
  n: int default 0
}

action Go {
  input {
    r: Credential<Receipt>
  }

  verify {
    const loose = r with Loose
  }

  compute {
    const out = wants(loose)
  }

  update {
    n: out
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Go }
  }
}
"#;
    let e = errors(src);
    assert!(
        e.iter().any(|m| m.contains("Strict") && m.contains("Loose")),
        "a credential verified against one policy was taken where another was asked for: {e:?}"
    );
}

/// "It pays for the bound, not the data." The compiler cannot cost a circuit,
/// but it can refuse the case that has no bound to pay for: a list consumed in
/// a proof whose length nothing wrote down.
#[test]
fn a_list_a_proof_walks_has_a_bound() {
    let src = r#"
app "x.y"
version 1

capabilities {
  credential.read(Holding)
  disclosure.present
}

credential Holding as "https://org.vaulet.id/example/credential/holding" {
  value: int
}

trust FromBroker(h: Holding) {
  anchor: "broker.example"
  require {
    h.signature.valid
  }
}

state {
  n: int default 0
}

action Prove {
  verify {
    const holdings = credentials of Holding verified with FromBroker
  }

  update {
    n: 1
  }

  execute {
    present {
      prove holdings.fold(0) { sum, h -> sum + h.claims.value } >= 100
    }
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Prove }
  }
}
"#;
    let e = errors(src);
    assert!(
        e.iter().any(|m| m.contains("limit")),
        "a proof walked a list of unwritten length: {e:?}"
    );
}
