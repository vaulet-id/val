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

@main
screen Home {
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

/// A closed vocabulary is one the host has to understand — an icon it draws, a
/// transition it performs. A word outside it is a mistake, not a preference.
#[test]
fn a_word_outside_a_closed_vocabulary_is_reported_with_the_words() {
    let src = BASE.replace("emphasis: primary", "state: shouty");
    let msgs = errors(&src, false);
    assert!(
        msgs.iter().any(|m| m.contains("is not one of state") && m.contains("busy")),
        "got {msgs:?}"
    );
}

/// An open vocabulary is this design system's suggestion. A Micro App is
/// somebody's own product, and a value of its own is a customer rather than an
/// attack — so a token guides and does not fence.
#[test]
fn an_open_vocabulary_takes_a_value_of_your_own() {
    let src = BASE.replace(
        "emphasis: primary",
        "emphasis: primary, background: \"#EEF7F1\", padding: 24",
    );
    assert!(errors(&src, false).is_empty(), "{:?}", errors(&src, false));
}

/// It still catches a misspelt token: a dotted name that is not one of them is
/// not something an application meant to invent.
#[test]
fn a_misspelt_token_is_reported_even_where_the_vocabulary_is_open() {
    let src = BASE.replace("emphasis: primary", "color: foreground.primry");
    let msgs = errors(&src, false);
    assert!(
        msgs.iter().any(|m| m.contains("is not a colorToken this host has")),
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
/// order files were read decides what somebody sees. Asked of one screen as
/// well as of several: one screen today is two tomorrow, and the day it becomes
/// two is not the day to find out which one opens.
#[test]
fn a_package_with_screens_says_where_it_opens() {
    let src = BASE.replace("@main\nscreen Home", "screen Home");
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("no screen is marked `@main`")), "got {msgs:?}");
}

#[test]
fn two_screens_may_not_both_start() {
    let src = BASE.replace("screen Done present: sheet", "@main\nscreen Done");
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("a package opens at one")), "got {msgs:?}");
}

/// Drawing something privileged is not permission to do it. A person consents
/// to a list of capabilities, and a component that quietly carried one would be
/// a way to have that list say less than the application does.
#[test]
fn drawing_something_privileged_needs_the_capability_declared() {
    let src = BASE.replace("tile(text: phrase(\"row\"), onTap: Go)", "video(\"intro.mp4\")");
    let msgs = errors(&src, false);
    assert!(
        msgs.iter().any(|m| m.contains("needs `media.video`") && m.contains("not the same as being allowed to")),
        "got {msgs:?}"
    );

    let allowed = src.replace("capabilities {\n}", "capabilities {\n  media.video\n}");
    assert!(errors(&allowed, false).is_empty(), "{:?}", errors(&allowed, false));
}
