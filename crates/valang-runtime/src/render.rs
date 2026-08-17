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
    /// Argument names to the values they evaluated to. A slot the host will
    /// format: the application never touches a rendered number.
    pub args: BTreeMap<String, Value>,
    pub children: Vec<Component>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub name: String,
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
    let screen: &ScreenDecl = program
        .screens
        .iter()
        .find(|s| s.name == screen_name)
        .ok_or_else(|| Trap::Unsupported(format!("no screen named `{screen_name}`")))?;

    let mut ev = Eval::new(program, host.context());
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

    let tree = screen.tree.iter().map(|n| component(&mut ev, n, state)).collect::<Result<Vec<_>, _>>()?;
    Ok(Screen { name: screen.name.clone(), data: resolved, derived, tree })
}

fn component(ev: &mut Eval, n: &UiNode, state: &BTreeMap<String, Value>) -> Result<Component, Trap> {
    let mut args = BTreeMap::new();
    for (i, a) in n.args.iter().enumerate() {
        let key = a.name.clone().unwrap_or_else(|| i.to_string());
        // `onTap: ScanToEarn` names an action; evaluating it would look for a
        // value that is not one. The name is the payload.
        let value = match (&key[..], a.value.path()) {
            ("onTap", Some(action)) => Value::Str(action),
            _ => ev.expr(&a.value, state).unwrap_or(Value::Null),
        };
        args.insert(key, value);
    }
    Ok(Component {
        kind: n.kind.clone(),
        args,
        children: n.children.iter().map(|c| component(ev, c, state)).collect::<Result<Vec<_>, _>>()?,
    })
}

/// Every screen a program declares, for a host that wants to warm them.
pub fn screens(program: &Program) -> Vec<&str> {
    program.screens.iter().map(|s| s.name.as_str()).collect()
}
