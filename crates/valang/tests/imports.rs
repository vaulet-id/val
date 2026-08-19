//! Components that cross a package boundary.
//!
//! Resolved at build time and expanded into the importing package, so the thing
//! a host admits is one program. There is no linking step and nothing resolved
//! while somebody is looking at a screen.

use valang::capability::{Host, Hosts};
use valang::expand::Packages;

const CORE: &str = include_str!("../../../hosts/core.json");

/// A package that exports one component and keeps a helper to itself.
const KIT: &str = r#"
app "org.vaulet.ui"
version 1

capabilities {
}

export component MoneyCard(label: string, amount: string) {
  card {
    text: label
    Amount(amount: amount)
  }
}

component Amount(amount: string) {
  text(amount)
}
"#;

fn errors(src: &str, packages: &[&str]) -> Vec<String> {
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let loaded = Packages::of(packages.iter().map(|p| valang::analyse(p).0).collect());
    let (_, d) = valang::analyse_with_packages(src, None, &hosts, &loaded);
    d.into_iter()
        .filter(|x| x.severity == valang::diag::Severity::Error)
        .map(|x| x.message)
        .collect()
}

fn app(body: &str) -> String {
    format!(
        r#"
app "org.vaulet.shop"
version 1

capabilities {{
}}

state {{
  points: int default 0
}}

{body}
"#
    )
}

#[test]
fn an_exported_component_may_be_drawn_by_another_package() {
    let e = errors(
        &app(
            r#"
import "org.vaulet.ui/1" { MoneyCard }

@main
screen Home {
  column {
    MoneyCard(label: "Balance", amount: "120")
  }
}
"#,
        ),
        &[KIT],
    );
    assert!(e.is_empty(), "{e:?}");
}

/// Expanded in the package that wrote it, so the exporting package's private
/// helper never has to not collide with a name the importer chose.
#[test]
fn a_private_helper_comes_along_without_its_name() {
    let e = errors(
        &app(
            r#"
import "org.vaulet.ui/1" { MoneyCard }

component Amount(text: string) {
  section(text)
}

@main
screen Home {
  column {
    MoneyCard(label: "Balance", amount: "120")
    Amount(text: "mine")
  }
}
"#,
        ),
        &[KIT],
    );
    assert!(e.is_empty(), "{e:?}");
}

#[test]
fn what_a_package_keeps_to_itself_cannot_be_taken() {
    let e = errors(
        &app("import \"org.vaulet.ui/1\" { Amount }\n\n@main\nscreen Home {\n  column {\n    card(\"x\")\n  }\n}"),
        &[KIT],
    );
    assert!(e.iter().any(|m| m.contains("declares `Amount` and does not export it")), "{e:?}");
}

#[test]
fn a_package_the_build_cannot_reach_says_which_ones_it_has() {
    let e = errors(
        &app("import \"org.vaulet.ui/2\" { MoneyCard }\n\n@main\nscreen Home {\n  column {\n    card(\"x\")\n  }\n}"),
        &[KIT],
    );
    assert!(e.iter().any(|m| m.contains("`org.vaulet.ui/1`")), "{e:?}");
}

#[test]
fn a_name_that_is_both_imported_and_declared_is_refused() {
    let e = errors(
        &app(
            r#"
import "org.vaulet.ui/1" { MoneyCard }

component MoneyCard(label: string, amount: string) {
  card(label)
}

@main
screen Home {
  column {
    MoneyCard(label: "Balance", amount: "120")
  }
}
"#,
        ),
        &[KIT],
    );
    assert!(e.iter().any(|m| m.contains("is imported and this package also declares it")), "{e:?}");
}

/// `state.points` inside an exported component would resolve against whichever
/// package it was expanded into — a mistake neither author can see.
#[test]
fn an_exported_component_may_not_read_state() {
    let leaky = r#"
app "org.vaulet.leaky"
version 1

capabilities {
}

state {
  points: int default 0
}

export component Balance() {
  card(state.points)
}
"#;
    let e = errors(leaky, &[]);
    assert!(e.iter().any(|m| m.contains("is exported and reads `state`")), "{e:?}");
}

