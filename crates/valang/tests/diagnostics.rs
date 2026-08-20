//! What a diagnostic looks like.
//!
//! The message is part of the language, and so is where it points: a position
//! on its own is a thing to go and look up.

const SRC: &str = "app \"x.y\"\nversion 1\n\ncapabilities {\n}\n\nstate {\n  n: int default 0\n}\n\naction Go {\n  compute {\n    const fixed = 1\n    fixed = 2\n  }\n\n  update {\n    n: 1\n  }\n}\n\n@main\nscreen H {\n  column {\n    button(\"go\") { onTap: Go }\n  }\n}\n";

#[test]
fn a_diagnostic_shows_the_line_and_underlines_the_word() {
    let (_, d) = valang::analyse(SRC);
    let one = d
        .iter()
        .find(|x| x.message.contains("is a `const`"))
        .expect("the const is reported");
    let rendered = one.render(SRC);

    assert!(rendered.contains("    fixed = 2"), "the line is not in it:\n{rendered}");
    assert!(rendered.contains("--> 14:5"), "the position is not in it:\n{rendered}");
    // Five carets under `fixed`, at its column.
    assert!(rendered.contains("^^^^^"), "the word is not underlined:\n{rendered}");
    let caret_line = rendered.lines().last().unwrap();
    assert_eq!(
        caret_line.find('^'),
        rendered.lines().nth(3).unwrap().find("fixed"),
        "the underline is not under the word:\n{rendered}"
    );
}

/// Columns are characters, not bytes. Half of what these files say is Thai.
#[test]
fn a_line_with_thai_in_it_is_underlined_in_the_right_place() {
    let src = "app \"x.y\"\nversion 1\n\ncapabilities {\n}\n\nstate {\n  n: int default 0\n}\n\n@main\nscreen H {\n  column {\n    section(\"ประโยคภาษาไทยยาวๆ\", nope: 1)\n  }\n}\n";
    let (_, d) = valang::analyse_fully(
        src,
        None,
        &valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
            "../../../hosts/core.json"
        ))
        .unwrap()]),
    );
    let one = d.iter().find(|x| x.message.contains("`nope`")).expect("reported");
    let rendered = one.render(src);
    let line = rendered.lines().nth(3).unwrap();
    let carets = rendered.lines().last().unwrap();

    // Both measured the way a terminal measures: the caret's column is how wide
    // the text before it is, not how many bytes or characters it has.
    use unicode_width::UnicodeWidthStr;
    let caret_at = UnicodeWidthStr::width(&carets[..carets.find('^').unwrap()]);
    let word_at = UnicodeWidthStr::width(&line[..line.find("nope").unwrap()]);
    assert_eq!(
        caret_at, word_at,
        "a line with Thai in it was marked in the wrong place:\n{rendered}"
    );
}

/// A diagnostic about the package rather than a place in it says its piece and
/// points at nothing.
#[test]
fn a_diagnostic_with_nowhere_to_point_still_reads() {
    let src = "app \"x.y\"\nversion 1\n\ncapabilities {\n  credential.read(R)\n}\n\ncredential R {\n  a: int\n}\n\nstate {\n  n: int default 0\n}\n\n@main\nscreen H {\n  column {\n    section(\"x\")\n  }\n}\n";
    let (_, d) = valang::analyse(src);
    for x in &d {
        let rendered = x.render(src);
        assert!(!rendered.is_empty());
        assert!(rendered.contains(&x.message));
    }
}
