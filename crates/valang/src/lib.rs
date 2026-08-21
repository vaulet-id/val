//! The VAL front end.
//!
//! Source text in, typed AST out, having refused everything the language does
//! not allow. The checks live in `check` and each one is written to produce the
//! sentence the rule is taught by — see `examples/rejected.val` in this
//! repository, which is the checklist this crate is written against.

#![forbid(unsafe_code)]

pub mod ast;
pub mod check;
pub mod capability;
pub mod diag;
pub mod expand;
pub mod lex;
pub mod parse;
pub mod print;
pub mod report;
pub mod typeck;
pub mod types;

pub use diag::{Diagnostic, Severity};

/// Parse and check one source file. The host runs exactly this over the package
/// it received; a publisher's build passing proves nothing (§1).
pub fn analyse(src: &str) -> (ast::Program, Vec<Diagnostic>) {
    analyse_with(src, None)
}

/// The text bundle a package ships. Key to locale to template.
pub type TextBundle = std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>;

/// Read a text bundle off disk: `{ "locales": [...], "keys": { key: { locale:
/// text } } }`.
///
/// One reader, because two of them disagreed: `valc` read the shape above and
/// the packager read the top level as the keys, so it built a package whose
/// bundle had three entries called `_comment`, `locales` and `keys` — and every
/// program that says a word to anybody was refused for a missing language.
pub fn read_bundle(text: &str) -> Option<(TextBundle, Vec<String>)> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let locales = json["locales"]
        .as_array()?
        .iter()
        .filter_map(|l| l.as_str().map(str::to_string))
        .collect();
    let keys = json["keys"]
        .as_object()?
        .iter()
        .map(|(key, per_locale)| {
            let inner = per_locale
                .as_object()
                .map(|m| {
                    m.iter()
                        .filter_map(|(l, t)| t.as_str().map(|t| (l.clone(), t.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            (key.clone(), inner)
        })
        .collect();
    Some((keys, locales))
}

/// Analyse against the bundle as well as the code. They are signed as one
/// package, so checking them apart would mean signing a pairing nobody verified
/// (§9) — and every sentence a person reads is in there rather than in the
/// program.
pub fn analyse_with(src: &str, bundle: Option<(&TextBundle, &[String])>) -> (ast::Program, Vec<Diagnostic>) {
    analyse_against(src, bundle, &capability::Hosts::default())
}

/// Analyse against the host's catalogues as well.
///
/// A screen is checked against what the host published rather than against a
/// list inside this crate: the language does not define a catalogue, and a front
/// end that carried one would carry the first host's.
pub fn analyse_against(
    src: &str,
    bundle: Option<(&TextBundle, &[String])>,
    hosts: &capability::Hosts,
) -> (ast::Program, Vec<Diagnostic>) {
    analyse_fully(src, bundle, hosts)
}

/// Analyse against everything the host published: its catalogues and its
/// capabilities. Both are documents rather than lists in this crate, for the
/// same reason — a second host implements VAL, not the first host.
pub fn analyse_fully(
    src: &str,
    bundle: Option<(&TextBundle, &[String])>,
    hosts: &capability::Hosts,
) -> (ast::Program, Vec<Diagnostic>) {
    analyse_with_packages(src, bundle, hosts, &expand::Packages::default())
}

/// Analyse against the other packages this build can reach, as well.
///
/// Where a package comes from is not the language's question — a registry, a
/// directory, an editor's open projects are all answers — so the caller resolves
/// them and hands them over, the way it hands over the host registries.
pub fn analyse_with_packages(
    src: &str,
    bundle: Option<(&TextBundle, &[String])>,
    hosts: &capability::Hosts,
    packages: &expand::Packages,
) -> (ast::Program, Vec<Diagnostic>) {
    let (mut program, mut diagnostics) = parse::parse(src);
    // A package's own components become the host's catalogue before anything
    // else looks at a screen, so every later pass — the checks, the capability
    // report, the renderer — sees one kind of node.
    // Before expansion, because a component's call site and the parameter it
    // fills are both gone afterwards.
    diagnostics.extend(typeck::check_component_calls(&program));
    diagnostics.extend(expand::expand(&mut program, packages));
    // Before `check`, which asks whether a declared capability goes unused and
    // needs to know that drawing a video is a use of `media.video`.
    diagnostics.extend(capability::check(&mut program, hosts));
    diagnostics.extend(check::check(&program));
    diagnostics.extend(typeck::check_types_against(&program, hosts));
    if let Some((bundle, locales)) = bundle {
        diagnostics.extend(check::check_bundle(&program, bundle, locales, hosts));
    }
    diagnostics.sort_by_key(|d| (d.span.line, d.span.col));
    // The same sentence twice on one line is noise, and noise is how the one
    // that mattered gets skipped. Not `dedup_by`, which only sees neighbours:
    // two reports of one field can sit either side of a third message.
    let mut seen = std::collections::HashSet::new();
    diagnostics.retain(|d| seen.insert((d.span.line, d.message.clone())));
    (program, diagnostics)
}
