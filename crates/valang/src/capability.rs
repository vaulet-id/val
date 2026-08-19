//! What a host provides to a Micro App, as a document the host publishes.
//!
//! One registry. Drawing a grid and issuing a credential are the same kind of
//! thing — something this host offers — and they differ only in where they are
//! written and what they cost the person:
//!
//! - `draws` says a capability appears in a screen's tree. Without it, it is
//!   called in `execute`.
//! - `consent` says it appears on the sheet the person approves. That is a
//!   policy attribute of one capability, not a category the language has.
//!
//! The language defines none of them. A wallet publishes what it can do; a
//! package names the registries it needs; the compiler checks against those. A
//! list inside this crate would be the first host's list, and every later wallet
//! would be implementing Vaulet rather than implementing VAL.

use std::collections::BTreeMap;

use serde_json::Value as Json;

/// The registry every conforming host provides. A package naming only this runs
/// on any of them.
pub const CORE: &str = "org.val.core";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    /// Reverse-DNS, so two hosts cannot claim one name.
    pub name: String,
    pub version: u32,
    pub capabilities: BTreeMap<String, Capability>,
    /// Props a screen may carry — where a package opens, how it is presented,
    /// what address reaches it.
    pub screen: BTreeMap<String, String>,
    /// Prop name to the words it accepts. An application cannot add a word,
    /// which is what makes "not one of these" something the compiler can say.
    pub vocabularies: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    /// Written in a screen's tree rather than called in `execute`.
    pub draws: bool,
    /// Holds children, for the ones that are drawn.
    pub children: bool,
    /// On the sheet the person approves, and so declared in `capabilities { }`.
    pub consent: bool,
    /// Whether it can be taken back. The compiler orders irreversible effects
    /// last in a batch and the report says an action has one — neither of which
    /// it can do by guessing from a name.
    pub reversible: bool,
    /// Prop name to its type, as written in the document: `Text`, `Text?`,
    /// `Action?`, `List<T>`. A `?` means the prop may be left out.
    pub props: BTreeMap<String, String>,
}

impl Host {
    pub fn id(&self) -> String {
        format!("{}/{}", self.name, self.version)
    }

    /// How a capability of this host is written. The core registry's are
    /// written bare; a host's own are written under its name, so a reader sees
    /// from the line that a screen has stopped being portable.
    pub fn qualified(&self, capability: &str) -> String {
        if self.name == CORE || capability.contains('.') {
            capability.to_string()
        } else {
            format!("{}.{}", short(&self.name), capability)
        }
    }

    pub fn parse(source: &str) -> Result<Host, String> {
        let json: Json = serde_json::from_str(source).map_err(|e| e.to_string())?;
        let name = json["host"].as_str().ok_or("a host names itself")?.to_string();
        let version = json["version"].as_u64().ok_or("a host carries a version")? as u32;

        let mut capabilities = BTreeMap::new();
        for (kind, spec) in json["capabilities"].as_object().ok_or("`capabilities` is an object")? {
            capabilities.insert(
                kind.clone(),
                Capability {
                    draws: spec["draws"].as_bool().unwrap_or(false),
                    children: spec["children"].as_bool().unwrap_or(false),
                    consent: spec["consent"].as_bool().unwrap_or(false),
                    reversible: spec["reversible"].as_bool().unwrap_or(true),
                    props: props(&spec["props"]),
                },
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

        Ok(Host { name, version, capabilities, screen: props(&json["screen"]["props"]), vocabularies })
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

/// The last label of a reverse-DNS name, which is what a screen writes:
/// `id.vaulet.wallet` is written `wallet.avatar`.
fn short(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// Every registry a package may draw on.
#[derive(Debug, Clone, Default)]
pub struct Hosts {
    pub loaded: Vec<Host>,
}

impl Hosts {
    pub fn of(loaded: Vec<Host>) -> Self {
        Hosts { loaded }
    }

    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty()
    }

    /// The capability as a package wrote it, and the host it came from.
    pub fn find(&self, written: &str) -> Option<(&Host, &Capability)> {
        for h in &self.loaded {
            for (kind, cap) in &h.capabilities {
                if h.qualified(kind) == written {
                    return Some((h, cap));
                }
            }
        }
        None
    }

    /// The words a prop accepts, from whichever registry defines them. Shared
    /// across a host's registries rather than repeated, because `emphasis`
    /// meaning two things on one phone is worse than a duplicate.
    pub fn words(&self, prop: &str) -> Option<&[String]> {
        self.loaded.iter().find_map(|h| h.vocabularies.get(prop).map(|v| v.as_slice()))
    }

    pub fn screen_props(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for h in &self.loaded {
            out.extend(h.screen.clone());
        }
        out
    }
}

/// Check a package against what this host provides.
///
/// Without a registry there is nothing to check against, and a front end that
/// guessed would be a front end with a favourite host. It says nothing instead.
pub fn check(program: &crate::ast::Program, hosts: &Hosts) -> Vec<crate::diag::Diagnostic> {
    use crate::ast::{Expr, Phase, Stmt};
    use crate::diag::Diagnostic;

    let mut d = Vec::new();
    if hosts.is_empty() {
        return d;
    }

    let declared: Vec<String> = program.hosts.clone();
    let usable = Hosts::of(
        hosts
            .loaded
            .iter()
            .filter(|h| h.name == CORE || declared.iter().any(|x| x == &h.id()))
            .cloned()
            .collect(),
    );

    // Drawn capabilities, in the tree.
    for screen in &program.screens {
        walk_ui(&screen.tree, &mut |node| {
            let Some((_, cap)) = usable.find(&node.kind) else {
                let known = hosts.find(&node.kind).is_some();
                d.push(Diagnostic::error(
                    node.span,
                    if known {
                        format!("`{}` needs a `host` declaration for the registry it comes from", node.kind)
                    } else {
                        format!("`{}` is not something this host provides", node.kind)
                    },
                ));
                return;
            };

            if !cap.draws {
                d.push(Diagnostic::error(
                    node.span,
                    format!("`{}` is not drawn — it is called in `execute`", node.kind),
                ));
                return;
            }
            if !cap.children && !node.children.is_empty() {
                d.push(Diagnostic::error(node.span, format!("`{}` holds no children", node.kind)));
            }

            for a in &node.args {
                let Some(name) = &a.name else { continue };
                // A value filled into a phrase is checked against the words,
                // not against the registry: they are the package's own.
                if node.slots.contains(name) {
                    continue;
                }
                if !cap.props.contains_key(name) {
                    d.push(Diagnostic::error(a.span, format!("`{}` has no `{name}`", node.kind)));
                    continue;
                }
                check_word(&usable, &cap.props[name], &a.value, a.span, &mut d);
            }
        });
    }

    // A press may name a capability too — `onTap: navigation.back(…)`.
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
                if usable.find(&target).is_none() {
                    d.push(Diagnostic::error(
                        a.span,
                        format!("`{target}` is not something this host provides"),
                    ));
                }
            }
        });
    }

