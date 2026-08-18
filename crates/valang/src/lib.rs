//! The VAL front end.
//!
//! Source text in, typed AST out, having refused everything the language does
//! not allow. The checks live in `check` and each one is written to produce the
//! sentence the rule is taught by — see `examples/rejected.val` in this
//! repository, which is the checklist this crate is written against.

pub mod ast;
pub mod check;
pub mod catalogue;
pub mod interface;
pub mod diag;
pub mod expand;
pub mod lex;
pub mod parse;
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

/// Analyse against the bundle as well as the code. They are signed as one
/// package, so checking them apart would mean signing a pairing nobody verified
/// (§9) — and every sentence a person reads is in there rather than in the
/// program.
pub fn analyse_with(src: &str, bundle: Option<(&TextBundle, &[String])>) -> (ast::Program, Vec<Diagnostic>) {
    analyse_against(src, bundle, &catalogue::Catalogues::default())
}

/// Analyse against the host's catalogues as well.
///
/// A screen is checked against what the host published rather than against a
/// list inside this crate: the language does not define a catalogue, and a front
/// end that carried one would carry the first host's.
pub fn analyse_against(
    src: &str,
    bundle: Option<(&TextBundle, &[String])>,
    catalogues: &catalogue::Catalogues,
) -> (ast::Program, Vec<Diagnostic>) {
    analyse_fully(src, bundle, catalogues, &interface::Interfaces::default())
}

/// Analyse against everything the host published: its catalogues and its
/// capabilities. Both are documents rather than lists in this crate, for the
/// same reason — a second host implements VAL, not the first host.
pub fn analyse_fully(
    src: &str,
    bundle: Option<(&TextBundle, &[String])>,
    catalogues: &catalogue::Catalogues,
    interfaces: &interface::Interfaces,
) -> (ast::Program, Vec<Diagnostic>) {
    let (mut program, mut diagnostics) = parse::parse(src);
    // A package's own components become the host's catalogue before anything
    // else looks at a screen, so every later pass — the checks, the capability
    // report, the renderer — sees one kind of node.
    diagnostics.extend(expand::expand(&mut program));
    diagnostics.extend(check::check(&program));
    diagnostics.extend(typeck::check_types(&program));
    if let Some((bundle, locales)) = bundle {
        diagnostics.extend(check::check_bundle(&program, bundle, locales));
    }
    diagnostics.extend(catalogue::check_screens(&program, catalogues));
    diagnostics.extend(interface::check(&program, interfaces));
    diagnostics.sort_by_key(|d| (d.span.line, d.span.col));
    // The same sentence twice on one line is noise, and noise is how the one
    // that mattered gets skipped. Not `dedup_by`, which only sees neighbours:
    // two reports of one field can sit either side of a third message.
    let mut seen = std::collections::HashSet::new();
    diagnostics.retain(|d| seen.insert((d.span.line, d.message.clone())));
    (program, diagnostics)
}
