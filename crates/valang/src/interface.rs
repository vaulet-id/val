//! What a host can do, as a document the host publishes.
//!
//! The same move as the catalogue, one layer down. A capability — issuing a
//! credential, taking a payment, moving to another screen — is an operation with
//! named props and closed vocabularies, and which ones exist is the host's
//! answer rather than a list inside this crate.
//!
//! The parser learns three shapes and no words: an effect is
//! `capability.operation(prop: value)`, a screen carries settings, and a press
//! names a target. `replace`, `sheet` and `issue` are read from here.

use std::collections::BTreeMap;

use serde_json::Value as Json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// `credential`, `navigation`, `payment` — what a call writes before the dot.
    pub capability: String,
    pub version: u32,
    pub operations: BTreeMap<String, Operation>,
    /// Props a screen may carry, where this capability gives it any. Navigation
    /// is the one that does: where a package opens, how a screen is presented,
    /// what address reaches it.
    pub screen: BTreeMap<String, String>,
    pub vocabularies: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub props: BTreeMap<String, String>,
    /// Whether it can be taken back. The compiler orders irreversible effects
    /// last in a batch, and the report says an action has one — neither of which
    /// it can do by guessing from the name.
    pub reversible: bool,
}

impl Interface {
    /// One document, holding one capability or several. A host publishes what it
    /// offers; whether that is one file per capability or one file for all of
    /// them is the host's filing, not the language's.
    pub fn parse_many(source: &str) -> Result<Vec<Interface>, String> {
        let json: Json = serde_json::from_str(source).map_err(|e| e.to_string())?;
        match json {
            Json::Array(items) => {
                items.iter().map(|i| Interface::of(i)).collect::<Result<Vec<_>, _>>()
            }
            other => Interface::of(&other).map(|i| vec![i]),
        }
    }

    pub fn parse(source: &str) -> Result<Interface, String> {
        let json: Json = serde_json::from_str(source).map_err(|e| e.to_string())?;
        Interface::of(&json)
    }

    fn of(json: &Json) -> Result<Interface, String> {
        let capability =
            json["capability"].as_str().ok_or("an interface names its capability")?.to_string();
        let version = json["version"].as_u64().ok_or("an interface carries a version")? as u32;

        let mut operations = BTreeMap::new();
        for (name, spec) in json["operations"].as_object().ok_or("`operations` is an object")? {
            operations.insert(
                name.clone(),
                Operation { props: props(&spec["props"]), reversible: spec["reversible"].as_bool().unwrap_or(true) },
            );
        }

        let mut vocabularies = BTreeMap::new();
        if let Some(map) = json["vocabularies"].as_object() {
            for (prop, words) in map {
                vocabularies.insert(
                    prop.clone(),
                    words
                        .as_array()
                        .map(|a| a.iter().filter_map(|w| w.as_str().map(str::to_string)).collect())
                        .unwrap_or_default(),
                );
            }
        }

        Ok(Interface {
            capability,
            version,
            operations,
            screen: props(&json["screen"]["props"]),
            vocabularies,
        })
    }
}

fn props(j: &Json) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(map) = j.as_object() {
        for (k, v) in map {
            if let Some(ty) = v.as_str() {
                out.insert(k.clone(), ty.to_string());
            }
        }
    }
    out
}

/// Every capability a host offers.
#[derive(Debug, Clone, Default)]
pub struct Interfaces {
    pub loaded: Vec<Interface>,
}

impl Interfaces {
    pub fn of(loaded: Vec<Interface>) -> Self {
        Interfaces { loaded }
    }

    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty()
    }

    /// `credential.issue` — the capability, then the operation.
    pub fn find(&self, call: &str) -> Option<(&Interface, &Operation)> {
        let (capability, operation) = call.split_once('.')?;
        let iface = self.loaded.iter().find(|i| i.capability == capability)?;
        iface.operations.get(operation).map(|op| (iface, op))
    }

    /// Whether a name written in `execute` is an effect at all, which is what
    /// decides whether it may appear there and nowhere else.
    pub fn is_effect(&self, call: &str) -> bool {
        self.find(call).is_some()
    }

    pub fn words(&self, prop: &str) -> Option<&[String]> {
        self.loaded.iter().find_map(|i| i.vocabularies.get(prop).map(|v| v.as_slice()))
    }

    /// The props a screen may carry, from every capability that gives it any.
    pub fn screen_props(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for i in &self.loaded {
            out.extend(i.screen.clone());
        }
        out
    }
}