    // Called capabilities, in `execute`.
    for action in &program.actions {
        for block in action.phases.iter().filter(|b| b.phase == Phase::Execute) {
            walk_stmts(&block.stmts, &mut |stmt| {
                let Stmt::Expr { value: Expr::Call { callee, args, span }, .. } = stmt else {
                    return;
                };
                let Some(call) = callee.path() else { return };
                if !call.contains('.') {
                    return;
                }
                let Some((_, cap)) = usable.find(&call) else {
                    d.push(Diagnostic::error(
                        *span,
                        format!("`{call}` is not something this host provides"),
                    ));
                    return;
                };
                if cap.draws {
                    d.push(Diagnostic::error(
                        *span,
                        format!("`{call}` is drawn on a screen, not called here"),
                    ));
                }
                for a in args {
                    let Some(name) = &a.name else { continue };
                    let Some(ty) = cap.props.get(name) else {
                        d.push(Diagnostic::error(a.span, format!("`{call}` has no `{name}`")));
                        continue;
                    };
                    check_word(&usable, ty, &a.value, a.span, &mut d);
                }
            });
        }
    }

    // Screen settings.
    let screen_props = usable.screen_props();
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
                    format!("a screen has no `{name}` — nothing this host provides gives it one"),
                ));
                continue;
            };
            check_word(&usable, ty, &a.value, a.span, &mut d);
        }
    }

    // Where a package opens. A package is several files, so "the first screen
    // declared" would mean the order files were read decides what somebody sees.
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

    for id in &program.hosts {
        if !hosts.loaded.iter().any(|h| &h.id() == id) {
            d.push(Diagnostic::error(
                crate::diag::Span::default(),
                format!("this package names the host `{id}`, and this one is not it"),
            ));
        }
    }

    d
}

/// A prop whose type names a vocabulary takes one of its words.
fn check_word(
    hosts: &Hosts,
    ty: &str,
    value: &crate::ast::Expr,
    span: crate::diag::Span,
    d: &mut Vec<crate::diag::Diagnostic>,
) {
    use crate::ast::Expr;
    let key = lower_first(ty.trim_end_matches('?'));
    let (Some(words), Expr::Ident { name: word, .. }) = (hosts.words(&key), value) else {
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

fn walk_stmts(stmts: &[crate::ast::Stmt], f: &mut impl FnMut(&crate::ast::Stmt)) {
    use crate::ast::Stmt;
    for s in stmts {
        f(s);
        match s {
            Stmt::Effect { body, .. } => walk_stmts(body, f),
            Stmt::If { then, other, .. } => {
                walk_stmts(then, f);
                walk_stmts(other, f);
            }
            _ => {}
        }
    }
}
