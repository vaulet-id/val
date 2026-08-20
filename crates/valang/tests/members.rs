//! What may be written after a dot, answered by the typechecker.
//!
//! The editor used to answer this itself and had nothing to answer with, so it
//! offered every keyword in the language after `state.`. The rules here are the
//! ones the checker enforces — most of all that an issuer's words are behind
//! `claims` on a `Verified<P>` and are out of reach on anything else.

use valang::capability::Hosts;

const SRC: &str = r#"app "x.y"
version 1

capabilities {
  credential.read(Receipt)
}

enum Tier { bronze, silver, gold }

credential Receipt {
  merchant: string
  amount: int
}

type Address {
  city: string
  postcode: string
}

trust Shop(r: Receipt) {
  anchor: "shop.example.com"
  require {
    r.signature.valid
  }
}

state {
  points: int default 0
  tier: Tier default Tier.bronze
  home: Address?
  visits: List(int) default []
}

action Earn {
  input {
    receipt: Credential<Receipt>
  }

  verify {
    const checked = receipt with Shop
  }

  compute {
    const spent = checked.claims.amount
    const names = state.visits
    const x = 1
  }

  update {
    points: spent
  }
}
"#;

/// Line and column as an editor counts them: the position of the caret, which
/// is one past the dot.
fn at(src: &str, line: u32, col: u32) -> Vec<String> {
    let (p, _) = valang::parse::parse(src);
    valang::typeck::members_at(&p, &Hosts::default(), src, line, col)
        .into_iter()
        .map(|m| m.name)
        .collect()
}

/// The position just after the last character of `needle` on the line that
/// holds it. Written this way so a line added to the sample does not renumber
/// every assertion — which is how the first draft of these tests broke.
fn after(src: &str, needle: &str) -> (u32, u32) {
    let (i, line) = src.lines().enumerate().find(|(_, l)| l.contains(needle)).expect("no such line");
    let col = line.find(needle).unwrap() + needle.len();
    (i as u32 + 1, col as u32 + 1)
}

#[test]
fn state_offers_the_fields_this_program_declares() {
    let (l, c) = after(SRC, "  update {");
    let src = SRC.replace("    points: spent", "    points: state.");
    let (l2, c2) = after(&src, "points: state.");
    assert_eq!(at(&src, l2, c2), vec!["points", "tier", "home", "visits"], "line {l}:{c}");
}

#[test]
fn a_verified_credential_offers_claims_and_nothing_else() {
    let src = SRC.replace("const spent = checked.claims.amount", "const spent = checked.");
    let (l, c) = after(&src, "const spent = checked.");
    assert_eq!(at(&src, l, c), vec!["claims"]);
}

/// The rule the language is built on, from the other side: a credential that
/// has not been through `verify` offers nothing at all.
#[test]
fn a_held_credential_offers_nothing() {
    let src = SRC.replace("const spent = checked.claims.amount", "const spent = receipt.");
    let (l, c) = after(&src, "const spent = receipt.");
    assert!(at(&src, l, c).is_empty(), "{:?}", at(&src, l, c));
}

#[test]
fn claims_offer_what_the_credential_declares() {
    let src = SRC.replace("const spent = checked.claims.amount", "const spent = checked.claims.");
    let (l, c) = after(&src, "const spent = checked.claims.");
    assert_eq!(at(&src, l, c), vec!["merchant", "amount"]);
}

#[test]
fn an_enum_offers_its_members() {
    let src = SRC.replace("  const x = 1", "  const x = Tier.");
    let (l, c) = after(&src, "const x = Tier.");
    assert_eq!(at(&src, l, c), vec!["bronze", "silver", "gold"]);
}

/// A list literal is deliberately untyped — the checker leaves `["a", "b"]`
/// alone, and says so where it does it — so the list here comes from a declared
/// one. Offering the combinators on a literal would mean this file knew
/// something the checker does not, which is the disagreement it exists to
/// prevent.
#[test]
fn a_list_offers_the_operations_that_consume_one() {
    let src = SRC.replace("  const x = 1", "  const x = names.");
    let (l, c) = after(&src, "const x = names.");
    assert_eq!(at(&src, l, c), vec!["map", "filter", "fold", "any", "all", "count", "first"]);
}

/// An optional record still says what it holds. Reaching through it without
/// `?.` is a separate mistake, with a message of its own — an editor that
/// offered nothing here would be teaching that the field does not exist.
#[test]
fn an_optional_field_offers_what_is_under_it() {
    let src = SRC.replace("  const x = 1", "  const x = state.home.");
    let (l, c) = after(&src, "const x = state.home.");
    assert_eq!(at(&src, l, c), vec!["city", "postcode"]);
}

#[test]
fn context_is_the_two_things_the_host_answers() {
    let src = SRC.replace("  const x = 1", "  const x = context.");
    let (l, c) = after(&src, "const x = context.");
    assert_eq!(at(&src, l, c), vec!["time", "uuid"]);

    let src = SRC.replace("  const x = 1", "  const x = context.time.");
    let (l, c) = after(&src, "const x = context.time.");
    assert_eq!(at(&src, l, c), vec!["now"]);
}

/// A binding is in scope from the line under it, and a name from another
/// action is not in scope at all. Declarations are not checked in the order
/// they were written, so without the block the cursor is in this answered with
/// whatever had been walked last.
#[test]
fn a_name_from_another_action_is_not_in_scope() {
    let src = format!(
        "{SRC}\naction Other {{\n  compute {{\n    const y = checked.\n  }}\n}}\n"
    );
    let (l, c) = after(&src, "const y = checked.");
    assert!(at(&src, l, c).is_empty(), "{:?}", at(&src, l, c));
}

/// Nothing is offered where nothing was asked. A cursor that is not after a dot
/// is the editor's own business — keywords, the names in the file — and this
/// must not answer it, or every one of those would be replaced by a claim.
#[test]
fn a_cursor_that_is_not_after_a_dot_says_nothing() {
    let (l, c) = after(SRC, "  const x = 1");
    assert!(at(SRC, l, c).is_empty());
}

/// The half-written case, which is the only case: `state.po` is a path, a dot,
/// and a word being typed that Monaco filters with.
#[test]
fn the_word_being_typed_is_not_part_of_the_path() {
    let src = SRC.replace("  const x = 1", "  const x = state.po");
    let (l, c) = after(&src, "const x = state.po");
    assert_eq!(at(&src, l, c), vec!["points", "tier", "home", "visits"]);
}
