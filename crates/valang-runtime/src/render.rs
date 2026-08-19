//! Resolving a screen.
//!
//! The application declared what it needs and which components it wants; the
//! host answers both. Nothing here draws anything — the output is a description
//! for whatever toolkit the host uses, which for the first host is Flutter, and
//! `button` there means Flutter's button.

use std::collections::BTreeMap;

use valang::ast::{DataSource, Program, ScreenDecl, UiNode};

use crate::eval::{Eval, Trap};
use crate::host::Host;
use crate::value::Value;

/// One line of "what this screen sees", which is the block a reviewer reads to
/// learn what an application looks at of somebody's wallet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub name: String,
    /// `issuer`, `origin`, or `unverified` — the three grades, decided here and
    /// not by the application, which has an interest in the answer.
    pub grade: &'static str,
    pub of: String,
    pub policy: Option<String>,
    pub rows: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub kind: String,
    /// Argument names to the values they evaluated to. A slot the renderer will
    /// format — the application never touches a rendered number, and neither
    /// does anything upstream of the toolkit that knows what a date looks like
    /// in a locale.
    pub args: BTreeMap<String, Value>,
    pub children: Vec<Component>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub name: String,
    /// Where the package opens. A setting rather than a keyword — see the
    /// navigation interface — and one screen carries it.
    pub start: bool,
    /// What the person reads at the top, resolved like any other sentence. The
    /// identifier is what the code calls this screen; it is not what anybody
    /// reads, and it could not be Thai if it were.
    pub title: Option<Component>,
    pub data: Vec<Resolved>,
    pub derived: BTreeMap<String, Value>,
    pub tree: Vec<Component>,
}

pub fn render(
    program: &Program,
    screen_name: &str,
    state: &BTreeMap<String, Value>,
    host: &dyn Host,
) -> Result<Screen, Trap> {
    render_with(program, screen_name, state, &BTreeMap::new(), host)
}

/// Resolve a screen with what a press handed it.
///
/// A screen takes parameters the way a component does, so a detail screen can be
/// written once and opened with any row. The values are bound before anything is
/// evaluated, which is why a parameterised screen cannot be resolved ahead of
/// time and is resolved when it is opened.
pub fn render_with(
    program: &Program,
    screen_name: &str,
    state: &BTreeMap<String, Value>,
    args: &BTreeMap<String, Value>,
    host: &dyn Host,
) -> Result<Screen, Trap> {
    let screen: &ScreenDecl = program
        .screens
        .iter()
        .find(|s| s.name == screen_name)
        .ok_or_else(|| Trap::Unsupported(format!("no screen named `{screen_name}`")))?;

    let mut ev = Eval::new(program, host.context());
    for (name, value) in args {
        ev.bind(name, value.clone());
    }
    let mut resolved = Vec::new();

    for d in &screen.data {
        let (value, line) = match &d.source {
            DataSource::Credentials { ty, policy, limit } => {
                let rows = host.credentials_of(ty, policy.as_deref(), *limit);
                let items: Vec<Value> = rows
                    .into_iter()
                    .map(|claims| Value::Credential {
                        ty: ty.clone(),
                        claims,
                        // The host checked the policy before handing these over,
                        // so they arrive verified. A screen cannot verify: it
                        // has no `verify` phase, and adding one would be adding
                        // a second place trust is decided.
                        verified: policy.clone(),
                    })
                    .collect();
                let n = items.len();
                (
                    Value::List(items),
                    Resolved {
                        name: d.name.clone(),
                        grade: if policy.is_some() { "issuer" } else { "unverified" },
                        of: ty.clone(),
                        policy: policy.clone(),
                        rows: n,
                    },
                )
            }
            DataSource::Query { audience } => {
                let rows = host.query(audience, audience);
                let n = rows.len();
                (
                    Value::List(rows),
                    Resolved { name: d.name.clone(), grade: "origin", of: audience.clone(), policy: None, rows: n },
                )
            }
            DataSource::Unknown => (Value::Null, Resolved { name: d.name.clone(), grade: "unverified", of: String::new(), policy: None, rows: 0 }),
        };
        ev.bind(&d.name, value);
        resolved.push(line);
    }

    // A screen derives and does not act: the same rules as an action's
    // `compute`, and the reason a total is not kept in `state`.
    let mut derived = BTreeMap::new();
    for stmt in &screen.compute {
        let mut ignored = state.clone();
        ev.stmt(stmt, valang::ast::Phase::Compute, &mut ignored)?;
        if let valang::ast::Stmt::Let { name, .. } = stmt {
            if let Ok(v) = ev.expr(&valang::ast::Expr::Ident { name: name.clone(), span: Default::default() }, state) {
                derived.insert(name.clone(), v);
            }
        }
    }

    let tree = components(&mut ev, &screen.tree, state)?;
    let title = screen.title.as_ref().map(|t| component(&mut ev, t, state)).transpose()?;
    let start = screen.is_main();
    Ok(Screen { name: screen.name.clone(), start, title, data: resolved, derived, tree })
}