/// The author of the exporting package hears it at their own build, and the
/// importer hears it again — a package arrives as an artifact, not a promise.
#[test]
fn the_importer_is_told_too() {
    let leaky = r#"
app "org.vaulet.leaky"
version 1

capabilities {
}

state {
  points: int default 0
}

export component Balance() {
  card(state.points)
}
"#;
    let e = errors(
        &app("import \"org.vaulet.leaky/1\" { Balance }\n\n@main\nscreen Home {\n  column {\n    Balance()\n  }\n}"),
        &[leaky],
    );
    assert!(e.iter().any(|m| m.contains("is exported and reads `state`")), "{e:?}");
}

#[test]
fn export_marks_a_component_and_nothing_else() {
    let e = errors(&app("export action Go {\n  update {\n    points: 1\n  }\n}"), &[]);
    assert!(e.iter().any(|m| m.contains("`export` marks a component")), "{e:?}");
}

#[test]
fn an_import_lists_what_it_takes() {
    let e = errors(&app("import \"org.vaulet.ui/1\"\n"), &[KIT]);
    assert!(e.iter().any(|m| m.contains("an import lists what it takes")), "{e:?}");
}

/// The load-bearing claim. What an imported component draws lands in the
/// importing package's capability report: a person consents to one list, not to
/// one per package that happened to be involved.
#[test]
fn what_an_import_draws_is_declared_by_the_package_that_draws_it() {
    let media = r#"
app "org.vaulet.media"
version 1

capabilities {
  media.video
}

export component Clip(src: string) {
  video(of: src)
}
"#;
    let without = errors(
        &app("import \"org.vaulet.media/1\" { Clip }\n\n@main\nscreen Home {\n  column {\n    Clip(src: \"a.mp4\")\n  }\n}"),
        &[media],
    );
    assert!(
        without.iter().any(|m| m.contains("needs `media.video`")),
        "an imported capability went undeclared and nobody said so: {without:?}"
    );

    let with = errors(
        &app("import \"org.vaulet.media/1\" { Clip }\n\n@main\nscreen Home {\n  column {\n    Clip(src: \"a.mp4\")\n  }\n}")
            .replace("capabilities {\n}", "capabilities {\n  media.video\n}"),
        &[media],
    );
    assert!(with.is_empty(), "{with:?}");
}

/// An import that failed used to be reported twice: once for the import, and
/// once per call site as a name the host does not provide — which sends the
/// author looking in the host's registry for a component they know they wrote.
#[test]
fn a_failed_import_is_reported_once() {
    let e = errors(
        &app("import \"org.vaulet.ui/1\" { Amount }\n\n@main\nscreen Home {\n  column {\n    Amount(amount: \"120\")\n  }\n}"),
        &[KIT],
    );
    assert_eq!(e.len(), 1, "{e:?}");
}

/// A package of nothing but components had nothing checked at all: a screen is
/// what makes a component's body reachable, and a UI kit has no screens.
#[test]
fn a_package_with_no_screens_is_still_checked() {
    let e = errors(
        "app \"org.vaulet.badkit\"\nversion 1\n\ncapabilities {\n}\n\nexport component Broken(x: string) {\n  tabs {\n    text(x)\n  }\n}\n",
        &[],
    );
    assert!(e.iter().any(|m| m.contains("`tabs` is not something this host provides")), "{e:?}");
}

/// And a component a screen does draw is reported once, not twice — the body
/// and the call site are the same span.
#[test]
fn a_component_a_screen_draws_is_reported_once() {
    let e = errors(
        &app("component Broken(x: string) {\n  tabs {\n    text(x)\n  }\n}\n\n@main\nscreen Home {\n  column {\n    Broken(x: \"a\")\n  }\n}"),
        &[],
    );
    assert_eq!(e.iter().filter(|m| m.contains("`tabs`")).count(), 1, "{e:?}");
}

/// A component's body may name another component. Screens have theirs expanded
/// before anything checks them, so a body is the one place a component name
/// survives to be checked — and reporting it as a name the host does not have
/// sends the author to the wrong document.
#[test]
fn a_component_may_use_another_component() {
    let e = errors(
        "app \"org.vaulet.kit\"\nversion 1\n\ncapabilities {\n}\n\nexport component Outer(x: string) {\n  card {\n    text: x\n    Inner(x: x)\n  }\n}\n\ncomponent Inner(x: string) {\n  text(x)\n}\n",
        &[],
    );
    assert!(e.is_empty(), "{e:?}");
}
