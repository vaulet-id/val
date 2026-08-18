//! Expanding a package's own components into the host's catalogue.
//!
//! A `component` is a name for a piece of the catalogue arranged a particular
//! way. It is not a primitive: by the time anything leaves the front end, every
//! node in every screen is a component the host ships, so there is no new
//! rendering path, no new consent surface, and nothing for a host to have to
//! learn.
//!
//! Expansion runs after the checks that would otherwise report the same mistake
//! once per call site.

use std::collections::BTreeMap;

use crate::ast::{Arg, ComponentDecl, Expr, Program, UiNode};
use crate::diag::{Diagnostic, Span};

/// What the host ships. A component may not take one of these names — that
/// would be a package overriding the catalogue rather than composing it.
pub const CATALOGUE: &[&str] =
    &["column", "row", "card", "section", "list", "button", "tab", "tabs"];

pub fn expand(program: &mut Program) -> Vec<Diagnostic> {
    let mut d = Vec::new();

    let by_name: BTreeMap<String, ComponentDecl> =
        program.components.iter().map(|c| (c.name.clone(), c.clone())).collect();

    for c in &program.components {
        if CATALOGUE.contains(&c.name.as_str()) {
            d.push(Diagnostic::error(
                c.span,
                format!("`{}` is a component the host ships — a package composes the catalogue rather than replacing it", c.name),
            ));
        }
        if !c.name.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()) {
            d.push(Diagnostic::error(
                c.span,
                format!("`{}` is a component this package declares, so it is capitalised — the lowercase names are the host's", c.name),
            ));
        }
    }

    // A cycle would expand forever, and totality is the one promise this
    // language cannot bend. Checked before anything is expanded, so a cyclic
    // package reports the cycle rather than running out of memory.
    for c in &program.components {
        if let Some(path) = cycle_from(&c.name, &by_name) {
            d.push(Diagnostic::error(
                c.span,
                format!("`{}` uses itself: {path}", c.name),
            ));
        }
    }
    if d.iter().any(|x| x.severity == crate::diag::Severity::Error) {
        return d;
    }

    let mut screens = std::mem::take(&mut program.screens);
    for s in &mut screens {
        let mut out = Vec::new();
        for node in std::mem::take(&mut s.tree) {
            out.extend(expand_node(node, &by_name, &mut d));
        }
        s.tree = out;
    }
    program.screens = screens;
    d
}

/// One node, and everything it stands for.
///
/// A component expands to its own tree with the arguments bound, so it may
/// stand for several nodes where the call site wrote one.
fn expand_node(
    node: UiNode,
    by_name: &BTreeMap<String, ComponentDecl>,
    d: &mut Vec<Diagnostic>,
) -> Vec<UiNode> {
    let Some(decl) = by_name.get(&node.kind) else {
        let UiNode { kind, args, lambda, children, span } = node;
        let children = children.into_iter().flat_map(|c| expand_node(c, by_name, d)).collect();
        return vec![UiNode { kind, args, lambda, children, span }];
    };

    let bound = bind(&node, decl, d);
    decl.tree
        .iter()
        .cloned()
        .map(|n| substitute(n, &bound))
        .flat_map(|n| expand_node(n, by_name, d))
        .collect()
}

/// Argument to parameter, by name.
///
/// A spread contributes the fields of the record it names. A parameter with no
/// argument and no `?` is a missing argument, reported here rather than as a
/// name that resolves to nothing inside the component.
fn bind(call: &UiNode, decl: &ComponentDecl, d: &mut Vec<Diagnostic>) -> BTreeMap<String, Expr> {
    let mut out = BTreeMap::new();

    for a in &call.args {
        if a.spread {
            // The fields are known from the record's type, which the type
            // checker has already established; what arrives here is the
            // expression, and each parameter reads its own field from it.
            for p in &decl.params {
                out.entry(p.name.clone()).or_insert_with(|| Expr::Member {
                    obj: Box::new(a.value.clone()),
                    name: p.name.clone(),
                    span: a.span,
                });
            }
            continue;
        }
        if let Some(name) = &a.name {
            out.insert(name.clone(), a.value.clone());
        }
    }

    for p in &decl.params {
        if !out.contains_key(&p.name) && !p.ty.optional {
            d.push(Diagnostic::error(
                call.span,
                format!("`{}` needs `{}`", decl.name, p.name),
            ));
        }
    }

    for a in &call.args {
        if let Some(name) = &a.name {
            if !decl.params.iter().any(|p| &p.name == name) {
                d.push(Diagnostic::error(
                    a.span,
                    format!("`{}` has no `{name}`", decl.name),
                ));
            }
        }
    }

    out
}

/// Replace a component's parameters with what the call site handed it.
fn substitute(node: UiNode, bound: &BTreeMap<String, Expr>) -> UiNode {
    UiNode {
        kind: node.kind,
        args: node
            .args
            .into_iter()
            .map(|a| Arg { value: replace(a.value, bound), ..a })
            .collect(),
        lambda: node.lambda,
        children: node.children.into_iter().map(|c| substitute(c, bound)).collect(),
        span: node.span,
    }
}

fn replace(e: Expr, bound: &BTreeMap<String, Expr>) -> Expr {
    match e {
        Expr::Ident { ref name, .. } => bound.get(name).cloned().unwrap_or(e),
        Expr::Member { obj, name, span } => {
            Expr::Member { obj: Box::new(replace(*obj, bound)), name, span }
        }
        Expr::Record { spread, fields, span } => Expr::Record {
            spread: spread.map(|s| Box::new(replace(*s, bound))),
            fields: fields.into_iter().map(|(k, v)| (k, replace(v, bound))).collect(),
            span,
        },
        Expr::Call { callee, args, span } => Expr::Call {
            callee,
            args: args.into_iter().map(|a| Arg { value: replace(a.value, bound), ..a }).collect(),
            span,
        },
        Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
            op,
            lhs: Box::new(replace(*lhs, bound)),
            rhs: Box::new(replace(*rhs, bound)),
            span,
        },
        Expr::Ternary { cond, then, other, span } => Expr::Ternary {
            cond: Box::new(replace(*cond, bound)),
            then: Box::new(replace(*then, bound)),
            other: Box::new(replace(*other, bound)),
            span,
        },
        other => other,
    }
}

/// The path back to a name, if it reaches itself.
fn cycle_from(start: &str, by_name: &BTreeMap<String, ComponentDecl>) -> Option<String> {
    fn walk(
        name: &str,
        start: &str,
        by_name: &BTreeMap<String, ComponentDecl>,
        seen: &mut Vec<String>,
    ) -> Option<String> {
        let decl = by_name.get(name)?;
        for used in uses(&decl.tree) {
            if used == start {
                seen.push(used.clone());
                return Some(seen.join(" → "));
            }
            if seen.contains(&used) {
                continue;
            }
            seen.push(used.clone());
            if let Some(path) = walk(&used, start, by_name, seen) {
                return Some(path);
            }
            seen.pop();
        }
        None
    }

    let mut seen = vec![start.to_string()];
    walk(start, start, by_name, &mut seen)
}

fn uses(tree: &[UiNode]) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(nodes: &[UiNode], out: &mut Vec<String>) {
        for n in nodes {
            out.push(n.kind.clone());
            walk(&n.children, out);
        }
    }
    walk(tree, &mut out);
    out
}

/// Where a span is not worth inventing.
pub const NOWHERE: Span = Span { line: 0, col: 0, len: 0 };