/// Whether a path starts at something the runtime binds. Everything else that
/// resolves to nothing is a word from the host's vocabulary.
fn is_bound_root(path: &str) -> bool {
    matches!(path.split('.').next(), Some("state" | "context" | "next"))
}

/// A run of sibling nodes.
///
/// One node in, one node out is not enough here: an `if` stands for the branch
/// it chose, which may be several nodes or none. Choosing here rather than in
/// the host is what keeps the condition out of what a host receives — the tree
/// that leaves this function has already made up its mind.
fn components(
    ev: &mut Eval,
    nodes: &[UiNode],
    state: &BTreeMap<String, Value>,
) -> Result<Vec<Component>, Trap> {
    let mut out = Vec::new();
    for n in nodes {
        // `for (r in rows) { … }` — the body once per item, spliced where the
        // loop was written. Resolved here for the same reason `if` is: what a
        // host receives is a tree, and a host that had to implement a loop
        // would be a host implementing the language.
        if n.kind == "for" {
            let items = match n.args.first() {
                Some(a) => match ev.expr(&a.value, state)? {
                    Value::List(items) => items,
                    Value::Null => Vec::new(),
                    one => vec![one],
                },
                None => Vec::new(),
            };
            for item in items {
                if let Some(bind) = &n.lambda {
                    ev.bind(bind, item);
                }
                out.extend(components(ev, &n.children, state)?);
            }
            continue;
        }

        if n.kind == "if" {
            let taken = match n.args.first() {
                Some(a) => matches!(ev.expr(&a.value, state)?, Value::Bool(true)),
                None => false,
            };
            let branch = if taken { &n.children } else { &n.otherwise };
            out.extend(components(ev, branch, state)?);
            continue;
        }
        out.push(component(ev, n, state)?);
    }
    Ok(out)
}

fn component(ev: &mut Eval, n: &UiNode, state: &BTreeMap<String, Value>) -> Result<Component, Trap> {
    let mut args = BTreeMap::new();
    for (i, a) in n.args.iter().enumerate() {
        let key = a.name.clone().unwrap_or_else(|| i.to_string());
        // Some arguments are names rather than expressions. `onTap: ScanToEarn`
        // names an action; `emphasis: primary` names a word in the catalogue's
        // own vocabulary, which the language does not define and cannot
        // evaluate. A bare identifier that resolves to nothing is one of those,
        // and handing back null instead would silently demote every primary
        // button on every screen.
        let evaluated = ev.expr(&a.value, state).unwrap_or(Value::Null);
        let value = match (&key[..], a.value.path(), &evaluated) {
            ("onTap", Some(name), _) => Value::Str(name),
            // A name that resolves to nothing is a word rather than a value:
            // `emphasis: primary` and `color: foreground.primary` are both the
            // catalogue's own vocabulary, which the language does not define and
            // cannot evaluate. Dotted ones count — a token is one word with a
            // dot in it — except where the root is something the runtime binds,
            // because `state.missing` resolving to a word would hide a mistake.
            (_, Some(name), Value::Null) if !is_bound_root(&name) => Value::Str(name),
            _ => evaluated,
        };
        args.insert(key.clone(), value);

        // `onTap: Detail(receipt: r)` — the target's own arguments, evaluated
        // here where `r` is bound, so what a screen is opened with is a value
        // rather than an expression somebody else has to evaluate later.
        if key == "onTap" {
            if let valang::ast::Expr::Call { args: given, .. } = &a.value {
                let mut with = BTreeMap::new();
                for g in given {
                    if let Some(name) = &g.name {
                        with.insert(name.clone(), ev.expr(&g.value, state).unwrap_or(Value::Null));
                    }
                }
                if !with.is_empty() {
                    args.insert("onTapWith".to_string(), Value::Map(with));
                }
            }
        }
    }

    // `list(receipts) { r -> row(…) }` is expanded here, with `r` bound, so what
    // comes out is the rows themselves rather than a template and a promise.
    // Doing it in the renderer instead would put `limit`, `order by` and
    // `verified with` in whatever language the renderer happens to be written
    // in — and then in the next one too.
    if n.kind == "list" {
        // `of`, the name the registry gives a list's positional argument. The
        // front end names it before anything downstream sees it, so reading the
        // index here would be reading a shape that only exists when the
        // compiler was run without a registry — which is not a program a host
        // would ever admit.
        let items = match args.get("of") {
            Some(Value::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        let mut rows = Vec::new();
        for item in items {
            if let Some(bind) = &n.lambda {
                ev.bind(bind, item.clone());
            }
            rows.extend(components(ev, &n.children, state)?);
        }
        return Ok(Component { kind: n.kind.clone(), args, children: rows });
    }

    Ok(Component {
        kind: n.kind.clone(),
        args,
        children: components(ev, &n.children, state)?,
    })
}

/// Every screen a program declares, for a host that wants to warm them.
pub fn screens(program: &Program) -> Vec<&str> {
    program.screens.iter().map(|s| s.name.as_str()).collect()
}
