//! The capability report (§7): what this application can do to the person,
//! derived from the code rather than written by its author.
//!
//! A publisher ships a copy for review and it is evidence of nothing — the host
//! recomputes this and refuses on mismatch, because the host owns the checker.

use std::collections::BTreeSet;
use std::fmt;

use crate::ast::*;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub app: String,
    pub version: String,
    pub reads: BTreeSet<String>,
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

/// Which claims are read, not only which credential. §7's example is
/// `PurchaseReceipt.amount, PurchaseReceipt.purchased_at` — a person deciding
/// whether to install this is owed the fields, since "reads your receipts" and
/// "reads the amount and the date" are different sentences.
fn claim_paths(p: &Program) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let policy_subject = |name: &str| {
        p.trusts.iter().find(|t| t.name == name).map(|t| t.subject_type.clone())
    };

    // `checked.claims.amount`, where `checked` came from `x with Policy`.
    let mut verified: std::collections::BTreeMap<String, String> = Default::default();
    let visit = |e: &Expr, verified: &mut std::collections::BTreeMap<String, String>, out: &mut BTreeSet<String>| {
        e.walk(&mut |inner| {
            if let Expr::Member { obj, name, .. } = inner {
                if let Expr::Member { obj: root, name: claims, .. } = obj.as_ref() {
                    if claims == "claims" {
                        if let Some(base) = root.path() {
                            if let Some(ty) = verified.get(&base) {
                                out.insert(format!("{ty}.{name}"));
                            }
                        }
                    }
                }
            }
        })
    };

    for a in &p.actions {
        for block in &a.phases {
            let mut flat: Vec<&Stmt> = Vec::new();
            walk_stmts(&block.stmts, &mut flat);
            for s in flat {
                match s {
                    Stmt::Let { name, value, .. } => {
                        if let Expr::With { policy, .. } = value {
                            if let Some(ty) = policy_subject(policy) {
                                verified.insert(name.clone(), ty);
                            }
                        }
                        visit(value, &mut verified, &mut out);
                    }
                    Stmt::Expr { value, .. } | Stmt::Patch { value, .. } => visit(value, &mut verified, &mut out),
                    Stmt::Effect { args, body, .. } => {
                        for arg in args {
                            visit(&arg.value, &mut verified, &mut out);
                        }
                        for inner in body {
                            if let Stmt::Effect { args, .. } = inner {
                                for arg in args {
                                    visit(&arg.value, &mut verified, &mut out);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

/// `checked.claims.country` is what the author wrote; `NationalId.country` is
/// what the person is being asked about. The report is read by the second one.
fn in_credential_terms(p: &Program, expr: &str) -> String {
    let Some((base, rest)) = expr.split_once(".claims.") else { return expr.to_string() };
    for a in &p.actions {
        for block in &a.phases {
            let mut flat: Vec<&Stmt> = Vec::new();
            walk_stmts(&block.stmts, &mut flat);
            for s in flat {
                if let Stmt::Let { name, value: Expr::With { policy, .. }, .. } = s {
                    if name == base {
                        if let Some(t) = p.trusts.iter().find(|t| t.name == *policy) {
                            return format!("{}.{rest}", t.subject_type);
                        }
                    }
                }
            }
        }
    }
    expr.to_string()
}

pub fn report(p: &Program) -> Report {
    let mut r = Report {
        app: p.app.clone().unwrap_or_default(),
        version: p.version.clone().unwrap_or_default(),
        ..Default::default()
    };

    for c in &p.capabilities {
        if c.name == "api.query" {
            for a in &c.args {
                if a.name.as_deref() == Some("audience") {
                    if let Expr::Str { value, .. } = &a.value {
                        r.audiences.insert(value.clone());
                    }
                }
            }
        }
    }

    for s in &p.screens {
        for d in &s.data {
            match &d.source {
                DataSource::Credentials { ty, policy, .. } => {
                    r.reads.insert(match policy {
                        Some(p) => format!("{ty} under {p}"),
                        None => format!("{ty} — unverified"),
                    });
                }
                // The audience is the one fixed in the manifest — `api.query`
                // above. The head of the call is an operation on it, and
                // reporting it as a second party to talk to would be a lie
                // about how many people see this.
                DataSource::Query { .. } | DataSource::Unknown => {}
            }
        }
    }

    for a in &p.actions {
        for block in &a.phases {
            collect(&block.stmts, p, &mut r);
            if block.phase == Phase::Update {
                let mut flat: Vec<&Stmt> = Vec::new();
                walk_stmts(&block.stmts, &mut flat);
                for s in flat {
                    if let Stmt::Patch { path, .. } = s {
                        r.writes.insert(path.join("."));
                    }
                }
            }
        }
    }

    r.discloses = r.discloses.iter().map(|d| in_credential_terms(p, d)).collect();

    // Narrow `reads` to the claims actually touched, where they are known.
    let claims = claim_paths(p);
    if !claims.is_empty() {
        let mut narrowed = BTreeSet::new();
        for entry in &r.reads {
            let (ty, rest) = entry.split_once(' ').unwrap_or((entry.as_str(), ""));
            let touched: Vec<&String> = claims.iter().filter(|c| c.starts_with(&format!("{ty}."))).collect();
            if touched.is_empty() {
                narrowed.insert(entry.clone());
            } else {
                narrowed.insert(format!(
                    "{} {rest}",
                    touched.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
        }
        r.reads = narrowed;
    }

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
fn walk_stmts<'a>(stmts: &'a [Stmt], out: &mut Vec<&'a Stmt>) {
    Stmt::flatten(stmts, out)
}

fn collect(stmts: &[Stmt], p: &Program, r: &mut Report) {
    let mut flat: Vec<&Stmt> = Vec::new();
    walk_stmts(stmts, &mut flat);
    for s in flat {
        match s {
            Stmt::Binding { ty, .. } if ty.name == "Credential" => {
                if let Some(inner) = ty.args.first() {
                    // A policy is named in `verify`; if one names this type, say so.
                    let policy = p
                        .trusts
                        .iter()
                        .find(|t| t.subject_type == inner.name)
                        .map(|t| t.name.clone());
                    r.reads.insert(match policy {
                        Some(pn) => format!("{} under {pn}", inner.name),
                        None => format!("{} — unverified", inner.name),
                    });
                }
            }
            Stmt::Data { source, .. } => match source {
                DataSource::Credentials { ty, policy, .. } => {
                    r.reads.insert(match policy {
                        Some(pn) => format!("{ty} under {pn}"),
                        None => format!("{ty} — unverified"),
                    });
                }
                DataSource::Query { .. } | DataSource::Unknown => {}
            },
            Stmt::Effect { name, args, body, .. } => {
                match name.as_str() {
                    "disclose" => {
                        r.discloses.insert(args.first().and_then(|a| a.value.path()).unwrap_or_else(|| "—".into()));
                    }
                    "prove" => {
                        r.proves.insert(render(args.first().map(|a| &a.value)));
                    }
                    "credential.issue" => {
                        if let Some(a) = args.first() {
                            r.issues.insert(match &a.value {
                                Expr::Call { callee, .. } => callee.path().unwrap_or_else(|| "?".into()),
                                Expr::Record { .. } => "record".into(),
                                other => other.path().unwrap_or_else(|| "?".into()),
                            });
                        }
                    }
                    "payment.request" => {
                        r.payments.insert(
                            args.iter()
                                .map(|a| match &a.name {
                                    Some(n) => format!("{n}: {}", render(Some(&a.value))),
                                    None => render(Some(&a.value)),
                                })
                                .collect::<Vec<_>>()
                                .join(", "),
                        );
                    }
                    _ => {}
                }
                collect(body, p, r);
            }
            _ => {}
        }
    }
}

fn render(e: Option<&Expr>) -> String {
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
