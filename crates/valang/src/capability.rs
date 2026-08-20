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
    /// Layout, accessibility and style, on everything this host draws. Held
    /// once rather than repeated on every capability, and a renderer reads one
    /// set.
    pub common: BTreeMap<String, String>,
    /// Prop name to the words it accepts.
    pub vocabularies: BTreeMap<String, Vocabulary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vocabulary {
    pub words: Vec<String>,
    /// Whether a value of one's own is allowed beside the words.
    ///
    /// Closed where the host has to know what the word means — a transition, a
    /// field's kind, an icon it draws. Open where the words are this design
    /// system's and the application is somebody's own product: a token is what
    /// makes it look like it belongs on this phone, and a Micro App that wants
    /// its own colour is a customer, not an attack.
    pub open: bool,
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
    /// Which prop a positional first argument fills, where there is an obvious
    /// one: `card("Profile")` is `card(text: "Profile")`. A call site is read
    /// far more often than written, and the first argument of a card is not in
    /// doubt.
    pub primary: Option<String>,
    /// The capability drawing this one needs, where drawing it does something
    /// privileged: `video` needs `media.video`.
    ///
    /// Drawing is not permission. A person consents to a list of capabilities,
    /// and a component that quietly carried one would be a way to have the
    /// list say less than the application does.
    pub requires: Option<String>,
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
                    primary: spec["primary"].as_str().map(str::to_string),
                    requires: spec["requires"].as_str().map(str::to_string),
                },
            );
        }

        let mut vocabularies = BTreeMap::new();
        if let Some(map) = json["vocabularies"].as_object() {
            for (prop, spec) in map {
                // A bare array is closed; `{ words, open }` says which.
                let (list, open) = match spec {
                    Json::Array(a) => (a.clone(), false),
                    other => (
                        other["words"].as_array().cloned().unwrap_or_default(),
                        other["open"].as_bool().unwrap_or(false),
                    ),
                };
                vocabularies.insert(
                    prop.clone(),
                    Vocabulary {
                        words: list.iter().filter_map(|w| w.as_str().map(str::to_string)).collect(),
                        open,
                    },
                );
            }
        }

        let mut common = BTreeMap::new();
        for group in ["layout", "accessibility", "style"] {
            common.extend(props(&json["common"][group]));
        }

        Ok(Host {
            name,
            version,
            capabilities,
            screen: props(&json["screen"]["props"]),
            common,
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
    /// Every word every vocabulary holds, across every registry loaded.
    ///
    /// A screen is full of them — `primary`, `money`, `sheet` — and they are
    /// written as bare names because that is what they read like. Somewhere has
    /// to know the whole set, or a name that is neither a word nor anything the
    /// program declared cannot be told apart from a typo.
    pub fn words(&self) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        for h in &self.loaded {
            for v in h.vocabularies.values() {
                out.extend(v.words.iter().cloned());
            }
            out.extend(h.capabilities.keys().cloned());
        }
        out
    }

    /// The vocabulary a prop's declared type names — `ColorToken?` is
    /// `colorToken`. A prop that has one holds a word, and which words it may
    /// hold is checked where the registry is read.
    pub fn vocabulary_for_type(&self, ty: &str) -> Option<&Vocabulary> {
        self.vocabulary(&lower_first(ty.trim_end_matches('?')))
    }

    pub fn vocabulary(&self, prop: &str) -> Option<&Vocabulary> {
        self.loaded.iter().find_map(|h| h.vocabularies.get(prop))
    }

    /// Layout, accessibility and style, from every registry that defines any.
    pub fn common(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for h in &self.loaded {
            out.extend(h.common.clone());
        }
        out
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
pub fn check(program: &mut crate::ast::Program, hosts: &Hosts) -> Vec<crate::diag::Diagnostic> {
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

    // `card("Profile")` is `card(text: "Profile")`. Named here, where the
    // registry says which prop it is, so nothing downstream — the renderer
    // included — has to know a positional argument was ever written.
    let mut screens = std::mem::take(&mut program.screens);
    for screen in &mut screens {
        name_primary(&mut screen.tree, &usable);
    }
    program.screens = screens;

    let common = usable.common();
    let declared_capabilities: Vec<String> =
        program.capabilities.iter().map(|c| c.name.clone()).collect();
    let mut uses: Vec<String> = Vec::new();

    // Drawn capabilities, in the tree — and in the components, which a package
    // of nothing but components would otherwise never have checked: a screen is
    // what makes a component's body reachable, and a UI kit has no screens. A
    // component a screen does draw is reported at the same span either way, and
    // the duplicate collapses.
    let trees: Vec<&[crate::ast::UiNode]> = program
        .screens
        .iter()
        .map(|s| s.tree.as_slice())
        .chain(program.components.iter().map(|c| c.tree.as_slice()))
        .collect();

    for tree in &trees {
        walk_ui(tree, &mut |node| {
            // The language's own, not a host's: one chooses between two trees
            // and the other repeats one, and neither draws anything itself.
            if node.kind == "if" || node.kind == "for" {
                return;
            }
            // A component this package declares. Screens have had theirs
            // expanded by now, so this is a component's body naming another
            // one — checked where that one is declared, not once per use.
            if program.components.iter().any(|c| c.name == node.kind) {
                return;
            }
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

            // Drawing something privileged needs the capability declared, and
            // the person consented to that list rather than to a component.
            if let Some(needs) = &cap.requires {
                uses.push(needs.clone());
                if !declared_capabilities.iter().any(|c| c == needs) {
                    d.push(Diagnostic::error(
                        node.span,
                        format!(
                            "`{}` needs `{needs}`, and drawing it is not the same as being allowed to",
                            node.kind
                        ),
                    ));
                }
            }

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

            for (i, a) in node.args.iter().enumerate() {
                let Some(name) = &a.name else {
                    // A positional first argument fills the capability's
                    // primary prop, where it declares one.
                    if i == 0 && cap.primary.is_some() {
                        continue;
                    }
                    if node.kind == "list" || node.kind == "grid" {
                        continue;
                    }
                    d.push(Diagnostic::error(
                        a.span,
                        format!("`{}` takes named arguments", node.kind),
                    ));
                    continue;
                };
                // A value filled into a phrase is checked against the words,
                // not against the registry: they are the package's own.
                if node.slots.contains(name) {
                    continue;
                }
                let ty = cap.props.get(name).or_else(|| common.get(name));
                let Some(ty) = ty else {
                    d.push(Diagnostic::error(a.span, format!("`{}` has no `{name}`", node.kind)));
                    continue;
                };
                check_word(&usable, ty, &a.value, a.span, &mut d);
            }
        });
    }

    // A press may name a capability too — `onTap: navigation.back(…)`.
    for screen in &program.screens {
        walk_ui(&screen.tree, &mut |node| {
            for a in &node.args {
                let Some(prop) = a.name.as_deref() else { continue };
                if !is_handler(&usable, prop) {
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
    let starts = program.screens.iter().filter(|s| s.is_main()).count();
    for screen in &program.screens {
        for a in &screen.settings {
            let Some(name) = &a.name else { continue };
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
    // One screen and no mark used to be allowed, which left a package whose
    // only screen nothing opens at: every screen said `start: false` and a host
    // had nothing to draw first.
    if !program.screens.is_empty() && starts == 0 {
        d.push(Diagnostic::error(
            program.screens[0].span,
            "no screen is marked `@main`, and a package opens at one. Which one is not the order the files were read, and one screen today is two tomorrow".to_string(),
        ));
    }
    if starts > 1 {
        d.push(Diagnostic::error(
            program.screens[0].span,
            "two screens are marked `@main`, and a package opens at one".to_string(),
        ));
    }

    program.uses = uses;

    // Which props hold an action, from the registry rather than from a name
    // written into the compiler.
    let mut handlers: Vec<String> = Vec::new();
    for host in &usable.loaded {
        for cap in host.capabilities.values() {
            for (prop, ty) in &cap.props {
                if ty.trim_end_matches('?') == "Action" && !handlers.contains(prop) {
                    handlers.push(prop.clone());
                }
            }
        }
    }
    handlers.sort();
    program.handlers = handlers;

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

/// A prop whose type names a vocabulary takes one of its words — and, where the
/// vocabulary is open, a value of the application's own.
///
/// A word is checked because a misspelt one would otherwise be a value nobody
/// notices; a number or a string is not, because that is the application saying
/// what it wants rather than naming something this host knows.
fn check_word(
    hosts: &Hosts,
    ty: &str,
    value: &crate::ast::Expr,
    span: crate::diag::Span,
    d: &mut Vec<crate::diag::Diagnostic>,
) {
    use crate::ast::Expr;
    let key = lower_first(ty.trim_end_matches('?'));
    let Some(vocab) = hosts.vocabulary(&key) else { return };

    // `foreground.primary` is one word with a dot in it, not a field access.
    let named = match value {
        Expr::Ident { name, .. } => Some(name.clone()),
        Expr::Member { .. } => value.path(),
        _ => None,
    };
    let Some(word) = named else { return };

    if vocab.words.contains(&word) {
        return;
    }
    if vocab.open {
        // An open vocabulary still catches a misspelt token: a dotted name that
        // is not one of them is not something an application meant to invent.
        if word.contains('.') {
            d.push(crate::diag::Diagnostic::error(
                span,
                format!("`{word}` is not a {key} this host has: {}", vocab.words.join(", ")),
            ));
        }
        return;
    }
    d.push(crate::diag::Diagnostic::error(
        span,
        format!("`{word}` is not one of {key}: {}", vocab.words.join(", ")),
    ));
}

/// Whether a prop holds an action, according to the registries in reach.
fn is_handler(hosts: &Hosts, prop: &str) -> bool {
    hosts.loaded.iter().any(|h| {
        h.capabilities.values().any(|c| {
            c.props.get(prop).is_some_and(|ty| ty.trim_end_matches('?') == "Action")
        })
    })
}

fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn walk_ui(nodes: &[crate::ast::UiNode], f: &mut impl FnMut(&crate::ast::UiNode)) {
    crate::ast::UiNode::walk_all(nodes, f);
}

fn walk_stmts(stmts: &[crate::ast::Stmt], f: &mut impl FnMut(&crate::ast::Stmt)) {
    for s in stmts {
        s.walk(f);
    }
}

/// Give the positional first argument the name the registry says it has.
fn name_primary(nodes: &mut [crate::ast::UiNode], hosts: &Hosts) {
    crate::ast::UiNode::walk_all_mut(nodes, &mut |n| {
        let Some((_, cap)) = hosts.find(&n.kind) else { return };
        let Some(primary) = cap.primary.clone() else { return };
        if let Some(first) = n.args.first_mut() {
            if first.name.is_none() {
                first.name = Some(primary);
            }
        }
    });
}
