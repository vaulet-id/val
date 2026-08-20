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

credential Card {
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
