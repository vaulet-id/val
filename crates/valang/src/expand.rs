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

use crate::ast::{Arg, ComponentDecl, CredentialDecl, Expr, Program, UiNode};
use crate::diag::{Diagnostic, Span};

/// A component this package declares may not take a name the catalogue uses —
/// that would be overriding the host rather than composing it.
///
/// The catalogue itself is a document the host publishes; what is left here is
/// only the shape of the name. A component is capitalised and a catalogue's
/// components are not, so the two cannot collide in the first place, and this
/// check is what makes that rule enforceable rather than conventional.
pub fn is_catalogue_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

/// The call that carries a phrase's values.
///
/// `phrase("You have {points} points", points: total)`. The first argument is the
/// words themselves, or a key in the text bundle where the package has one — an
/// application in one language never meets a key, and the wrapper is there to
/// hold the values rather than to look anything up.
///
/// A component takes the finished phrase and never learns which it was or how many
/// values it took, so one component renders any of them. A component that took
/// the key and filled the values itself could only be used with lines that
/// happened to name the same ones.
pub const PHRASE: &str = "phrase";

pub fn expand(program: &mut Program) -> Vec<Diagnostic> {
    let mut d = Vec::new();

    let by_name: BTreeMap<String, ComponentDecl> =
        program.components.iter().map(|c| (c.name.clone(), c.clone())).collect();

    for c in &program.components {
        if is_catalogue_name(&c.name) {
            d.push(Diagnostic::error(
                c.span,
                format!("`{}` is a component this package declares, so it is capitalised — the lowercase names belong to the host's catalogue", c.name),
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

    let types: BTreeMap<String, CredentialDecl> =
        program.types.iter().map(|t| (t.name.clone(), t.clone())).collect();

    let mut screens = std::mem::take(&mut program.screens);
    for s in &mut screens {
        let mut out = Vec::new();
        for node in std::mem::take(&mut s.tree) {
            out.extend(expand_node(node, &by_name, &types, &mut d));
        }
        s.tree = out.into_iter().map(|n| open_phrases(n, &mut d)).collect();
        s.title = s.title.take().map(|t| open_phrases(t, &mut d));
    }
    program.screens = screens;
    d
}

/// `text: phrase("You have {points} points", points: total)` becomes
/// `text: "You have {points} points"` with `points: total` beside it, and the
/// slot names kept on the node.
///
/// Flattening here means one shape leaves the front end: a host is handed a key
/// and named values, and does not have to know that the source wrote them as one
/// call. The renderer stays the renderer.
fn open_phrases(node: UiNode, d: &mut Vec<Diagnostic>) -> UiNode {
    let mut args = Vec::new();
    let mut slots = node.slots;

    for a in node.args {
        let Expr::Call { callee, args: inner, span } = &a.value else {
            args.push(a);
            continue;
        };
        if callee.path().as_deref() != Some(PHRASE) {
            args.push(a);
            continue;
        }

        let mut inner = inner.iter();
        let Some(key) = inner.next() else {
            d.push(Diagnostic::error(
                *span,
                "`phrase` carries its values: `line(\"You have {points} points\", points: total)`"
                    .to_string(),
            ));
            continue;
        };
        if !matches!(key.value, Expr::Str { .. }) {
            d.push(Diagnostic::error(
                key.span,
                "a phrase's words are written here, not passed in — they are checked against what was signed".to_string(),
            ));
            continue;
        }

        args.push(Arg { name: a.name.clone(), value: key.value.clone(), spread: false, span: a.span });
        for v in inner {
            match &v.name {
                Some(name) => {
                    slots.push(name.clone());
                    args.push(Arg {
                        name: Some(name.clone()),
                        value: v.value.clone(),
                        spread: false,
                        span: v.span,
                    });
                }
                None => d.push(Diagnostic::error(
                    v.span,
                    "a phrase's values are named, because the words they go into name them"
                        .to_string(),
                )),
            }
        }
    }

    UiNode {
        args,
        slots,
        children: node.children.into_iter().map(|c| open_phrases(c, d)).collect(),
        ..node
    }
}

/// `...style` inside a component's body, where `style` is one of its parameters
/// and that parameter's type is a record this package declared.
///
/// It becomes one named argument per field of that record. A spread of anything
/// else is refused: a list cannot be spread into an argument list, and a value
/// with no declared record type would leave a reader unable to say what the call
/// passes.
fn spread_args(
    a: &Arg,
    decl: &ComponentDecl,
    types: &BTreeMap<String, CredentialDecl>,
    d: &mut Vec<Diagnostic>,
) -> Vec<Arg> {
    let Expr::Ident { name, .. } = &a.value else {
        d.push(Diagnostic::error(a.span, "`...` spreads a record named by a parameter".to_string()));
        return Vec::new();
    };
    let Some(param) = decl.params.iter().find(|p| &p.name == name) else {
        d.push(Diagnostic::error(
            a.span,
            format!("`{}` is not a parameter of `{}`", name, decl.name),
        ));
        return Vec::new();
    };
    let Some(record) = types.get(&param.ty.name) else {
        d.push(Diagnostic::error(
            a.span,
            format!(
                "`{}` is a `{}`, and only a record this package declares with `type` can be spread",
                name, param.ty.name
            ),
        ));
        return Vec::new();
    };

    record
        .fields
        .iter()
        .map(|f| Arg {
            name: Some(f.name.clone()),
            value: Expr::Member {
                obj: Box::new(a.value.clone()),
                name: f.name.clone(),
                span: a.span,
            },
            spread: false,
            span: a.span,
        })
        .collect()
}

/// One node, and everything it stands for.
///
/// A component expands to its own tree with the arguments bound, so it may
/// stand for several nodes where the call site wrote one.
fn expand_node(
    node: UiNode,
    by_name: &BTreeMap<String, ComponentDecl>,
    types: &BTreeMap<String, CredentialDecl>,
    d: &mut Vec<Diagnostic>,
) -> Vec<UiNode> {
    let Some(decl) = by_name.get(&node.kind) else {
        let UiNode { kind, args, lambda, children, slots, otherwise, span } = node;
        let children =
            children.into_iter().flat_map(|c| expand_node(c, by_name, types, d)).collect();
        let otherwise =
            otherwise.into_iter().flat_map(|c| expand_node(c, by_name, types, d)).collect();
        return vec![UiNode { kind, args, lambda, children, slots, otherwise, span }];
    };

    let bound = bind(&node, decl, d);

    // A spread in the body names one of this component's parameters, so it is
    // resolved here, where that parameter's declared type is in reach.
    let body: Vec<UiNode> = decl.tree.iter().cloned().map(|n| flatten(n, decl, types, d)).collect();

    body.into_iter()
        .map(|n| substitute(n, &bound))
        .flat_map(|n| expand_node(n, by_name, types, d))
        .collect()
}

/// Replace every `...param` in a component's body with the named arguments it
/// stands for, before the parameters themselves are substituted.
fn flatten(
    node: UiNode,
    decl: &ComponentDecl,
    types: &BTreeMap<String, CredentialDecl>,
    d: &mut Vec<Diagnostic>,
) -> UiNode {
    let mut args = Vec::new();
    for a in node.args {
        if a.spread {
            args.extend(spread_args(&a, decl, types, d));
        } else {
            args.push(a);
        }
    }
    UiNode {
        args,
        children: node.children.into_iter().map(|c| flatten(c, decl, types, d)).collect(),
        otherwise: node.otherwise.into_iter().map(|c| flatten(c, decl, types, d)).collect(),
        ..node
    }
}

/// Argument to parameter, by name.
///
/// A parameter with no argument and no `?` is a missing argument, reported here
/// rather than as a name that resolves to nothing inside the component.
fn bind(call: &UiNode, decl: &ComponentDecl, d: &mut Vec<Diagnostic>) -> BTreeMap<String, Expr> {
    let mut out = BTreeMap::new();

    for a in &call.args {
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
        args: node.args.into_iter().map(|a| Arg { value: replace(a.value, bound), ..a }).collect(),
        children: node.children.into_iter().map(|c| substitute(c, bound)).collect(),
        ..node
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
