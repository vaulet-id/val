//! The VAL front end.
//!
//! Source text in, typed AST out, having refused everything the language does
//! not allow. The checks live in `check` and each one is written to produce the
//! sentence the rule is taught by — see `examples/rejected.val` in this
//! repository, which is the checklist this crate is written against.

pub mod ast;
pub mod check;
pub mod diag;
pub mod lex;
pub mod parse;
pub mod report;

pub use diag::{Diagnostic, Severity};

/// Parse and check one source file. The host runs exactly this over the package
/// it received; a publisher's build passing proves nothing (§1).
pub fn analyse(src: &str) -> (ast::Program, Vec<Diagnostic>) {
    let (program, mut diagnostics) = parse::parse(src);
    diagnostics.extend(check::check(&program));
    diagnostics.sort_by_key(|d| (d.span.line, d.span.col));
    // The same sentence twice on one line is noise, and noise is how the one
    // that mattered gets skipped.
    diagnostics.dedup_by(|a, b| a.span.line == b.span.line && a.message == b.message);
    (program, diagnostics)
}
