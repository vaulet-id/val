//! The code and the bundle, which are signed as one thing.
//!
//! Every sentence a person reads lives in the bundle, and every value in one
//! comes from the code. Neither half is allowed to be a guess: what the two
//! disagree about is what somebody sees on a screen in a language nobody on the
//! team reads.

use std::collections::BTreeMap;
use valang::capability::{Host, Hosts};

const CORE: &str = include_str!("../../../hosts/core.json");

fn bundle(entries: &[(&str, &[(&str, &str)])]) -> valang::TextBundle {
    entries
        .iter()
        .map(|(key, per)| {
            (
                key.to_string(),
                per.iter().map(|(l, t)| (l.to_string(), t.to_string())).collect::<BTreeMap<_, _>>(),
            )
        })
        .collect()
}

fn errors(src: &str, keys: valang::TextBundle, locales: &[&str]) -> Vec<String> {
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let locales: Vec<String> = locales.iter().map(|l| l.to_string()).collect();
    valang::analyse_fully(src, Some((&keys, &locales)), &hosts)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::diag::Severity::Error)
        .map(|d| d.message)
        .collect()
}

/// The warnings, which is what the unread-key rule produces.
fn warnings(src: &str, keys: &valang::TextBundle, locales: &[String]) -> Vec<String> {
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    valang::analyse_fully(src, Some((keys, locales)), &hosts)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::diag::Severity::Warning)
        .map(|d| d.message)
        .collect()
}

fn app(tree: &str) -> String {
    format!(
        "app \"x.y\"\nversion \"1.0.0\"\n\ncapabilities {{\n}}\n\nstate {{\n  points: int default 0\n}}\n\n@main\nscreen Home {{\n  column {{\n{tree}\n  }}\n}}\n"
    )
}

/// A slot named after a prop the node already has.
#[test]
fn a_slot_does_not_take_a_props_place() {
    let src = app("    card(text: phrase(\"in {color}\", color: \"red\"), color: accent)");
    let e = errors(src.as_str(), bundle(&[]), &["en"]);
    assert!(!e.is_empty(), "a slot and a prop shared a name and nothing was said: {e:?}");
}

/// Two phrases on one node, each with a slot of the same name.
#[test]
fn two_phrases_on_one_node_keep_their_own_slots() {
    let src = app(
        "    banner(text: phrase(\"a {n}\", n: 1), detail: phrase(\"b {n}\", n: 2), icon: money)",
    );
    let e = errors(src.as_str(), bundle(&[]), &["en"]);
    assert!(!e.is_empty(), "two slots with one name became one and nothing was said: {e:?}");
}

/// A locale the manifest promises, missing a key.
#[test]
fn a_promised_locale_has_every_key() {
    let src = app("    card(text: \"greeting\")");
    let e = errors(src.as_str(), bundle(&[("greeting", &[("en", "hello")])]), &["en", "th"]);
    assert!(e.iter().any(|m| m.contains("th")), "a locale was promised and left empty: {e:?}");
}

/// A key in the bundle that nothing uses. Said rather than refused: an
/// unused capability is consent somebody gave for nothing, and an unread key is
/// waste.
#[test]
fn a_key_nothing_uses_is_said_so() {
    let src = app("    card(text: \"greeting\")");
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let keys = bundle(&[
        ("greeting", &[("en", "hello"), ("th", "หวัดดี")]),
        ("orphan", &[("en", "nobody"), ("th", "ไม่มีใคร")]),
    ]);
    let locales = vec!["en".to_string(), "th".to_string()];
    let said: Vec<String> = valang::analyse_fully(src.as_str(), Some((&keys, &locales)), &hosts)
        .1
        .into_iter()
        .map(|d| d.message)
        .collect();
    assert!(
        said.iter().any(|m| m.contains("`orphan` is in the text bundle and nothing reads it")),
        "a key nothing reads was signed into the package: {said:?}"
    );
}

/// A key whose sentence has a slot the code does not fill.
#[test]
fn a_sentence_with_a_slot_nobody_fills() {
    let src = app("    card(text: \"greeting\")");
    let e = errors(
        src.as_str(),
        bundle(&[("greeting", &[("en", "hello {name}"), ("th", "หวัดดี {name}")])]),
        &["en", "th"],
    );
    assert!(!e.is_empty(), "a sentence had a hole nobody filled: {e:?}");
}

/// A second text prop. Only `text:` was ever collected, so a key written in
/// `detail:` was checked in no language at all.
#[test]
fn every_text_prop_is_checked_against_the_bundle() {
    let src = app("    banner(text: \"headline\", detail: \"Words in place\", icon: money)");
    let e = errors(
        src.as_str(),
        bundle(&[("headline", &[("en", "hi"), ("th", "หวัดดี")])]),
        &["en", "th"],
    );
    assert!(
        e.iter().any(|m| m.contains("Words in place")),
        "a sentence in `detail:` was never checked: {e:?}"
    );
}

/// **A key the host reads is not a key nobody reads.**
///
/// A wallet listing applications has to call one something, and every other
/// sentence a person sees is already a key in this bundle — so the name is one
/// more rather than a field in the package format. The compiler cannot see that
/// reader, so it is told; without that it warned about the one key every
/// package is meant to carry.
#[test]
fn a_key_the_host_reads_is_not_reported_as_unread() {
    let src = r#"
app "x.y"
version "1.0.0"
capabilities { }
state { n: int default 0 }
@main
screen Home {
  title: phrase("homeTitle")
  column {
    card(text: phrase("homeTitle"))
  }
}
"#;
    let bundle = bundle(&[
        ("homeTitle", &[("en", "Home"), ("th", "หน้าหลัก")]),
        ("appName", &[("en", "Something"), ("th", "อะไรสักอย่าง")]),
        ("nobodyReads", &[("en", "Waste"), ("th", "เปล่าประโยชน์")]),
    ]);
    let said = warnings(src, &bundle, &["en".to_string(), "th".to_string()]);
    assert!(
        said.iter().any(|w| w.contains("nobodyReads")),
        "an unread key is still reported: {said:?}"
    );
    assert!(
        !said.iter().any(|w| w.contains("appName")),
        "the host reads this one: {said:?}"
    );
}
