//! Source is UTF-8, and the lexer walks characters rather than bytes.
//!
//! A string with an em dash or a Thai vowel in it used to advance one byte into
//! the character and then slice there, which panics — on source the language
//! says is valid, since strings are full UTF-8 by design.

fn analyse(src: &str) -> Vec<String> {
    let (_, d) = valang::analyse(src);
    d.iter().map(|x| x.message.clone()).collect()
}

const WITH_UTF8: &str = r#"
app "example.utf8"
version 1

capabilities {
}

state {
  note: string default "เขียนคำลงไปตรงๆ — ทั้งประโยค"
}

screen Home {
  column {
    card("A dash — and Thai ประโยค in one string")
  }
}
"#;

#[test]
fn a_string_may_hold_any_character() {
    let msgs = analyse(WITH_UTF8);
    assert!(msgs.is_empty(), "{msgs:?}");
}

/// The lexer must move past a character it does not expect rather than sitting
/// on one byte of it.
#[test]
fn an_unclosed_string_of_wide_characters_still_terminates() {
    let msgs = analyse("app \"x\"\nversion 1\nstate { s: string default \"— ไทย");
    assert!(msgs.iter().any(|m| m.contains("never closed")), "{msgs:?}");
}
