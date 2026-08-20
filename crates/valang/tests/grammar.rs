//! The grammar and the parser, compared. Each test below is a production the
//! grammar states, checked against what the parser does with it.

/// An anchor names something outside the package, and every other name of that
/// kind in this language is quoted. Written bare it was read as a single token,
/// so `shop.example.com` became `shop` — a policy trusting a root nobody wrote.
#[test]
fn an_anchor_is_quoted() {
    let src = r#"
app "x.y"
version 1

capabilities {
}

credential R {
  amount: int
}

trust FromShop(r: R) {
  anchor: shop.example.com
}

state {
  n: int default 0
}

@main
screen H {
  column {
    section("x")
  }
}
"#;
    let (program, d) = valang::analyse(src);
    assert!(
        d.iter().any(|x| x.message.contains("an anchor is quoted")),
        "an anchor written bare was accepted: {:?}",
        d.iter().map(|x| &x.message).collect::<Vec<_>>()
    );
    // And what it holds is the whole name, so the message can quote it back.
    assert_eq!(program.trusts[0].anchor.as_deref(), Some("shop.example.com"));
}

/// `enum Tier { bronze silver }` parses, and so does the same with commas. Two
/// ways to write one thing is what this language spends its budget avoiding,
/// and the enum body is where it does not.
#[test]
fn an_enum_body_is_written_one_way() {
    let with_commas = "app \"x.y\"\nversion 1\n\ncapabilities {\n}\n\nenum Tier { bronze, silver }\n\nstate {\n  n: int default 0\n}\n\n@main\nscreen H {\n  column {\n    section(\"x\")\n  }\n}\n";
    let without = with_commas.replace("bronze, silver", "bronze silver");

    let said = |src: &str| {
        valang::analyse(src)
            .1
            .into_iter()
            .filter(|d| d.severity == valang::Severity::Error)
            .map(|d| d.message)
            .collect::<Vec<_>>()
    };
    assert!(said(with_commas).is_empty(), "{:?}", said(with_commas));
    assert!(
        !said(&without).is_empty(),
        "an enum written without commas was accepted, so there are two ways to write one"
    );
}

/// `const { a, b } = row` works and `let { a, b } = row` does not. Two binding
/// forms and only one of them takes a record apart.
#[test]
fn both_bindings_take_a_record_apart() {
    let src = |word: &str| {
        format!("app \"x.y\"\nversion 1\n\ncapabilities {{\n}}\n\nstate {{\n  n: int default 0\n}}\n\naction Go {{\n  compute {{\n    {word} {{ a }} = {{ a: 1 }}\n  }}\n\n  update {{\n    n: a\n  }}\n}}\n\n@main\nscreen H {{\n  column {{\n    button(\"g\") {{ onTap: Go }}\n  }}\n}}\n")
    };
    let said = |s: String| {
        valang::analyse(&s)
            .1
            .into_iter()
            .filter(|d| d.severity == valang::Severity::Error)
            .map(|d| d.message)
            .collect::<Vec<_>>()
    };
    assert!(said(src("const")).is_empty(), "{:?}", said(src("const")));
    assert!(said(src("let")).is_empty(), "`let` cannot take a record apart: {:?}", said(src("let")));
}
