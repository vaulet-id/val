//! What a host can draw, as a document the host publishes.
//!
//! The language does not define a catalogue. It defines this: a named, versioned
//! list of components, the props each takes, and the closed vocabularies those
//! props draw their words from. A wallet publishes one; a package names the ones
//! it needs; the compiler checks a screen against them.
//!
//! That is what lets a second host exist. A list of components compiled into the
//! front end would be a list of what the first host happened to ship, and every
//! later wallet would be implementing Vaulet rather than implementing VAL.

use std::collections::BTreeMap;

use serde_json::Value as Json;

/// The profile every conforming host implements. A package that names only this
/// runs on any of them.
pub const CORE: &str = "org.val.core";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalogue {
    /// Reverse-DNS, so two hosts cannot claim one name.
    pub name: String,
    pub version: u32,
    pub components: BTreeMap<String, ComponentSpec>,
    /// Prop name to the words it accepts. `emphasis` is one; an application
    /// cannot add a word to it, which is what makes "not in this catalogue"
    /// something the compiler can say rather than something a renderer discovers.
    pub vocabularies: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSpec {
    /// Prop name to its type, as written in the document: `Text`, `Text?`,
    /// `Action?`, `List<T>`. A `?` means the prop may be left out.
    pub props: BTreeMap<String, String>,
    pub children: bool,
}

impl Catalogue {
    /// `org.val.core/1`, as a package names it.
    pub fn id(&self) -> String {
        format!("{}/{}", self.name, self.version)
    }

    /// How a component of this catalogue is written in a screen. The core
    /// profile's components are written bare; a host's own are written under its
    /// name, so a reader can see from the line that a screen has stopped being
    /// portable.
    pub fn qualified(&self, component: &str) -> String {
        if self.name == CORE {
            component.to_string()
        } else {
            format!("{}.{}", short(&self.name), component)
        }
    }

    pub fn parse(source: &str) -> Result<Catalogue, String> {
        let json: Json = serde_json::from_str(source).map_err(|e| e.to_string())?;

        let name = json["catalogue"].as_str().ok_or("a catalogue names itself")?.to_string();
        let version =
            json["version"].as_u64().ok_or("a catalogue carries a version")? as u32;

        let mut components = BTreeMap::new();
        for (kind, spec) in json["components"].as_object().ok_or("`components` is an object")? {
            let mut props = BTreeMap::new();
            if let Some(map) = spec["props"].as_object() {
                for (prop, ty) in map {
                    props.insert(
                        prop.clone(),
                        ty.as_str().ok_or("a prop's type is a string")?.to_string(),
                    );
                }
            }
            components.insert(
                kind.clone(),
                ComponentSpec { props, children: spec["children"].as_bool().unwrap_or(false) },
            );
        }

        let mut vocabularies = BTreeMap::new();
        if let Some(map) = json["vocabularies"].as_object() {
            for (prop, words) in map {
                let list = words
                    .as_array()
                    .ok_or("a vocabulary is an array of words")?
                    .iter()
                    .filter_map(|w| w.as_str().map(str::to_string))
                    .collect();
                vocabularies.insert(prop.clone(), list);
            }
        }

        Ok(Catalogue { name, version, components, vocabularies })
    }
}

/// The last label of a reverse-DNS name, which is what a screen writes:
/// `com.alipay.wallet` is written `wallet.payButton`.
fn short(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// Every catalogue a package may draw from, keyed by what a screen writes.
#[derive(Debug, Clone, Default)]
pub struct Catalogues {
    pub loaded: Vec<Catalogue>,
}

impl Catalogues {
    pub fn of(loaded: Vec<Catalogue>) -> Self {
        Catalogues { loaded }
    }

    /// The spec for a component as a screen wrote it, and the catalogue it came
    /// from.
    pub fn find(&self, written: &str) -> Option<(&Catalogue, &ComponentSpec)> {
        for c in &self.loaded {
            for (kind, spec) in &c.components {
                if c.qualified(kind) == written {
                    return Some((c, spec));
                }
            }
        }
        None
    }

    /// The words a prop accepts, from whichever catalogue defines them. A
    /// vocabulary is shared across a host's catalogues rather than repeated,
    /// because `emphasis` meaning two things on one phone is worse than a
    /// duplicate.
    pub fn words(&self, prop: &str) -> Option<&[String]> {
        self.loaded.iter().find_map(|c| c.vocabularies.get(prop).map(|v| v.as_slice()))
    }

    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty()
    }
}

/// Check every screen against the catalogues the package named.
///
/// Without a catalogue there is nothing to check against, and a front end that
/// guessed would be a front end with a favourite host. It says so instead.
pub fn check_screens(
    program: &crate::ast::Program,
    catalogues: &Catalogues,
) -> Vec<crate::diag::Diagnostic> {
    use crate::ast::Expr;
    use crate::diag::Diagnostic;

    let mut d = Vec::new();
    if catalogues.is_empty() {
        return d;
    }

    let declared: Vec<String> = program.catalogues.clone();
    for c in &catalogues.loaded {
        if c.name != CORE && !declared.iter().any(|x| x == &c.id()) {
            // Loaded but not named: the host can draw it, and this package did
            // not ask for it, so its components are not in reach.
            continue;
        }
    }

    let usable = Catalogues::of(
        catalogues
            .loaded
            .iter()
            .filter(|c| c.name == CORE || declared.iter().any(|x| x == &c.id()))
            .cloned()
            .collect(),
    );

    for screen in &program.screens {
        walk(&screen.tree, &mut |node| {
            let Some((_, spec)) = usable.find(&node.kind) else {
                // Named by a catalogue the host has but the package did not ask
                // for, which is a different mistake and worth a different
                // sentence.
                if catalogues.find(&node.kind).is_some() {
                    d.push(Diagnostic::error(
                        node.span,
                        format!(
                            "`{}` needs a `catalogue` declaration for the catalogue it comes from",
                            node.kind
                        ),
                    ));
                } else {
                    d.push(Diagnostic::error(
                        node.span,
                        format!("`{}` is not in this catalogue", node.kind),
                    ));
                }
                return;
            };

            if !spec.children && !node.children.is_empty() {
                d.push(Diagnostic::error(
                    node.span,
                    format!("`{}` holds no children", node.kind),
                ));
            }

            for a in &node.args {
                let Some(name) = &a.name else { continue };
                // A slot filled from a sentence is checked against the sentence,
                // not against the catalogue: the words are the package's own.
                if node.slots.contains(name) {
                    continue;
                }
                if !spec.props.contains_key(name) {
                    d.push(Diagnostic::error(
                        a.span,
                        format!("`{}` has no `{name}`", node.kind),
                    ));
                    continue;
                }
                // A prop whose type names a vocabulary takes one of its words.
                let ty = spec.props[name].trim_end_matches('?');
                let key = lower_first(ty);
                if let (Some(words), Expr::Ident { name: word, .. }) = (usable.words(&key), &a.value)
                {
                    if !words.contains(word) {
                        d.push(Diagnostic::error(
                            a.span,
                            format!(
                                "`{word}` is not one of {}: {}",
                                key,
                                words.join(", ")
                            ),
                        ));
                    }
                }
            }
        });
    }

    // A field writes into a name, and that name is an input of the action the
    // screen's press calls. Checked here because it is the only place both are
    // in reach — and unchecked it fails at the one moment a person has already
    // filled the form in.
    for screen in &program.screens {
        let mut into: Vec<(String, crate::diag::Span)> = Vec::new();
        let mut actions: Vec<String> = Vec::new();
        walk(&screen.tree, &mut |node| {
            for a in &node.args {
                match a.name.as_deref() {
                    Some("into") => {
                        if let Some(name) = a.value.path() {
                            into.push((name, a.span));
                        }
                    }
                    Some("onTap") => {
                        if let Some(name) = a.value.path() {
                            actions.push(name);
                        }
                    }
                    _ => {}
                }
            }
        });

        for (name, span) in into {
            let known = actions.iter().any(|action| {
                program
                    .actions
                    .iter()
                    .find(|a| &a.name == action)
                    .is_some_and(|a| declares_input(a, &name))
            });
            if !known {
                d.push(Diagnostic::error(
                    span,
                    format!(
                        "nothing this screen can press takes an input named `{name}`"
                    ),
                ));
            }
        }
    }

    for p in &program.catalogues {
        if !catalogues.loaded.iter().any(|c| &c.id() == p) {
            d.push(Diagnostic::error(
                crate::diag::Span::default(),
                format!("this package names the catalogue `{p}`, and this host does not have it"),
            ));
        }
    }

    d
}

fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn walk(nodes: &[crate::ast::UiNode], f: &mut impl FnMut(&crate::ast::UiNode)) {
    for n in nodes {
        f(n);
        walk(&n.children, f);
    }
}

/// Whether an action's `input` block declares this name.
fn declares_input(action: &crate::ast::ActionDecl, name: &str) -> bool {
    use crate::ast::{Phase, Stmt};
    action.phases.iter().filter(|b| b.phase == Phase::Input).any(|b| {
        b.stmts.iter().any(|s| matches!(s, Stmt::Binding { name: n, .. } if n == name))
    })
}
