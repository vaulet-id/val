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

/// The other packages a build can reach.
///
/// Supplied by whoever runs the compiler, the way the host registries are: a
/// front end that knew where packages live would know one publisher's answer,
/// and there is more than one place a package can come from.
#[derive(Debug, Default)]
pub struct Packages {
    by_id: BTreeMap<String, Program>,
}

impl Packages {
    /// Keyed by what an import writes — `org.vaulet.ui/1`, the package's own
    /// name and version.
    pub fn of(programs: Vec<Program>) -> Self {
        let by_id = programs
            .into_iter()
            .filter_map(|p| {
                let app = p.app.clone()?;
                let version = p.version.clone()?;
                Some((format!("{app}/{version}"), p))
            })
            .collect();
        Self { by_id }
    }

    pub fn find(&self, id: &str) -> Option<&Program> {
        self.by_id.get(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.by_id.keys()
    }
}

/// What an exported component may not reach for.
///
/// It is expanded into a package that is not the one that wrote it, so a name
/// resolved against the wrong package's state is not a mistake the author of
/// either package could see. A component that leaves a package is a function of
/// its arguments and nothing else.
const NOT_IN_AN_EXPORT: &[&str] = &["state", "input", "context"];

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

pub fn expand(program: &mut Program, packages: &Packages) -> Vec<Diagnostic> {
    let mut d = Vec::new();

    let mut by_name: BTreeMap<String, ComponentDecl> =
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
    // package reports the cycle rather than running out of memory — and its
    // early return comes before imports are resolved, so an import that failed
    // does not stop the screens being expanded and leave every call to it
    // reported a second time as a name the host does not have.
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

    // What this package wrote is checked here; what it imported was checked
    // when its own package was built, and is checked again on the way in
    // because a package arrives as an artifact rather than as a promise.
    for c in &program.components {
        if c.exported {
            for node in &c.tree {
                reaches_outside_its_arguments(node, &c.name, &mut d);
            }
            for p in &c.params {
                if let Some(value) = &p.default {
                    reaches_outside_its_arguments_expr(value, &c.name, p.span, &mut d);
                }
            }
        }
    }

    let (imported, unresolved) = resolve_imports(program, packages, &mut d);
    for (name, decl) in imported {
        if by_name.contains_key(&name) {
            d.push(Diagnostic::error(
                decl.span,
                format!("`{name}` is imported and this package also declares it. Which one a screen meant would depend on which pass ran first"),
            ));
            continue;
        }
        by_name.insert(name, decl);
    }

    let types: BTreeMap<String, CredentialDecl> =
        program.types.iter().map(|t| (t.name.clone(), t.clone())).collect();

    let mut screens = std::mem::take(&mut program.screens);
    for s in &mut screens {
        let mut out = Vec::new();
        for node in std::mem::take(&mut s.tree) {
            out.extend(expand_node(node, &by_name, &types, &unresolved, &mut d));
        }
        s.tree = out.into_iter().map(|n| open_phrases(n, &mut d)).collect();
        s.title = s.title.take().map(|t| open_phrases(t, &mut d));
    }
    program.screens = screens;
    d
}

/// The components this package takes from others, expanded in the package that
/// wrote them.
///
/// Expanding there rather than here is what keeps the two packages' names
/// apart: what arrives is a tree of the host's catalogue and this component's
/// own parameters, so an exporting package's private helper never has to not
/// collide with an importing package's component.
///
/// It also means there is no linking step and nothing resolved at run time. The
/// package a host admits is one program, and what an imported component draws
/// lands in that program's capability report — a person consents to one list,
/// not to one per package that happened to be involved.
fn resolve_imports(
    program: &Program,
    packages: &Packages,
    d: &mut Vec<Diagnostic>,
) -> (BTreeMap<String, ComponentDecl>, std::collections::BTreeSet<String>) {
    let mut chain = Vec::new();
    if let (Some(app), Some(version)) = (&program.app, &program.version) {
        chain.push(format!("{app}/{version}"));
    }
    take(&program.imports, packages, &mut chain, d)
}

/// What a list of imports brings in.
///
/// Recursive, because a package may export something built out of what it
/// imported: resolving only the exporting package's own components left that
/// name unexpanded, and it arrived at the host as a component nobody ships.
///
/// The chain is the packages being resolved, so that two packages importing
/// each other are told about rather than expanded until the memory runs out.
fn take(
    imports: &[crate::ast::ImportDecl],
    packages: &Packages,
    chain: &mut Vec<String>,
    d: &mut Vec<Diagnostic>,
) -> (BTreeMap<String, ComponentDecl>, std::collections::BTreeSet<String>) {
    let mut out: BTreeMap<String, ComponentDecl> = BTreeMap::new();
    let mut unresolved = std::collections::BTreeSet::new();

    for import in imports {
        if chain.contains(&import.package) {
            d.push(Diagnostic::error(
                import.span,
                format!(
                    "`{}` is imported by something it imports: {} → {}. A package is built out of the ones it names, so a circle among them has no bottom",
                    import.package,
                    chain.join(" → "),
                    import.package
                ),
            ));
            unresolved.extend(import.names.iter().cloned());
            continue;
        }

        let Some(source) = packages.find(&import.package) else {
            let known: Vec<&String> = packages.ids().collect();
            d.push(Diagnostic::error(
                import.span,
                if known.is_empty() {
                    format!("`{}` is not a package this build can reach, and no packages were supplied to it at all", import.package)
                } else {
                    format!(
                        "`{}` is not a package this build can reach. It has: {}",
                        import.package,
                        known.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")
                    )
                },
            ));
            unresolved.extend(import.names.iter().cloned());
            continue;
        };

        // What that package can build with: its own components, and whatever it
        // imported in turn.
        chain.push(import.package.clone());
        let (theirs_imported, _) = take(&source.imports, packages, chain, d);
        chain.pop();

        let mut theirs: BTreeMap<String, ComponentDecl> =
            source.components.iter().map(|c| (c.name.clone(), c.clone())).collect();
        for (name, decl) in theirs_imported {
            theirs.entry(name).or_insert(decl);
        }

        let their_types: BTreeMap<String, CredentialDecl> =
            source.types.iter().map(|t| (t.name.clone(), t.clone())).collect();

        for name in &import.names {
            if out.contains_key(name) {
                d.push(Diagnostic::error(
                    import.span,
                    format!("`{name}` is imported twice, from two packages"),
                ));
                continue;
            }
            let Some(decl) = theirs.get(name) else {
                d.push(Diagnostic::error(
                    import.span,
                    format!("`{}` declares no `{name}`", import.package),
                ));
                unresolved.insert(name.clone());
                continue;
            };
            // A component that reaches back into the package that imported it
            // expands forever. The per-package check cannot see this one: it is
            // a circle drawn through two packages, and neither half of it is a
            // cycle on its own.
            if let Some(path) = cycle_from(name, &theirs) {
                d.push(Diagnostic::error(
                    import.span,
                    format!("`{name}` uses itself, through the packages it was built from: {path}"),
                ));
                unresolved.insert(name.clone());
                continue;
            }
            if !decl.exported {
                d.push(Diagnostic::error(
                    import.span,
                    format!("`{}` declares `{name}` and does not export it. What leaves a package is the package's decision, because changing it breaks somebody who is not in the room", import.package),
                ));
                unresolved.insert(name.clone());
                continue;
            }

            let tree: Vec<UiNode> = decl
                .tree
                .iter()
                .cloned()
                .flat_map(|n| expand_node(n, &theirs, &their_types, &Default::default(), d))
                .collect();

            for node in &tree {
                reaches_outside_its_arguments(node, name, d);
            }
            for p in &decl.params {
                if let Some(value) = &p.default {
                    reaches_outside_its_arguments_expr(value, name, p.span, d);
                }
            }

            out.insert(name.clone(), ComponentDecl { tree, ..decl.clone() });
        }
    }

    (out, unresolved)
}

/// An exported component reads its own parameters and nothing else.
///
/// `state.points` inside one would resolve against whichever package it was
/// expanded into, which is a mistake neither author can see: the one who wrote
/// it was looking at their own state, and the one who imported it never read
/// the body.
fn reaches_outside_its_arguments(node: &UiNode, component: &str, d: &mut Vec<Diagnostic>) {
    for a in &node.args {
        reaches_outside_its_arguments_expr(&a.value, component, a.span, d);
    }
    for child in node.children.iter().chain(node.otherwise.iter()) {
        reaches_outside_its_arguments(child, component, d);
    }
}

/// The same rule for one expression, so that a parameter's default is held to
/// it as well: `n: int default state.points` is read where the component lands,
/// which is a package whose state it has never seen.
fn reaches_outside_its_arguments_expr(
    value: &Expr,
    component: &str,
    span: Span,
    d: &mut Vec<Diagnostic>,
) {
    let mut found = None;
    value.walk(&mut |e| {
        if found.is_some() {
            return;
        }
        if let Some(root) = root_of(e) {
            if NOT_IN_AN_EXPORT.contains(&root.as_str()) {
                found = Some(root);
            }
        }
    });
    if let Some(root) = found {
        d.push(Diagnostic::error(
            span,
            format!("`{component}` is exported and reads `{root}`, which belongs to whichever package it is expanded into. An exported component takes what it draws as an argument"),
        ));
    }
}

/// The name a path starts at. `state.member.points` is `state`.
fn root_of(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident { name, .. } => Some(name.clone()),
        Expr::Member { obj, .. } => root_of(obj),
        _ => None,
    }
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
        otherwise: node.otherwise.into_iter().map(|c| open_phrases(c, d)).collect(),
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
                optional: false,
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
    unresolved: &std::collections::BTreeSet<String>,
    d: &mut Vec<Diagnostic>,
) -> Vec<UiNode> {
    // A name an import could not supply is dropped rather than left to the pass
    // that asks what the host provides — which would answer that a component the
    // author knows they imported is not one of the host's, and send them looking
    // in the wrong place. The build has already failed by here.
    if unresolved.contains(&node.kind) {
        return Vec::new();
    }
    let Some(decl) = by_name.get(&node.kind) else {
        let UiNode { kind, args, lambda, children, slots, otherwise, span } = node;
        let children =
            children.into_iter().flat_map(|c| expand_node(c, by_name, types, unresolved, d)).collect();
        let otherwise =
            otherwise.into_iter().flat_map(|c| expand_node(c, by_name, types, unresolved, d)).collect();
        return vec![UiNode { kind, args, lambda, children, slots, otherwise, span }];
    };

    let bound = bind(&node, decl, d);

    // A spread in the body names one of this component's parameters, so it is
    // resolved here, where that parameter's declared type is in reach.
    let body: Vec<UiNode> = decl.tree.iter().cloned().map(|n| flatten(n, decl, types, d)).collect();

    body.into_iter()
        .map(|n| substitute(n, &bound))
        .flat_map(|n| expand_node(n, by_name, types, unresolved, d))
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
        if out.contains_key(&p.name) {
            continue;
        }
        // A parameter with a default is one the call site may leave out, and
        // what it then means is written once where the component is, not once
        // per call site.
        if let Some(value) = &p.default {
            out.insert(p.name.clone(), value.clone());
            continue;
        }
        if !p.ty.optional {
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
        otherwise: node.otherwise.into_iter().map(|c| substitute(c, bound)).collect(),
        ..node
    }
}

