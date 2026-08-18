//! A package is checked against what the host says it can do.
//!
//! The documents under `interfaces/` are the fixtures. A list of capabilities
//! inside the compiler would be a list of the first host's, and the second host
//! would be implementing Vaulet rather than implementing VAL.

use valang::catalogue::{Catalogue, Catalogues};
use valang::interface::{Interface, Interfaces};

const CORE_CATALOGUE: &str = include_str!("../../../catalogues/core.json");
const EFFECTS: &str = include_str!("../../../interfaces/core-effects.json");
const NAVIGATION: &str = include_str!("../../../interfaces/navigation.json");

fn host() -> (Catalogues, Interfaces) {
    let cats = Catalogues::of(vec![Catalogue::parse(CORE_CATALOGUE).expect("catalogue")]);
    let mut loaded = Interface::parse_many(EFFECTS).expect("the effects parse");
    loaded.push(Interface::parse(NAVIGATION).expect("navigation parses"));
    (cats, Interfaces::of(loaded))
}

fn errors(src: &str) -> Vec<String> {
    let (cats, ifaces) = host();
    let (_, d) = valang::analyse_fully(src, None, &cats, &ifaces);
    d.iter()
        .filter(|x| x.severity == valang::diag::Severity::Error)
        .map(|x| x.message.clone())
        .collect()
}

const TWO_SCREENS: &str = r#"
app "example.navigation"
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

  execute {
    navigation.navigate(to: Done, as: replace)
  }
}

screen Home start: true {
  column {
    button(text: sentence("go"), emphasis: primary, onTap: Go)
  }
}

screen Done present: sheet {
  column {
    card(text: sentence("done"))
  }
}
"#;

#[test]
fn a_package_that_matches_the_host_is_clean() {
    assert!(errors(TWO_SCREENS).is_empty(), "{:?}", errors(TWO_SCREENS));
}

/// The words come from the document. A host without sheets does not list
/// `present`, and the message is the same one the catalogue gives.
#[test]
fn a_word_outside_a_vocabulary_is_reported_with_the_words() {
    let src = TWO_SCREENS.replace("as: replace", "as: slide");
    let msgs = errors(&src);
    assert!(
        msgs.iter().any(|m| m.contains("is not one of transition") && m.contains("replace")),
        "got {msgs:?}"
    );
}

#[test]
fn a_prop_the_operation_does_not_take_is_reported() {
    let src = TWO_SCREENS.replace("navigation.navigate(to: Done,", "navigation.navigate(to: Done, speed: fast,");
    let msgs = errors(&src);
    assert!(msgs.iter().any(|m| m.contains("has no `speed`")), "got {msgs:?}");
}

#[test]
fn a_capability_this_host_does_not_offer_is_reported() {
    let src = TWO_SCREENS.replace("navigation.navigate(to: Done, as: replace)", "hologram.project(of: n)");
    let msgs = errors(&src);
    assert!(
        msgs.iter().any(|m| m.contains("is not something this host offers")),
        "got {msgs:?}"
    );
}

#[test]
fn a_screen_setting_no_capability_gives_is_reported() {
    let src = TWO_SCREENS.replace("screen Done present: sheet", "screen Done colour: blue");
    let msgs = errors(&src);
    assert!(msgs.iter().any(|m| m.contains("a screen has no `colour`")), "got {msgs:?}");
}

/// A package is several files, so "the first screen declared" would mean the
/// order files were read decides what somebody sees.
#[test]
fn more_than_one_screen_says_where_it_opens() {
    let src = TWO_SCREENS.replace("screen Home start: true", "screen Home");
    let msgs = errors(&src);
    assert!(msgs.iter().any(|m| m.contains("none says `start: true`")), "got {msgs:?}");
}

#[test]
fn two_screens_may_not_both_start() {
    let src = TWO_SCREENS.replace("screen Done present: sheet", "screen Done start: true");
    let msgs = errors(&src);
    assert!(msgs.iter().any(|m| m.contains("a package opens at one")), "got {msgs:?}");
}
