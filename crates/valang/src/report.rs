//! The capability report (§7): what this application can do to the person,
//! derived from the code rather than written by its author.
//!
//! A publisher ships a copy for review and it is evidence of nothing — the host
//! derives this one and refuses on mismatch.
//!
//! **What it does to the person is not filled in here.** That comes from the
//! compiled module, whose import section is the whole of what it can reach —
//! see `valang_wasm::report_of`, which is what a caller wants. This half is
//! what the front end is the authority on: which application it is, which hosts
//! it needs, what it exports and what it takes from other packages.
//!
//! There were two routes for a while and they disagreed the first time anybody
//! compared them: the walk over the source said a program proving an age read
//! the birthdate, about a module that has no way to reach it. One route, and
//! this is the half that stayed.

use std::collections::BTreeSet;
use std::fmt;

use crate::ast::*;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub app: String,
    pub version: String,
    pub reads: BTreeSet<String>,
    /// Credentials checked against a policy and **not read**. A person is owed
    /// the difference: an application that checks your national ID against the
    /// government's key and reads nothing off it is not an application that
    /// reads your national ID.
    pub checks: BTreeSet<String>,
    pub discloses: BTreeSet<String>,
    pub proves: BTreeSet<String>,
    pub issues: BTreeSet<String>,
    pub audiences: BTreeSet<String>,
    pub payments: BTreeSet<String>,
    pub writes: BTreeSet<String>,
    /// The hosts this package needs beyond the registry every one of them
    /// provides. Empty means it runs on any of them, which is a sentence a
    /// person deciding to install it is owed as much as the rest.
    pub hosts: BTreeSet<String>,
    /// The addresses a link from outside can reach. A package that can be
    /// opened by a link is a package whose screens somebody else can point at,
    /// which is a sentence the person installing it is owed.
    pub addresses: BTreeSet<String>,
    /// What this package exports, with the parameters each one takes.
    ///
    /// The surface somebody else's build depends on. Changing a parameter here
    /// breaks a package that is not yours and whose author is not in the room,
    /// so it belongs in the report rather than only in the source: a publisher
    /// can see what they are about to change, and diff it against what they
    /// published.
    pub exports: BTreeSet<String>,
    /// What this package takes from others, as written.
    pub imports: BTreeSet<String>,
    pub irreversible: bool,
}

pub fn report(p: &Program) -> Report {
    let mut r = Report {
        app: p.app.clone().unwrap_or_default(),
        version: p.version.clone().unwrap_or_default(),
        ..Default::default()
    };

    r.hosts = p
        .hosts
        .iter()
        .filter(|h| !h.starts_with(crate::capability::CORE))
        .cloned()
        .collect();
    r.addresses = p
        .screens
        .iter()
        .flat_map(|s| &s.settings)
        .filter(|a| a.name.as_deref() == Some("address"))
        .filter_map(|a| match &a.value {
            Expr::Str { value, .. } => Some(value.clone()),
            _ => None,
        })
        .collect();
    r.exports = p
        .components
        .iter()
        .filter(|c| c.exported)
        .map(|c| {
            let params: Vec<String> = c
                .params
                .iter()
                .map(|f| format!("{}: {}", f.name, f.ty.written()))
                .collect();
            format!("{}({})", c.name, params.join(", "))
        })
        .collect();

    r.imports = p
        .imports
        .iter()
        .map(|i| format!("{} {{ {} }}", i.package, i.names.join(", ")))
        .collect();

    r.irreversible = !r.discloses.is_empty() || !r.proves.is_empty() || !r.payments.is_empty();
    r
}

/// Every statement, including the ones inside a branch.
///
/// A flat loop over a phase's statements is a report that stops at the first
/// `if`: an effect written in one branch never appeared, so the consent sheet —
/// which is a rendering of this report — did not mention something the
/// application does.
/// `checked.claims.country` is what the author wrote; `NationalId.country` is
/// what the person is asked about, and every sheet is read by the second one.
///
/// **One place.** The compiler names an import with it, the evaluator names a
/// statement it hands to the host with it, and the report renders with it — and
/// when there were two, a proof came out named after a local binding in one and
/// after the credential in the other, which is two different statements in two
/// records of the same run.
pub fn in_credential_terms(p: &Program, said: &str) -> String {
    let mut bound = Vec::new();
    for a in &p.actions {
        for block in &a.phases {
            verified_in(&block.stmts, &mut bound);
        }
    }
    let mut out = said.to_string();
    for (name, policy) in bound {
        let Some(t) = p.trusts.iter().find(|t| t.name == policy) else { continue };
        out = out.replace(&format!("{name}.claims."), &format!("{}.", t.subject_type));
    }
    out
}

/// Every `x with Policy` in a body, including the ones inside a branch — a
/// binding made in one names a credential the same way.
fn verified_in(stmts: &[Stmt], out: &mut Vec<(String, String)>) {
    for s in stmts {
        match s {
            Stmt::Let { name, value: Expr::With { policy, .. }, .. } => {
                out.push((name.clone(), policy.clone()))
            }
            Stmt::If { then, other, .. } => {
                verified_in(then, out);
                verified_in(other, out);
            }
            Stmt::Effect { body, .. } => verified_in(body, out),
            _ => {}
        }
    }
}

/// How a predicate or an argument is written down where a person reads it.
///
/// Public because the back end names an import with it — `prove:` carries the
/// statement it proves, and the two have to be the same string or the report a
/// wallet derives from a module would not be the report this says.
pub fn render(e: Option<&Expr>) -> String {
    match e {
        None => "—".into(),
        Some(Expr::Num { value, .. }) => value.to_string(),
        Some(Expr::Str { value, .. }) => format!("\"{value}\""),
        Some(Expr::Binary { op, lhs, rhs, .. }) => format!("{} {op} {}", render(Some(lhs)), render(Some(rhs))),
        Some(Expr::Call { callee, .. }) => format!("{}(…)", callee.path().unwrap_or_else(|| "?".into())),
        Some(other) => other.path().unwrap_or_else(|| "…".into()),
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let row = |f: &mut fmt::Formatter<'_>, label: &str, vals: &BTreeSet<String>| -> fmt::Result {
            if vals.is_empty() {
                writeln!(f, "{label:<14} —")
            } else {
                for (i, v) in vals.iter().enumerate() {
                    if i == 0 {
                        writeln!(f, "{label:<14} {v}")?;
                    } else {
                        writeln!(f, "{:<14} {v}", "")?;
                    }
                }
                Ok(())
            }
        };
        writeln!(f, "{} v{}", self.app, self.version)?;
        row(f, "reads", &self.reads)?;
        row(f, "checks", &self.checks)?;
        row(f, "discloses", &self.discloses)?;
        row(f, "proves", &self.proves)?;
        row(f, "issues", &self.issues)?;
        row(f, "talks to", &self.audiences)?;
        row(f, "moves money", &self.payments)?;
        row(f, "writes state", &self.writes)?;
        row(f, "runs only on", &self.hosts)?;
        row(f, "reachable at", &self.addresses)?;
        row(f, "exports", &self.exports)?;
        row(f, "imports", &self.imports)?;
        writeln!(f, "{:<14} {}", "irreversible", if self.irreversible { "yes" } else { "none" })
    }
}
