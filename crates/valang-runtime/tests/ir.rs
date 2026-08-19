//! The interface, as the bytes a renderer receives.
//!
//! One screen resolves to one document, and this pins it. A renderer for iOS or
//! for the web is held to the same bytes as the Flutter one — which is the whole
//! reason the interface is data rather than a call into a toolkit.
//!
//! When this test fails, one of two things happened: the resolver changed what
//! it emits, or somebody edited the fixture. Both are worth stopping for.

use std::collections::BTreeMap;

use valang::capability::{Host as Registry, Hosts};
use valang_runtime::host::{Context, EffectRequest, Host, Verdict};
use valang_runtime::render::render;
use valang_runtime::value::Value;

const CORE: &str = include_str!("../../../hosts/core.json");

/// Resolved the way a host resolves it: against the registry the host
/// publishes, because that is what names a positional argument.
fn analysed() -> valang::ast::Program {
    let hosts = Hosts::of(vec![Registry::parse(CORE).expect("the core registry parses")]);
    let (program, d) = valang::analyse_fully(SRC, None, &hosts);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");
    program
}

const SRC: &str = r#"
app "example.ir"
version 1

capabilities {
}

state {
  points: int default 1250
}

action Go {
  update {
    points: 0
  }
}

@main

screen Home {
  title: "Your points"

  column {
    width: fill
    padding: 24
    gap: 16

    card(phrase("You have {points} points", points: state.points)) {
      style: title
      color: foreground.primary
    }

    tile("A row") {
      detail: "And its line"
      icon: money
      trailing: badge
    }

    button("Continue") {
      width: fill
      variant: primary
      onTap: Go
      role: button
    }
  }
}
"#;

struct Bare;

impl Host for Bare {
    fn context(&self) -> Context {
        Context { time_now: 0, random_uuid: "0".into() }
    }
    fn credential(&self, _ty: &str, _policy: Option<&str>) -> Option<BTreeMap<String, Value>> {
        None
    }
    fn decide(&self, _effects: &[EffectRequest]) -> Verdict {
        Verdict::Approved
    }
    fn sign(&self, _bytes: &[u8]) -> Vec<u8> {
        vec![0; 64]
    }
    fn device_key(&self) -> Vec<u8> {
        vec![0; 32]
    }
}

/// One line per node, deepest last, with every argument in name order. Not JSON:
/// a fixture somebody has to read is worth more than one a serialiser produced,
/// and the ordering is the part being pinned.
fn draw(c: &valang_runtime::render::Component, depth: usize, out: &mut String) {
    out.push_str(&"  ".repeat(depth));
    out.push_str(&c.kind);
    for (name, value) in &c.args {
        out.push_str(&format!(" {name}={value}"));
    }
    out.push('\n');
    for child in &c.children {
        draw(child, depth + 1, out);
    }
}

const GOLDEN: &str = r#"title text="Your points"
column gap=16 padding=24 width="fill"
  card color="foreground.primary" points=1250 style="title" text="You have {points} points"
  tile detail="And its line" icon="money" text="A row" trailing="badge"
  button onTap="Go" role="button" text="Continue" variant="primary" width="fill"
"#;

#[test]
fn one_screen_resolves_to_one_document() {
    let program = analysed();
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());
    let screen = render(&program, "Home", &state, &Bare).expect("the screen resolves");

    let mut out = String::new();
    if let Some(title) = &screen.title {
        draw(title, 0, &mut out);
    }
    for node in &screen.tree {
        draw(node, 0, &mut out);
    }

    assert_eq!(out, GOLDEN, "\n--- got ---\n{out}\n--- want ---\n{GOLDEN}");
}

/// Twice over, the same bytes. A renderer that receives a different document for
/// the same screen cannot be held to anything.
#[test]
fn resolving_twice_gives_the_same_document() {
    let program = analysed();
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());

    let once = render(&program, "Home", &state, &Bare).expect("resolves");
    let twice = render(&program, "Home", &state, &Bare).expect("resolves");
    assert_eq!(once, twice);
}
