//! A screen is checked against the catalogue the host published.
//!
//! The documents under `catalogues/` are the fixtures, which is the point: they
//! are what a host ships, and a front end with its own list would be a front end
//! with a favourite host.

use valang::catalogue::{Catalogue, Catalogues};

const CORE: &str = include_str!("../../../catalogues/core.json");
const VAULET: &str = include_str!("../../../catalogues/vaulet.json");

fn cats(extra: bool) -> Catalogues {
    let mut loaded = vec![Catalogue::parse(CORE).expect("the core profile parses")];
    if extra {
        loaded.push(Catalogue::parse(VAULET).expect("vaulet's catalogue parses"));
    }
    Catalogues::of(loaded)
}

fn errors(src: &str, extra: bool) -> Vec<String> {
    let (_, d) = valang::analyse_against(src, None, &cats(extra));
    d.iter()
        .filter(|x| x.severity == valang::diag::Severity::Error)
        .map(|x| x.message.clone())
        .collect()
}

const BASE: &str = r#"
app "example.catalogue"
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

screen Home {
  column {
    tile(text: sentence("row"), onTap: Go)
    button(text: sentence("go"), emphasis: primary, onTap: Go)
  }
}
"#;

#[test]
fn the_core_profile_is_enough_on_its_own() {
    assert!(errors(BASE, false).is_empty(), "{:?}", errors(BASE, false));
}

#[test]
fn a_component_no_catalogue_has_is_reported() {
    let src = BASE.replace("tile(text: sentence(\"row\"), onTap: Go)", "carousel(text: sentence(\"row\"))");
    let msgs = errors(&src, false);
    assert!(
        msgs.iter().any(|m| m.contains("not in this catalogue")),
        "got {msgs:?}"
    );
}

/// A host's own component is written under the host's name, so a reader sees
/// from the line that the screen has stopped being portable.
#[test]
fn a_hosts_own_component_needs_that_catalogue_declared() {
    let src = BASE.replace(
        "tile(text: sentence(\"row\"), onTap: Go)",
        "wallet.avatar(of: state.n)",
    );

    let without = errors(&src, true);
    assert!(
        without.iter().any(|m| m.contains("needs a `catalogue` declaration")),
        "got {without:?}"
    );

    let declared = format!("catalogue \"id.vaulet.wallet/1\"\n{src}");
    assert!(errors(&declared, true).is_empty(), "{:?}", errors(&declared, true));
}

#[test]
fn a_catalogue_this_host_does_not_have_is_reported() {
    let src = format!("catalogue \"com.alipay.wallet/2\"\n{BASE}");
    let msgs = errors(&src, true);
    assert!(
        msgs.iter().any(|m| m.contains("does not have it")),
        "got {msgs:?}"
    );
}

#[test]
fn a_prop_the_component_does_not_take_is_reported() {
    let src = BASE.replace("button(text: sentence(\"go\"), emphasis: primary,", "button(text: sentence(\"go\"), colour: red,");
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("has no `colour`")), "got {msgs:?}");
}

/// A vocabulary is closed, and the message lists the words rather than saying
/// only that this one is wrong.
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
fn a_component_that_holds_no_children_is_reported() {
    let src = BASE.replace(
        "tile(text: sentence(\"row\"), onTap: Go)",
        "tile(text: sentence(\"row\")) { tile(text: sentence(\"row\")) }",
    );
    let msgs = errors(&src, false);
    assert!(msgs.iter().any(|m| m.contains("holds no children")), "got {msgs:?}");
}