/// Check a package against what the host says it can do.
///
/// Without interfaces there is nothing to check against and this says nothing,
/// the way the catalogue check does: a front end that carried the list would
/// carry the first host's.
pub fn check(
    program: &crate::ast::Program,
    interfaces: &Interfaces,
) -> Vec<crate::diag::Diagnostic> {
    use crate::ast::{Expr, Phase, Stmt};
    use crate::diag::Diagnostic;

    let mut d = Vec::new();
    if interfaces.is_empty() {
        return d;
    }

    let screen_props = interfaces.screen_props();
    let mut starts = 0;

    for screen in &program.screens {
        for a in &screen.settings {
            let Some(name) = &a.name else { continue };
            if name == "start" {
                starts += 1;
            }
            let Some(ty) = screen_props.get(name) else {
                d.push(Diagnostic::error(
                    a.span,
                    format!("a screen has no `{name}` — no capability this host offers gives it one"),
                ));
                continue;
            };
            check_word(interfaces, ty, &a.value, a.span, &mut d);
        }
    }

    // Where a package opens. One answer, and with more than one screen it has to
    // be written: a package is several files, so "the first screen declared"
    // would mean the order files were read decides what somebody sees.
    if program.screens.len() > 1 && starts == 0 {
        d.push(Diagnostic::error(
            program.screens[0].span,
            "more than one screen, and none says `start: true`".to_string(),
        ));
    }
    if starts > 1 {
        d.push(Diagnostic::error(
            program.screens[0].span,
            "two screens say `start: true`, and a package opens at one".to_string(),
        ));
    }

    // Every effect is an operation of a capability this host offers, with the
    // props that operation takes.
    // A press may name a capability's operation. `onTap: navigation.back(…)` is
    // the shorter half of the same call an `execute` block would make, and
    // without this a mistyped one reached the renderer and did nothing.
    for screen in &program.screens {
        walk_ui(&screen.tree, &mut |node| {
            for a in &node.args {
                if a.name.as_deref() != Some("onTap") {
                    continue;
                }
                let Some(target) = a.value.path() else { continue };
                if !target.contains('.') {
                    continue;
                }
                if interfaces.find(&target).is_none() {
                    d.push(Diagnostic::error(
                        a.span,
                        format!("`{target}` is not something this host offers"),
                    ));
                }
            }
        });
    }

    for action in &program.actions {
        for block in action.phases.iter().filter(|b| b.phase == Phase::Execute) {
            walk(&block.stmts, &mut |stmt| {
                let Stmt::Expr { value: Expr::Call { callee, args, span }, .. } = stmt else {
                    return;
                };
                let Some(call) = callee.path() else { return };
                if !call.contains('.') {
                    return;
                }
                let Some((iface, op)) = interfaces.find(&call) else {
                    d.push(Diagnostic::error(
                        *span,
                        format!("`{call}` is not something this host offers"),
                    ));
                    return;
                };
                for a in args {
                    let Some(name) = &a.name else { continue };
                    let Some(ty) = op.props.get(name) else {
                        d.push(Diagnostic::error(
                            a.span,
                            format!("`{call}` has no `{name}`"),
                        ));
                        continue;
                    };
                    let _ = iface;
                    check_word(interfaces, ty, &a.value, a.span, &mut d);
                }
            });
        }
    }

    d
}

/// A prop whose type names a vocabulary takes one of its words.
fn check_word(
    interfaces: &Interfaces,
    ty: &str,
    value: &crate::ast::Expr,
    span: crate::diag::Span,
    d: &mut Vec<crate::diag::Diagnostic>,
) {
    use crate::ast::Expr;
    let key = lower_first(ty.trim_end_matches('?'));
    let (Some(words), Expr::Ident { name: word, .. }) = (interfaces.words(&key), value) else {
        return;
    };
    if !words.contains(word) {
        d.push(crate::diag::Diagnostic::error(
            span,
            format!("`{word}` is not one of {key}: {}", words.join(", ")),
        ));
    }
}

fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn walk_ui(nodes: &[crate::ast::UiNode], f: &mut impl FnMut(&crate::ast::UiNode)) {
    for n in nodes {
        f(n);
        walk_ui(&n.children, f);
    }
}

fn walk(stmts: &[crate::ast::Stmt], f: &mut impl FnMut(&crate::ast::Stmt)) {
    use crate::ast::Stmt;
    for s in stmts {
        f(s);
        match s {
            Stmt::Effect { body, .. } => walk(body, f),
            Stmt::If { then, other, .. } => {
                walk(then, f);
                walk(other, f);
            }
            _ => {}
        }
    }
}
