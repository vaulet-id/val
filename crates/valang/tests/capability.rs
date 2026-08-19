//! A package is checked against what the host provides.
//!
//! The documents under `hosts/` are the fixtures, which is the point: they are
//! what a wallet publishes, and a list inside the compiler would be the first
//! wallet's.

use valang::capability::{Host, Hosts};

const CORE: &str = include_str!("../../../hosts/core.json");
const VAULET: &str = include_str!("../../../hosts/vaulet.json");

fn hosts(extra: bool) -> Hosts {
    let mut loaded = vec![Host::parse(CORE).expect("the core registry parses")];
    if extra {
        loaded.push(Host::parse(VAULET).expect("vaulet's registry parses"));
    }
    Hosts::of(loaded)
}

fn errors(src: &str, extra: bool) -> Vec<String> {
    let (_, d) = valang::analyse_fully(src, None, &hosts(extra));
    d.iter()
        .filter(|x| x.severity == valang::diag::Severity::Error)
        .map(|x| x.message.clone())
        .collect()
}

const BASE: &str = r#"
app "example.capability"
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
    tile(text: phrase("row"), onTap: Go)
    button(text: phrase("go"), emphasis: primary, onTap: Go)
  }
}

screen Done present: sheet {
  column {
    card(text: phrase("done"))
  }
}
"#;

#[test]
fn a_package_that_matches_the_host_is_clean() {
    assert!(errors(BASE, false).is_empty(), "{:?}", errors(BASE, false));
}

#[test]
fn something_no_host_provides_is_reported() {
    let src = BASE.replace("tile(text: phrase(\"row\"), onTap: Go)", "hologram(text: phrase(\"row\"))");
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("not something this host provides")), "got {msgs:?}");
}

/// One registry, so a drawn capability written as a call is a placement
/// mistake with its own sentence rather than an unknown name.
#[test]
fn a_drawn_capability_called_in_execute_is_reported() {
    let src = BASE.replace("navigation.navigate(to: Done, as: replace)", "ui.grid(columns: 2)");
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("not something this host provides")), "got {msgs:?}");
}

#[test]
fn a_called_capability_drawn_on_a_screen_is_reported() {
    let src = BASE.replace(
        "tile(text: phrase(\"row\"), onTap: Go)",
        "credential.issue(of: n)",
    );
    let msgs = errors(&src, false);
    assert!(
        msgs.iter().any(|m| m.contains("is not drawn") || m.contains("not something this host provides")),
        "got {msgs:?}"
    );
}

/// A host's own capability is written under the host's name, so a reader sees
/// from the line that the screen has stopped being portable.
#[test]
fn a_hosts_own_capability_needs_that_host_declared() {
    let src = BASE.replace("tile(text: phrase(\"row\"), onTap: Go)", "wallet.avatar(of: n)");

    let without = errors(&src, true);
    assert!(
        without.iter().any(|m| m.contains("needs a `host` declaration")),
        "got {without:?}"
    );

    let declared = format!("host \"id.vaulet.wallet/1\"\n{src}");
    assert!(errors(&declared, true).is_empty(), "{:?}", errors(&declared, true));
}

#[test]
fn a_host_this_one_is_not_is_reported() {
    let src = format!("host \"com.alipay.wallet/2\"\n{BASE}");
    let msgs = errors(&src, true);
    assert!(msgs.iter().any(|m| m.contains("is not it")), "got {msgs:?}");
}

#[test]
fn a_prop_it_does_not_take_is_reported() {
    let src = BASE.replace("button(text: phrase(\"go\"), emphasis: primary,", "button(text: phrase(\"go\"), colour: red,");
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("has no `colour`")), "got {msgs:?}");
}

#[test]
fn a_word_outside_a_vocabulary_is_reported_with_the_words() {
    let src = BASE.replace("emphasis: primary", "emphasis: shouty");
    let msgs = errors(&src, false);
    assert!(
        msgs.iter().any(|m| m.contains("is not one of emphasis") && m.contains("primary")),
        "got {msgs:?}"
    );
}

#[test]
fn a_transition_outside_its_vocabulary_is_reported() {
    let src = BASE.replace("as: replace", "as: slide");
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("is not one of transition")), "got {msgs:?}");
}

#[test]
fn something_that_holds_no_children_is_reported() {
    let src = BASE.replace(
        "tile(text: phrase(\"row\"), onTap: Go)",
        "tile(text: phrase(\"row\")) { tile(text: phrase(\"row\")) }",
    );
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("holds no children")), "got {msgs:?}");
}

#[test]
fn a_screen_setting_nothing_provides_is_reported() {
    let src = BASE.replace("screen Done present: sheet", "screen Done colour: blue");
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("a screen has no `colour`")), "got {msgs:?}");
}

/// A package is several files, so "the first screen declared" would mean the
/// order files were read decides what somebody sees.
#[test]
fn more_than_one_screen_says_where_it_opens() {
    let src = BASE.replace("screen Home start: true", "screen Home");
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("none says `start: true`")), "got {msgs:?}");
}

#[test]
fn two_screens_may_not_both_start() {
    let src = BASE.replace("screen Done present: sheet", "screen Done start: true");
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("a package opens at one")), "got {msgs:?}");
}