/// A parameter, wherever it appears in an expression.
///
/// Every variant is written out and there is no catch-all for the ones that
/// hold expressions: the arm that swallowed the rest is how `note exists` inside
/// a component kept the parameter's own name and evaluated to nothing — a
/// condition that was false for a value that was there. When a variant is added
/// to `Expr`, this stops compiling, which is the point.
fn replace(e: Expr, bound: &BTreeMap<String, Expr>) -> Expr {
    let go = |x: Box<Expr>| Box::new(replace(*x, bound));
    match e {
        Expr::Ident { ref name, .. } => bound.get(name).cloned().unwrap_or(e),
        Expr::Member { obj, name, optional, span } => {
            Expr::Member { obj: go(obj), name, optional, span }
        }
        Expr::Record { spread, fields, span } => Expr::Record {
            spread: spread.map(go),
            fields: fields.into_iter().map(|(k, v)| (k, replace(v, bound))).collect(),
            span,
        },
        Expr::Call { callee, args, span } => Expr::Call {
            callee,
            args: args.into_iter().map(|a| Arg { value: replace(a.value, bound), ..a }).collect(),
            span,
        },
        Expr::Unary { op, rhs, span } => Expr::Unary { op, rhs: go(rhs), span },
        Expr::Binary { op, lhs, rhs, span } => {
            Expr::Binary { op, lhs: go(lhs), rhs: go(rhs), span }
        }
        Expr::Ternary { cond, then, other, span } => {
            Expr::Ternary { cond: go(cond), then: go(then), other: go(other), span }
        }
        Expr::With { subject, policy, span } => {
            Expr::With { subject: go(subject), policy, span }
        }
        Expr::Exists { subject, span } => Expr::Exists { subject: go(subject), span },
        Expr::Elvis { subject, other, span } => {
            Expr::Elvis { subject: go(subject), other: go(other), span }
        }
        Expr::List { items, span } => {
            Expr::List { items: items.into_iter().map(|i| replace(i, bound)).collect(), span }
        }
        Expr::Switch { subject, arms, span } => Expr::Switch {
            subject: go(subject),
            arms: arms
                .into_iter()
                .map(|a| crate::ast::SwitchArm {
                    pattern: match a.pattern {
                        crate::ast::ArmPattern::Value(v) => {
                            crate::ast::ArmPattern::Value(replace(v, bound))
                        }
                        crate::ast::ArmPattern::Compare { op, rhs } => {
                            crate::ast::ArmPattern::Compare { op, rhs: replace(rhs, bound) }
                        }
                        crate::ast::ArmPattern::Default => crate::ast::ArmPattern::Default,
                    },
                    body: replace(a.body, bound),
                    span: a.span,
                })
                .collect(),
            span,
        },
        // A lambda's own parameters shadow whatever the call site bound, so they
        // are removed from the map before the body is walked — otherwise
        // `receipts.map { amount -> amount }` would take the component's
        // `amount` instead of the row's.
        Expr::Lambda { params, body, span } => {
            let mut inner = bound.clone();
            for p in &params {
                inner.remove(p);
            }
            Expr::Lambda { params, body: Box::new(replace(*body, &inner)), span }
        }
        Expr::From { value, policies, span } => {
            Expr::From { value: go(value), policies, span }
        }
        // The leaves. Nothing inside them to replace.
        Expr::Num { .. }
        | Expr::Float { .. }
        | Expr::Str { .. }
        | Expr::Bool { .. }
        | Expr::Error { .. } => e,
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
            // Both halves. A component used only in the branch that is not
            // taken is still a component this one uses, and a cycle hidden
            // there expands forever rather than being reported.
            walk(&n.otherwise, out);
        }
    }
    walk(tree, &mut out);
    out
}

/// Where a span is not worth inventing.
pub const NOWHERE: Span = Span { line: 0, col: 0, len: 0 };
