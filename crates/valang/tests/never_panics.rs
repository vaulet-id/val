//! A compiler answers. It does not fall over.
//!
//! Whatever the bytes are, the front end returns diagnostics — an input nobody
//! could have meant is still an input somebody will paste.

const INPUTS: &[&str] = &[
    "",
    " ",
    "\n\n\n",
    "\0",
    "app",
    "app \"",
    "app \"x\" version",
    "{",
    "}",
    "((((((((((",
    "))))))))))",
    "screen",
    "screen Home {",
    "screen Home { column { ",
    "function f(",
    "function f(): {",
    "action A { update { ",
    "const",
    "const x =",
    "x = ",
    "let",
    "for (",
    "for (i in",
    "if (",
    "if () { }",
    "`",
    "`${",
    "`${}`",
    "\"unterminated",
    "credential C {",
    "credential C as {",
    "credential C as \"\" {",
    "trust T(",
    "import",
    "import \"",
    "export",
    "@",
    "@main",
    "@main @main @main",
    "type T { a: List<",
    "type T { a: List(",
    "state { n: int default }",
    "switch",
    "1...",
    "...1",
    "a ?: ",
    "a?.",
    "é",
    "ก",
    "🙂",
];

#[test]
fn no_input_makes_the_front_end_fall_over() {
    for src in INPUTS {
        let (_, d) = valang::analyse(src);
        // Nothing is asserted about what it says. The test is that it said
        // something and came back.
        let _ = d.len();
    }
}

/// The same, against a registry — the passes that read one are a different set.
#[test]
fn no_input_makes_the_checks_fall_over() {
    let hosts = valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")]);
    for src in INPUTS {
        let (_, d) = valang::analyse_fully(src, None, &hosts);
        let _ = d.len();
    }
}

/// And every prefix of a real program: truncation is what a half-saved file and
/// a half-sent request both look like.
#[test]
fn no_prefix_of_a_program_makes_it_fall_over() {
    const LOYALTY: &str = include_str!("../../../examples/loyalty.val");
    for n in 0..LOYALTY.len() {
        if !LOYALTY.is_char_boundary(n) {
            continue;
        }
        let (_, d) = valang::analyse(&LOYALTY[..n]);
        let _ = d.len();
    }
}
