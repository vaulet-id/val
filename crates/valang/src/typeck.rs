//! Type checking, narrowing and provenance.
//!
//! The rule that does the work is small: **`.claims` is readable on a
//! `Verified<P>` and on nothing else.** Everything a credential says is behind
//! it, so an application cannot reach an issuer's words without having said,
//! in `verify`, which policy it checked them against — and the type it gets
//! back names that policy, so a stricter and a laxer check cannot be confused.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diag::{Diagnostic, Span};
use crate::types::{Provenance, Ty, Typed};

/// What a component is handed, checked where both halves are still present.
///
/// A component is expanded into the screen that drew it before anything else
/// looks at the tree, so by the time types are checked the call site is gone
/// and the parameter it filled is gone with it. This runs over the program as
/// it was written, and reports only the call — everything else about it is
/// checked after expansion, once.
/// How long a range may be. The runtime holds the same number and traps on it;
/// this is the same limit said early, where it can name the line.
const RANGE_LIMIT: i64 = 10_000;

pub fn check_component_calls(p: &Program) -> Vec<Diagnostic> {
    let mut cx = Cx::new(p);
    for s in &p.screens {
        cx.push();
        for f in &s.params {
            let t = cx.resolve(&f.ty);
            cx.bind(&f.name, Typed::plain(t));
        }
        for d in &s.data {
            cx.bind(&d.name, Typed::unknown());
        }
        for st in &s.compute {
            if let Stmt::Let { name, .. } = st {
                cx.bind(name, Typed::unknown());
            }
        }
        cx.calls(&s.tree);
        cx.pop();
    }
    for c in &p.components {
        cx.push();
        for f in &c.params {
            let t = cx.resolve(&f.ty);
            cx.bind(&f.name, Typed::plain(t));
        }
        cx.calls(&c.tree);
        cx.pop();
    }
    cx.diagnostics
}

pub fn check_types(p: &Program) -> Vec<Diagnostic> {
    check_types_against(p, &Default::default())
}

/// Type checking, with the host's words as well.
///
/// A screen is full of names nothing declared — `primary`, `money`,
/// `foreground.primary` — and they are words rather than mistakes. Without the
/// registry there is no way to tell them from a misspelt binding, so a typo in
/// a tree was drawn as itself and nobody was told.
pub fn check_types_against(p: &Program, hosts: &crate::capability::Hosts) -> Vec<Diagnostic> {
    let mut cx = Cx::new(p);
    cx.words = hosts.words();
    cx.hosts = hosts.clone();
    for f in &p.functions {
        cx.function(f);
    }
    for a in &p.actions {
        cx.action(a);
    }
    for s in &p.screens {
        cx.screen(s);
    }
    // A component's body, with its parameters bound to what they were declared
    // as. Nothing else looks inside one before it is expanded, and a component
    // in a package with no screens is expanded by nobody.
    for c in &p.components {
        cx.component(c);
    }
    cx.refinements();
    cx.diagnostics
}

// ------------------------------------------------------------- completion

/// One thing that may be written after a `.`.
///
/// **Answered by the typechecker, because it is a question about types.** An
/// editor that worked it out for itself would be a second implementation of
/// what a name means, disagreeing with this one wherever it had not been
/// taught — and the first thing it would get wrong is that `.claims` is
/// reachable on a `Verified<P>` and on nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    /// The type, as somebody would write it.
    pub ty: String,
    /// What kind of thing it is: `claim`, `field`, `member`, `face`.
    pub what: &'static str,
}

/// What may be written at `line`:`col`, where the text just before it is a path
/// and a dot.
///
/// Empty when the cursor is not after a dot, when the path names nothing, or
/// when what it names has no members — an editor offering its whole vocabulary
/// there is the case this exists to end.
pub fn members_at(p: &Program, hosts: &crate::capability::Hosts, src: &str, line: u32, col: u32) -> Vec<Member> {
    let Some(path) = path_before(src, line, col) else { return Vec::new() };
    if path.is_empty() {
        return Vec::new();
    }

    let mut cx = Cx::new(p);
    cx.words = hosts.words();
    cx.hosts = hosts.clone();
    cx.at = Some((line, col));
    // The declaration the cursor is in, from the parser's own record of what it
    // opened. A binding from another action is not in scope here, and without
    // this it would be: declarations are checked in the order this function
    // walks them, not the order they were written.
    cx.within = p
        .scopes
        .iter()
        .filter(|s| {
            matches!(s.kind, ScopeKind::Function | ScopeKind::Action | ScopeKind::Screen | ScopeKind::Component)
                && (line, col) >= (s.from.line, s.from.col)
                && (line, col) <= (s.to.line, s.to.col)
        })
        .next_back()
        .map(|s| (s.from, s.to));

    for f in &p.functions {
        cx.function(f);
    }
    for a in &p.actions {
        cx.action(a);
    }
    for s in &p.screens {
        cx.screen(s);
    }
    for c in &p.components {
        cx.component(c);
    }

    if let Some(members) = cx.root_members(&path) {
        return members;
    }
    let scope = cx.seen.take().map(|(_, s)| s).unwrap_or_default();
    let Some(ty) = cx.walk_path(&scope, &path) else { return Vec::new() };
    cx.members_of(&ty)
}

/// The `a.b.c.` immediately before the cursor, without the part being typed.
///
/// Read off the line rather than from the token stream: the text at a cursor is
/// mid-word by definition, and a lexer's answer for `state.` is a path followed
/// by a dot that belongs to nothing.
fn path_before(src: &str, line: u32, col: u32) -> Option<Vec<String>> {
    let text: Vec<char> = src.lines().nth(line.checked_sub(1)? as usize)?.chars().collect();
    let mut i = (col as usize).saturating_sub(1).min(text.len());
    // The word being typed, which is a filter and not part of the path.
    while i > 0 && (text[i - 1].is_alphanumeric() || text[i - 1] == '_') {
        i -= 1;
    }
    if i == 0 || text[i - 1] != '.' {
        return None;
    }
    i -= 1;
    let end = i;
    while i > 0 && (text[i - 1].is_alphanumeric() || text[i - 1] == '_' || text[i - 1] == '.') {
        i -= 1;
    }
    let path: Vec<String> = text[i..end].iter().collect::<String>().split('.').map(str::to_string).collect();
    if path.iter().any(String::is_empty) {
        return None;
    }
    Some(path)
}

struct Cx<'a> {
    p: &'a Program,
    scope: Vec<HashMap<String, Typed>>,
    /// Every word the host's registries define, so that a name which is
    /// neither in scope nor declared here can be told from a typo.
    words: std::collections::BTreeSet<String>,
    /// The registries themselves, for the props that introduce a name rather
    /// than reading one — a form field is called something, and that something
    /// is written where a value would go.
    hosts: crate::capability::Hosts,
    /// The names in each scope that were declared `let`. Held beside the types
    /// rather than inside them: what a name means and whether it may be written
    /// again are different questions, and only one of them is a type.
    mutable: Vec<HashSet<String>>,
    /// Where an editor's cursor is, when this run is answering that question
    /// rather than checking a file, and the block it must be inside for a
    /// binding to count.
    at: Option<(u32, u32)>,
    within: Option<(Span, Span)>,
    /// The scope stack as it stood after the last statement before the cursor,
    /// with that statement's position — kept rather than the first one found,
    /// because declarations are not visited in the order they were written and
    /// the nearest one is the one whose names are in scope.
    seen: Option<((u32, u32), Vec<HashMap<String, Typed>>)>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Cx<'a> {
    fn new(p: &'a Program) -> Self {
        Cx {
            p,
            words: Default::default(),
            hosts: Default::default(),
            scope: vec![HashMap::new()],
            mutable: vec![HashSet::new()],
            at: None,
            within: None,
            seen: None,
            diagnostics: Vec::new(),
        }
    }

    fn err(&mut self, span: Span, msg: impl Into<String>) {
        self.diagnostics.push(Diagnostic::error(span, msg));
    }

    fn push(&mut self) {
        self.scope.push(HashMap::new());
        self.mutable.push(HashSet::new());
    }
    fn pop(&mut self) {
        self.scope.pop();
        self.mutable.pop();
    }
    fn is_mutable(&self, name: &str) -> bool {
        self.mutable.iter().rev().any(|s| s.contains(name))
    }
    fn bind(&mut self, name: &str, t: Typed) {
        self.scope.last_mut().unwrap().insert(name.to_string(), t);
    }
    fn lookup(&self, name: &str) -> Option<&Typed> {
        self.scope.iter().rev().find_map(|s| s.get(name))
    }

    // ------------------------------------------------------------- declared

    fn resolve(&self, t: &TypeRef) -> Ty {
        let base = match t.name.as_str() {
            "int" => Ty::Int,
            "string" => Ty::Str,
            "bool" => Ty::Bool,
            "date" => Ty::Date,
            "datetime" => Ty::DateTime,
            "bytes" => Ty::Bytes,
            "List" => Ty::List(Box::new(t.args.first().map(|a| self.resolve(a)).unwrap_or(Ty::Unknown))),
            "Credential" => Ty::Credential(t.args.first().map(|a| a.name.clone()).unwrap_or_default()),
            "Verified" => Ty::Verified(t.args.first().map(|a| a.name.clone()).unwrap_or_default()),
            "Proof" => Ty::Proof,
            other if self.p.enums.iter().any(|e| e.name == other) => Ty::Enum(other.into()),
            other if self.credential(other).is_some() => Ty::Record(other.into()),
            _ => Ty::Unknown,
        };
        if t.optional {
            base.optional()
        } else {
            base
        }
    }

    /// A plain record and a credential's claims resolve the same way: the
    /// difference is who signed them, which is a question for `verify` and not
    /// for field access.
    fn credential(&self, name: &str) -> Option<&'a CredentialDecl> {
        self.p.credentials.iter().chain(&self.p.types).find(|c| c.name == name)
    }
    fn policy(&self, name: &str) -> Option<&'a TrustDecl> {
        self.p.trusts.iter().find(|t| t.name == name)
    }

    fn state_type(&self, path: &[String]) -> Option<Ty> {
        let head = self.p.state.iter().find(|f| f.name == path[0])?;
        let mut ty = self.resolve(&head.ty);
        for seg in &path[1..] {
            let name = match ty.inner() {
                Ty::Record(n) | Ty::Claims(n) => n.clone(),
                _ => return None,
            };
            let f = self.credential(&name)?.fields.iter().find(|f| &f.name == seg)?;
            ty = self.resolve(&f.ty);
        }
        Some(ty)
    }

    // -------------------------------------------------------------- entries

    fn function(&mut self, f: &FunctionDecl) {
        self.push();
        for param in &f.params {
            let t = self.resolve(&param.ty);
            self.bind(&param.name, Typed::plain(t));
        }
        for s in &f.body {
            self.stmt(s, None);
        }
        self.pop();
    }

    fn screen(&mut self, s: &ScreenDecl) {
        self.push();
        for d in &s.data {
            let t = match &d.source {
                // A list of verified credentials, and the policy travels with it.
                DataSource::Credentials { ty, policy: Some(pn), .. } => {
                    let _ = ty;
                    Typed::with(Ty::List(Box::new(Ty::Verified(pn.clone()))), [pn.clone()].into_iter().collect())
                }
                DataSource::Credentials { ty, policy: None, .. } => {
                    Typed::plain(Ty::List(Box::new(Ty::Credential(ty.clone()))))
                }
                // Origin-asserted: no policy, and the origin recorded, so a
                // verifier is told whose word this is rather than being left to
                // assume it is nobody's.
                DataSource::Query { audience } => {
                    Typed::from_origin(Ty::List(Box::new(Ty::Unknown)), &self.p.audience_for(audience))
                }
                DataSource::Unknown => Typed::unknown(),
            };
            self.bind(&d.name, t);
        }
        // What a press handed this screen. Declared the way a component
        // declares its parameters, and in scope for everything it draws.
        for f in &s.params {
            let t = self.resolve(&f.ty);
            self.bind(&f.name, Typed::plain(t));
        }
        for st in &s.compute {
            self.stmt(st, None);
        }
        if let Some(title) = &s.title {
            self.tree(std::slice::from_ref(title));
        }
        self.tree(&s.tree);
        self.pop();
    }

    /// Component call sites, and nothing else.
    fn calls(&mut self, nodes: &[UiNode]) {
        for n in nodes {
            if let Some(decl) = self.p.components.iter().find(|c| c.name == n.kind) {
                let params = decl.params.clone();
                for a in &n.args {
                    let Some(argname) = &a.name else { continue };
                    let Some(param) = params.iter().find(|p| p.name == *argname) else {
                        continue; // `bind` in expansion reports an unknown one.
                    };
                    let want = self.resolve(&param.ty);
                    if let Some(got) = self.typed(&a.value) {
                        if !fits(&got.ty, &want) {
                            self.err(
                                a.span,
                                format!(
                                    "`{}` takes `{argname}` as {want}, and this is {}",
                                    n.kind, got.ty
                                ),
                            );
                        }
                    }
                }
            }
            self.push();
            if let Some(bind) = &n.lambda {
                self.bind(bind, Typed::unknown());
            }
            self.calls(&n.children);
            self.calls(&n.otherwise);
            self.pop();
        }
    }

    fn component(&mut self, c: &ComponentDecl) {
        self.push();
        for f in &c.params {
            let t = self.resolve(&f.ty);
            self.bind(&f.name, Typed::plain(t));
        }
        self.tree(&c.tree);
        self.pop();
    }

    /// Every expression a screen draws.
    ///
    /// Until this existed, nothing in a tree was typed at all: `if
    /// (state.points)` took an int for a condition and the renderer read it as
    /// false, and a component was handed whatever the call site had.
    fn tree(&mut self, nodes: &[UiNode]) {
        for n in nodes {
            if n.kind == "if" {
                if let Some(a) = n.args.first() {
                    if let Some(t) = self.typed(&a.value) {
                        if !matches!(t.ty.inner(), Ty::Bool | Ty::Unknown) {
                            self.err(
                                a.value.span(),
                                format!(
                                    "a condition is true or false, and this is {}. A screen that showed one thing or another on a number would show whichever the host guessed",
                                    t.ty
                                ),
                            );
                        }
                    }
                }
            } else {
                for a in &n.args {
                    // What this prop was declared to hold, if the registry
                    // knows the node. Three types hold a name rather than read
                    // one — `into:` calls the field the wallet will keep what
                    // is typed under, and `onTap:` names an action or a screen,
                    // which something else checks and better — and a prop whose
                    // type is a vocabulary holds a word, checked where the
                    // registry is read. What is left reads a value, and a bare
                    // name in one is either bound or a mistake.
                    // A capability's own props, then the ones every drawn
                    // thing has — layout, accessibility, style. `color` is on
                    // none of them and on all of them.
                    let common = self.hosts.common();
                    let declared = a.name.as_ref().and_then(|name| {
                        self.hosts
                            .find(&n.kind)
                            .and_then(|(_, cap)| cap.props.get(name).cloned())
                            .or_else(|| common.get(name).cloned())
                    });
                    let elsewhere = declared.as_deref().is_some_and(|ty| {
                        matches!(ty.trim_end_matches('?'), "Name" | "Action" | "Screen")
                            || self.hosts.vocabulary_for_type(ty).is_some()
                    });
                    if !elsewhere {
                        self.a_name_or_a_word(&a.value);
                    }
                    self.value(&a.value);
                }
            }

            // `list(receipts) { r -> … }` introduces the row, and it is the one
            // name in this language that an interface introduces rather than
            // the program.
            self.push();
            if let Some(bind) = &n.lambda {
                let row = n
                    .args
                    .first()
                    .and_then(|a| self.typed(&a.value))
                    .map(|t| match t.ty.inner() {
                        Ty::List(inner) => (**inner).clone(),
                        _ => Ty::Unknown,
                    })
                    .unwrap_or(Ty::Unknown);
                self.bind(bind, Typed::plain(row));
            }
            self.tree(&n.children);
            self.tree(&n.otherwise);
            self.pop();
        }
    }

    /// A bare name in a tree is one of three things, and a fourth is a mistake.
    ///
    /// It is bound — state, data, a computed value, a parameter, a row — or it
    /// is a word the host's registry defines, or it names an action or a screen
    /// a press moves to. Anything else was drawn as itself: a misspelt binding
    /// reached the screen as the word somebody typed.
    fn a_name_or_a_word(&mut self, e: &Expr) {
        let (name, span) = match e {
            Expr::Ident { name, span } => (name.clone(), *span),
            Expr::Member { obj, .. } => return self.a_name_or_a_word(obj),
            _ => return,
        };
        if !self.is_word(e) || self.words.is_empty() {
            return;
        }
        let known = self.words.contains(&name)
            || self.p.actions.iter().any(|a| a.name == name)
            || self.p.screens.iter().any(|s| s.name == name)
            || self.p.trusts.iter().any(|t| t.name == name);
        if !known {
            self.err(
                span,
                format!("`{name}` is neither something this program declares nor a word this host has"),
            );
        }
    }

    /// One value written on a node.
    ///
    /// Three shapes, because a screen's arguments are not only expressions:
    /// `onTap: Receipt(merchant: …)` moves to a screen and its arguments are
    /// that screen's parameters, and `onTap: navigation.back(with: …)` calls
    /// something the host provides, whose name this crate does not have.
    fn value(&mut self, e: &Expr) {
        match e {
            Expr::Call { callee, args, span } => {
                if let Expr::Ident { name, .. } = &**callee {
                    if let Some(screen) = self.p.screens.iter().find(|s| s.name == *name) {
                        let params = screen.params.clone();
                        for a in args {
                            self.value(&a.value);
                            let Some(argname) = &a.name else { continue };
                            let Some(param) = params.iter().find(|p| p.name == *argname) else {
                                self.err(
                                    a.span,
                                    format!("`{name}` takes no `{argname}`"),
                                );
                                continue;
                            };
                            let want = self.resolve(&param.ty);
                            if let Some(got) = self.typed(&a.value) {
                                if !fits(&got.ty, &want) {
                                    self.err(
                                        a.span,
                                        format!("`{name}` takes `{argname}` as {want}, and this is {}", got.ty),
                                    );
                                }
                            }
                        }
                        for param in &params {
                            let given = args.iter().any(|a| a.name.as_deref() == Some(&param.name));
                            if !given && !param.ty.optional && param.default.is_none() {
                                self.err(
                                    *span,
                                    format!("`{name}` takes `{}` and this does not give it", param.name),
                                );
                            }
                        }
                        return;
                    }
                }
                // Something the host provides. Its arguments are still this
                // package's expressions, so they are checked; the call is not,
                // because what it takes is in the host's registry.
                if self.is_word(callee) {
                    for a in args {
                        self.value(&a.value);
                    }
                    return;
                }
                self.typed(e);
            }
            _ => {
                self.typed(e);
            }
        }
    }

    /// The type of an expression, or nothing where the expression is a word.
    ///
    /// A screen is full of words from the host's vocabulary — `primary`,
    /// `money`, `foreground.primary` — which parse as names and are bound by
    /// nothing. The same rule the runtime uses: a path whose root is not in
    /// scope is a word, not a mistake.
    fn typed(&mut self, e: &Expr) -> Option<Typed> {
        if self.is_word(e) {
            return None;
        }
        Some(self.expr(e))
    }

    fn is_word(&self, e: &Expr) -> bool {
        let mut root = e;
        loop {
            match root {
                Expr::Member { obj, .. } => root = obj,
                Expr::Ident { name, .. } => {
                    // The roots the runtime binds. `state` is not a field of
                    // itself, and reading it as a word is how `if (state.points)`
                    // went unchecked.
                    if matches!(name.as_str(), "state" | "input" | "context" | "next") {
                        return false;
                    }
                    return self.lookup(name).is_none()
                        && self.p.state.iter().all(|f| f.name != *name)
                        && self.p.enums.iter().all(|d| d.name != *name)
                        && self.p.functions.iter().all(|f| f.name != *name)
                        && self.p.credentials.iter().all(|c| c.name != *name)
                        && self.p.types.iter().all(|t| t.name != *name)
                }
                _ => return false,
            }
        }
    }

    fn action(&mut self, a: &ActionDecl) {
        self.push();
        for block in &a.phases {
            for s in &block.stmts {
                self.stmt(s, Some(block.phase));
            }
        }
        self.pop();
    }

    fn stmt(&mut self, s: &Stmt, phase: Option<Phase>) {
        self.typed_stmt(s, phase);
        // After, not before: `const a = 1` is in scope on the line under it,
        // which is where somebody asking what they may write is standing.
        self.note(s.span());
    }

    /// The scope here, if this is the nearest statement before the cursor.
    fn note(&mut self, sp: Span) {
        let (Some(at), Some((from, to))) = (self.at, self.within) else { return };
        let here = (sp.line, sp.col);
        let inside = here >= (from.line, from.col) && here <= (to.line, to.col);
        if !inside || here >= at {
            return;
        }
        if self.seen.as_ref().is_none_or(|(best, _)| *best < here) {
            self.seen = Some((here, self.scope.clone()));
        }
    }

    fn typed_stmt(&mut self, s: &Stmt, phase: Option<Phase>) {
        match s {
            Stmt::Binding { name, ty, .. } => {
                let t = self.resolve(ty);
                self.bind(name, Typed::plain(t));
            }
            // Every field takes the record's provenance, because that is where
            // it came from: pulling `amount` out of a verified receipt does not
            // make it a number somebody typed.
            Stmt::Destructure { names, value, mutable, span } => {
                let t = self.expr(value);
                for n in names {
                    let field = self.member(value, n, *span);
                    self.bind(n, Typed { ty: field.ty, from: t.from.clone(), origins: t.origins.clone() });
                    if *mutable {
                        self.mutable.last_mut().unwrap().insert(n.clone());
                    }
                }
            }
            Stmt::Data { name, source, .. } => {
                let t = match source {
                    DataSource::Credentials { ty, policy: Some(pn), .. } => {
                        let _ = ty;
                        Typed::with(Ty::List(Box::new(Ty::Verified(pn.clone()))), [pn.clone()].into_iter().collect())
                    }
                    DataSource::Credentials { ty, policy: None, .. } => {
                        Typed::plain(Ty::List(Box::new(Ty::Credential(ty.clone()))))
                    }
                    DataSource::Query { audience } => {
                        Typed::from_origin(Ty::List(Box::new(Ty::Unknown)), &self.p.audience_for(audience))
                    }
                    DataSource::Unknown => Typed::unknown(),
                };
                self.bind(name, t);
            }
            Stmt::Let { name, value, mutable, .. } => {
                let t = self.expr(value);
                self.bind(name, t);
                if *mutable {
                    self.mutable.last_mut().unwrap().insert(name.clone());
                }
            }

            // A phase is what it is: `require` and `verify` hold conditions,
            // `update` is a patch and `execute` is a batch of effects. What is
            // left is the two places whose whole purpose is working a value
            // out, and those are where a name may be written again.
            Stmt::Assign { name, value, span } => {
                let t = self.expr(value);
                let allowed = matches!(phase, None | Some(Phase::Compute));
                if !allowed {
                    let p = phase.map(|p| p.name()).unwrap_or("this block");
                    self.err(
                        *span,
                        format!("`{p}` is not where a value is worked out, so nothing is written again here"),
                    );
                } else if self.lookup(name).is_none() {
                    self.err(*span, format!("`{name}` was never declared. Write `let {name} = …` first"));
                } else if !self.is_mutable(name) {
                    self.err(
                        *span,
                        format!("`{name}` is a `const`, so it is what it was defined as. Declare it `let` to write it again"),
                    );
                } else {
                    let want = self.lookup(name).map(|x| x.ty.clone());
                    if let Some(want) = want {
                        if !fits(&t.ty, &want) {
                            self.err(
                                *span,
                                format!("`{name}` is {want}, and this is {}", t.ty),
                            );
                        }
                    }
                    self.bind(name, t);
                }
            }
            Stmt::Return { value, .. } => {
                self.expr(value);
            }
            Stmt::Expr { value, span } => {
                let t = self.expr(value);
                if matches!(phase, Some(Phase::Require) | Some(Phase::Verify))
                    && !t.ty.is_unknown()
                    && t.ty != Ty::Bool
                {
                    self.err(
                        *span,
                        format!(
                            "a line in `{}` is a condition, and this one is `{}`",
                            phase.unwrap().name(),
                            t.ty
                        ),
                    );
                }
                // `exists` narrows for everything after it in this action.
                if let Expr::Exists { subject, .. } = value {
                    if let Some(path) = subject.path() {
                        self.narrow(&path);
                    }
                }
            }
            Stmt::Patch { path, value, span } => {
                let got = self.expr(value);
                match self.state_type(path) {
                    Some(want) => {
                        if !want.accepts(&got.ty) {
                            self.err(
                                *span,
                                format!("`{}` is `{want}`, and this is `{}`", path.join("."), got.ty),
                            );
                        }
                    }
                    None => self.err(
                        *span,
                        format!("`{}` is not a field of `state`", path.join(".")),
                    ),
                }
            }
            Stmt::Refuse { .. } => {}
            Stmt::If { cond, then, other, .. } => {
                let c = self.expr(cond);
                if !c.ty.is_unknown() && c.ty != Ty::Bool {
                    self.err(cond.span(), format!("a condition is `bool`, and this is `{}`", c.ty));
                }
                // Each branch is its own scope: a binding made in one is not in
                // the other, and neither survives the statement.
                for (branch, _) in [(then, 0), (other, 1)] {
                    self.push();
                    for s in branch {
                        self.stmt(s, phase);
                    }
                    self.pop();
                }
            }
            Stmt::Effect { name, args, body, span } => {
                for a in args {
                    let t = self.expr(&a.value);
                    // A proof asserts something about data an issuer stood
                    // behind. Over an API's answer it asserts something nobody
                    // stood behind, in a form that looks exactly as strong.
                    if name == "prove" && !t.origins.is_empty() {
                        self.err(
                            a.value.span(),
                            format!(
                                "this proves something about data from {}, which nobody signed. A proof over an origin-asserted value looks exactly as strong as a proof over a credential and is not — verify a credential, or disclose the number and say where it came from",
                                t.origins.iter().cloned().collect::<Vec<_>>().join(", ")
                            ),
                        );
                    }
                    // `credential.issue(LoyaltyMember { … })`: the claims being
                    // signed are the place provenance has to be demanded, since
                    // it is the only point where this application's word becomes
                    // somebody else's evidence.
                    if name == "credential.issue" {
                        self.issue(&a.value, &t, *span);
                    }
                }
                for s in body {
                    self.stmt(s, phase);
                }
            }
        }
    }

    /// Narrow `state.member` from `T?` to `T` for the rest of the action.
    fn narrow(&mut self, path: &str) {
        let key = format!("__narrow:{path}");
        self.bind(&key, Typed::plain(Ty::Bool));
    }
    fn narrowed(&self, path: &str) -> bool {
        self.lookup(&format!("__narrow:{path}")).is_some()
    }

    fn issue(&mut self, e: &Expr, _t: &Typed, span: Span) {
        let Expr::Call { callee, args, .. } = e else { return };
        let Some(name) = callee.path() else { return };
        let Some(decl) = self.credential(&name) else {
            self.err(span, format!("`{name}` is not a credential this program declares"));
            return;
        };
        for a in args {
            let Expr::Record { fields, .. } = &a.value else { continue };
            for (field, value) in fields {
                let want = decl.fields.iter().find(|f| &f.name == field);
                let Some(want) = want else {
                    self.err(value.span(), format!("`{name}` has no claim called `{field}`"));
                    continue;
                };
                let got = self.expr(value);
                if !got.origins.is_empty() {
                    self.err(
                        value.span(),
                        format!(
                            "`{name}.{field}` would be signed by this application's publisher, and this value came from {}. Issuing somebody else's unsigned answer under your own name is the one thing a credential must not be able to say",
                            got.origins.iter().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    );
                }
                let want_ty = self.resolve(&want.ty);
                if !want_ty.accepts(&got.ty) {
                    self.err(
                        value.span(),
                        format!("`{name}.{field}` is `{want_ty}`, and this is `{}`", got.ty),
                    );
                }
                // `points: … from { ReceiptFromMerchant }`
                if let Expr::From { policies, span, .. } = value {
                    for want in policies {
                        if !got.from.contains(want) {
                            self.err(
                                *span,
                                format!(
                                    "this claim requires `{want}`, and the value descends from {}. A claim may be computed only from data verified under the policy it names — a credential carries the provenance of each claim, and whoever receives it next does not have to take our signature's word for how the number was reached",
                                    describe_provenance(&got.from)
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    fn refinements(&mut self) {
        for t in &self.p.trusts {
            let Some(base_name) = &t.refines else { continue };
            let Some(base) = self.policy(base_name) else {
                self.err(t.span, format!("`{base_name}` is not a trust policy this program declares"));
                continue;
            };
            // Syntactic containment, never semantic implication: a checker that
            // decides one predicate implies another is a checker that is wrong
            // quietly.
            let mine: Vec<String> = t.requires.iter().filter_map(|e| e.path()).collect();
            for req in &base.requires {
                if let Some(path) = req.path() {
                    let want = path.replace(&base.subject, &t.subject);
                    if !mine.iter().any(|m| *m == want || *m == path) {
                        self.err(
                            t.span,
                            format!(
                                "`{}` claims to refine `{base_name}` and does not require `{want}`. Refinement is checked as containment, one predicate at a time",
                                t.name
                            ),
                        );
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------- expressions

    fn expr(&mut self, e: &Expr) -> Typed {
        match e {
            Expr::Num { .. } => Typed::plain(Ty::Int),
            // A list written out. Every element is checked; what the list is a
            // list *of* is left alone, because the first use of one is a table's
            // columns and a picker's options, where the elements are strings and
            // the check that matters is the one on each of them.
            Expr::List { items, .. } => {
                for i in items {
                    self.expr(i);
                }
                Typed::unknown()
            }
            Expr::Float { .. } => Typed::unknown(),
            Expr::Str { .. } => Typed::plain(Ty::Str),
            Expr::Bool { .. } => Typed::plain(Ty::Bool),
            Expr::Error { .. } => Typed::unknown(),

            Expr::Ident { name, span } => {
                if name == "state" || name == "context" || name == "next" {
                    return Typed::plain(Ty::Unknown);
                }
                // `Date.now()` is one mistake with one sentence, in `check.rs`.
                // Reporting the receiver and the call as well would bury it.
                if is_nondeterministic_root(name) {
                    return Typed::plain(Ty::Unknown);
                }
                if let Some(t) = self.lookup(name) {
                    return t.clone();
                }
                if self.p.enums.iter().any(|en| en.name == *name) {
                    return Typed::plain(Ty::Enum(name.clone()));
                }
                if self.p.functions.iter().any(|f| f.name == *name) || is_builtin(name) {
                    return Typed::plain(Ty::Lambda);
                }
                // A screen is a place, not a value: `navigate(to: Done)` and
                // `onTap: Done` name one. Looking it up as a binding would make
                // every navigation an undefined name.
                if self.p.screens.iter().any(|s| s.name == *name)
                    || self.p.actions.iter().any(|a| a.name == *name)
                {
                    return Typed::plain(Ty::Unknown);
                }
                // **A credential's name, as the thing itself.** `capabilities`
                // has always taken one — `credential.check(EmployeeBadge)` —
                // and a screen that draws a card needs the same reference:
                // `credentialCard(of: EmployeeBadge)` names which card the host
                // is to draw. Nothing readable comes with it, which is the
                // point: a held credential has no claims until a `verify` block
                // produces a `Verified<…>`, so naming one here draws a card and
                // reads nothing off it.
                if self.p.credentials.iter().any(|c| c.name == *name) {
                    return Typed::plain(Ty::Credential(name.clone()));
                }
                // A word from a closed vocabulary — `primary`, `replace`,
                // `sheet`. Which words exist is the host's document, checked
                // there; here it is enough that a lowercase bare name in an
                // argument is a word rather than a binding.
                if name.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
                    return Typed::plain(Ty::Unknown);
                }
                self.err(*span, format!("no name `{name}` is in scope here"));
                Typed::unknown()
            }

            Expr::Member { obj, name, optional, span } => {
                let t = self.member(obj, name, *span);
                // `a?.b` is nothing when `a` is, so the path is optional
                // whatever the field was declared as.
                if *optional {
                    Typed { ty: Ty::Optional(Box::new(t.ty.inner().clone())), ..t }
                } else {
                    t
                }
            }

            Expr::Unary { rhs, .. } => self.expr(rhs),

            Expr::Binary { op, lhs, rhs, span } => {
                let a = self.expr(lhs);
                let b = self.expr(rhs);
                let ty = match op.as_str() {
                    "+" | "-" | "*" | "/" | "%" => {
                        for (t, e) in [(&a, lhs.as_ref()), (&b, rhs.as_ref())] {
                            if !t.ty.is_unknown() && !matches!(t.ty, Ty::Int | Ty::Date | Ty::DateTime) {
                                self.err(e.span(), format!("arithmetic is on integers, and this is `{}`", t.ty));
                            }
                        }
                        // A duration is an integer, because the language has no
                        // other number. That must not turn a time into one.
                        match (&a.ty, &b.ty) {
                            (Ty::DateTime, _) | (_, Ty::DateTime) => Ty::DateTime,
                            (Ty::Date, _) | (_, Ty::Date) => Ty::Date,
                            _ => Ty::Int,
                        }
                    }
                    // `0...10` is the integers from one to the other.
                    "..." => {
                        for (t, e) in [(&a, lhs.as_ref()), (&b, rhs.as_ref())] {
                            if !t.ty.is_unknown() && !matches!(t.ty, Ty::Int) {
                                self.err(
                                    e.span(),
                                    format!("a range runs between integers, and this is `{}`", t.ty),
                                );
                            }
                        }
                        // A range written out is the one list whose length is
                        // on the page, so the limit that the runtime enforces
                        // is worth saying here instead of at the moment
                        // somebody's screen fails to draw.
                        if let (Expr::Num { value: from, .. }, Expr::Num { value: to, .. }) =
                            (lhs.as_ref(), rhs.as_ref())
                        {
                            if to.saturating_sub(*from).saturating_add(1) > RANGE_LIMIT {
                                self.err(
                                    lhs.span().to(rhs.span()),
                                    format!("a range of {from} to {to} is more steps than a screen can be made of. The limit is {RANGE_LIMIT}"),
                                );
                            }
                        }
                        Ty::List(Box::new(Ty::Int))
                    }
                    "<" | "<=" | ">" | ">=" => {
                        if !a.ty.accepts(&b.ty) && !b.ty.accepts(&a.ty) {
                            self.err(*span, format!("`{}` and `{}` do not compare", a.ty, b.ty));
                        }
                        Ty::Bool
                    }
                    "==" | "!=" => {
                        if !a.ty.accepts(&b.ty) && !b.ty.accepts(&a.ty) {
                            self.err(*span, format!("`{}` and `{}` are never equal — they are different types", a.ty, b.ty));
                        }
                        Ty::Bool
                    }
                    _ => Ty::Bool,
                };
                Typed::join(&a, &b, ty)
            }

            Expr::Ternary { cond, then, other, .. } => {
                let c = self.expr(cond);
                if !c.ty.is_unknown() && c.ty != Ty::Bool {
                    self.err(cond.span(), format!("a condition is `bool`, and this is `{}`", c.ty));
                }
                let t = self.expr(then);
                let o = self.expr(other);
                let ty = if t.ty.accepts(&o.ty) { t.ty.clone() } else { o.ty.clone() };
                Typed::join(&t, &o, ty)
            }

            // `a ?: b` — the left side unless it is nothing, so the type is the
            // left side with its optionality removed, or the right side's.
            Expr::Elvis { subject, other, .. } => {
                let a = self.expr(subject);
                let b = self.expr(other);
                let mut from = a.from.clone();
                from.extend(b.from.clone());
                let mut origins = a.origins.clone();
                origins.extend(b.origins.clone());
                let ty = if a.ty.is_unknown() { b.ty.clone() } else { a.ty.inner().clone() };
                Typed { ty, from, origins }
            }

            Expr::Exists { subject, .. } => {
                self.expr(subject);
                Typed::plain(Ty::Bool)
            }

            Expr::With { subject, policy, span } => {
                let s = self.expr(subject);
                let Some(decl) = self.policy(policy) else {
                    self.err(
                        *span,
                        format!("`{policy}` is not a trust policy this program declares. A policy is the only way to obtain a `Verified<…>`, so there is nothing this could produce"),
                    );
                    return Typed::unknown();
                };
                let wanted = decl.subject_type.clone();
                match &s.ty {
                    Ty::Credential(held) if !wanted.is_empty() && *held != wanted => {
                        self.err(
                            *span,
                            format!("`{policy}` is a policy about `{wanted}`, and this is a `Credential<{held}>`"),
                        );
                    }
                    Ty::Verified(had) => {
                        self.err(
                            *span,
                            format!("this is already `Verified<{had}>`. Verifying twice does not make it more verified; if `{policy}` is genuinely stronger, declare `{policy} refines {had}`"),
                        );
                    }
                    _ => {}
                }
                let mut from = s.from.clone();
                from.insert(policy.clone());
                Typed::with(Ty::Verified(policy.clone()), from)
            }

            Expr::From { value, .. } => self.expr(value),

            Expr::Record { spread, fields, .. } => {
                let mut from = Provenance::new();
                if let Some(s) = spread {
                    from.extend(self.expr(s).from);
                }
                for (_, v) in fields {
                    from.extend(self.expr(v).from);
                }
                Typed::with(Ty::Record(String::new()), from)
            }

            Expr::Switch { subject, arms, .. } => {
                let s = self.expr(subject);
                let mut out: Option<Typed> = None;
                for a in arms {
                    match &a.pattern {
                        ArmPattern::Value(v) => {
                            let p = self.expr(v);
                            if !s.ty.accepts(&p.ty) && !p.ty.accepts(&s.ty) {
                                self.err(v.span(), format!("this switch is over `{}`, and this arm is `{}`", s.ty, p.ty));
                            }
                        }
                        ArmPattern::Compare { rhs, .. } => {
                            self.expr(rhs);
                        }
                        ArmPattern::Default => {}
                    }
                    let b = self.expr(&a.body);
                    out = Some(match out {
                        None => b,
                        Some(prev) => Typed::join(&prev, &b, prev.ty.clone()),
                    });
                }
                out.unwrap_or_else(Typed::unknown)
            }

            Expr::Lambda { params, body, .. } => {
                self.push();
                for p in params {
                    self.bind(p, Typed::unknown());
                }
                let t = self.expr(body);
                self.pop();
                Typed::with(Ty::Lambda, t.from)
            }

            Expr::Call { callee, args, span } => {
                // `xs.fold(0) { … }` and friends keep the list's provenance.
                if let Expr::Member { obj, name, .. } = callee.as_ref() {
                    let recv = self.expr(obj);
                    if let Ty::List(item) = recv.ty.clone() {
                        let mut from = recv.from.clone();
                        let mut origins = recv.origins.clone();
                        for a in args {
                            let t = self.expr(&a.value);
                            from.extend(t.from);
                            origins.extend(t.origins);
                        }
                        // `map`, `filter`, `fold`, `any` and `all` are given a
                        // function: written where they are used, or named. One
                        // that is given neither used to return the list
                        // unchanged, so the mistake ran and did nothing.
                        if matches!(name.as_str(), "map" | "filter" | "fold" | "any" | "all") {
                            let given = args.iter().any(|a| match &a.value {
                                Expr::Lambda { .. } => true,
                                Expr::Ident { name, .. } => {
                                    self.p.functions.iter().any(|f| f.name == *name)
                                }
                                _ => false,
                            });
                            if !given {
                                let at = obj.span().to(*span);
                                self.err(
                                    at,
                                    format!("`{name}` is given a function — `{{ r -> … }}`, or the name of one this package declares"),
                                );
                            }
                            // A named function takes the row, and `fold` hands
                            // it the running value first.
                            let wants = if name == "fold" { 2 } else { 1 };
                            for a in args {
                                let Expr::Ident { name: fname, span: fspan } = &a.value else {
                                    continue;
                                };
                                let Some(f) = self.p.functions.iter().find(|f| f.name == *fname)
                                else {
                                    continue;
                                };
                                if f.params.len() != wants {
                                    let n = f.params.len();
                                    self.err(
                                        *fspan,
                                        format!("`{name}` hands over {wants} value(s), and `{fname}` takes {n}"),
                                    );
                                }
                            }
                        }
                        let out = |ty| Typed { ty, from: from.clone(), origins: origins.clone() };
                        return match name.as_str() {
                            "fold" | "count" => out(Ty::Int),
                            "map" | "filter" => out(Ty::List(item)),
                            "any" | "all" => out(Ty::Bool),
                            "first" => out(Ty::Optional(item)),
                            other => {
                                self.err(*span, format!("a list has no `{other}`. It is consumed by `map`, `filter`, `fold`, `any`, `all`, `count` and `first`"));
                                out(Ty::Unknown)
                            }
                        };
                    }
                }

                let mut from = Provenance::new();
                for a in args {
                    from.extend(self.expr(&a.value).from);
                }
                let Some(name) = callee.path() else { return Typed::with(Ty::Unknown, from) };

                // Named once there are two of them (§2).
                let constructing = args.len() == 1 && matches!(args[0].value, Expr::Record { .. });
                // A phrase's words come first and are not named: the sentence
                // is the thing, and its values are named beside it.
                if name == crate::expand::PHRASE {
                    for a in args.iter().skip(1) {
                        self.value(&a.value);
                    }
                    return Typed::plain(Ty::Str);
                }
                // `fold(0) { acc, x -> … }` reads as one argument and a block,
                // which is how it is written and how it should be counted.
                let named_count = args.iter().filter(|a| !matches!(a.value, Expr::Lambda { .. })).count();
                if named_count > 1 && !constructing && args.iter().any(|a| a.name.is_none() && !matches!(a.value, Expr::Lambda { .. })) {
                    self.err(
                        *span,
                        format!("`{name}` takes {} arguments, so they are named. A call site is read far more often than it is written, frequently by somebody deciding whether to approve what it does", args.len()),
                    );
                }

                if let Some(f) = self.p.functions.iter().find(|f| f.name == name) {
                    if f.params.len() != args.len() {
                        self.err(
                            *span,
                            format!("`{name}` takes {} argument(s), and this passes {}", f.params.len(), args.len()),
                        );
                    }
                    for (param, arg) in f.params.iter().zip(args) {
                        let want = self.resolve(&param.ty);
                        let got = self.expr(&arg.value);
                        if !want.accepts(&got.ty) {
                            self.err(
                                arg.value.span(),
                                match (&want, &got.ty) {
                                    (Ty::Verified(p), Ty::Credential(c)) => format!(
                                        "expected `Verified<{p}>`, found `Credential<{c}>`. Pass it through a `verify` block; there is no cast, because a cast is how the check gets forgotten"
                                    ),
                                    (Ty::Verified(want_p), Ty::Verified(had)) => format!(
                                        "expected `Verified<{want_p}>`, found `Verified<{had}>`. Policies are nominal: a signature that is valid says nothing about whether the credential was revoked, or about who issued it. If one policy really does subsume the other, declare `{had} refines {want_p}`"
                                    ),
                                    _ => format!("`{}` expects `{want}` here, and this is `{}`", param.name, got.ty),
                                },
                            );
                        }
                    }
                    let ret = f.ret.as_ref().map(|r| self.resolve(r)).unwrap_or(Ty::Unknown);
                    return Typed::with(ret, from);
                }

                if self.credential(&name).is_some() {
                    return Typed::with(Ty::Record(name.clone()), from);
                }
                if is_builtin(&name) {
                    return Typed::with(builtin_type(&name), from);
                }
                // An effect in the wrong phase is one mistake, and `check.rs`
                // has already said so. Saying it again as "no such function"
                // buries the sentence that taught the rule.
                if is_effect_call(&name) {
                    return Typed::with(Ty::Unknown, from);
                }
                if constructing {
                    self.err(*span, format!("`{name}` is not a credential this program declares"));
                    return Typed::with(Ty::Unknown, from);
                }
                if name.split('.').next().is_some_and(is_nondeterministic_root) {
                    return Typed::with(Ty::Unknown, from);
                }
                // A method on something this pass could not type. The mistake,
                // if there is one, was reported where the type was lost.
                if let Expr::Member { obj, .. } = callee.as_ref() {
                    if self.expr(obj).ty.is_unknown() {
                        return Typed::with(Ty::Unknown, from);
                    }
                }
                self.err(
                    *span,
                    format!("no function named `{name}`. The library is closed: an application cannot add to it, because a builtin is the one place a non-terminating operation could enter a language that has proved it cannot have one"),
                );
                Typed::with(Ty::Unknown, from)
            }
        }
    }

    /// What a name means on a type, with nothing said about it being wrong.
    ///
    /// The half of `member` that is a lookup rather than a message, so the
    /// checker and the editor answer from one table. They disagreed while there
    /// were two: an editor's own idea of what a credential has is a second
    /// implementation of the rule this language is built on.
    fn member_ty(&self, base: &Ty, name: &str) -> Option<Ty> {
        match base.inner() {
            Ty::Verified(policy) => {
                if name != "claims" {
                    return None;
                }
                let subject = self.policy(policy).map(|t| t.subject_type.clone()).unwrap_or_default();
                Some(Ty::Claims(subject))
            }
            Ty::Claims(record) | Ty::Record(record) => {
                let f = self.credential(record)?.fields.iter().find(|f| f.name == name)?;
                Some(self.resolve(&f.ty))
            }
            Ty::Enum(e) => {
                let declared = self.p.enums.iter().find(|x| &x.name == e)?;
                declared.members.iter().any(|m| m == name).then(|| Ty::Enum(e.clone()))
            }
            _ => None,
        }
    }

    /// Everything that may be written after a dot on this type.
    fn members_of(&self, ty: &Ty) -> Vec<Member> {
        let written = |t: &Ty| t.to_string();
        match ty.inner() {
            // The whole of the paradigm, as a list of one: an issuer's words
            // are behind `claims` and there is no second way in.
            Ty::Verified(policy) => {
                let subject = self.policy(policy).map(|t| t.subject_type.clone()).unwrap_or_default();
                vec![Member { name: "claims".into(), ty: format!("{subject}'s claims"), what: "face" }]
            }
            // A credential that has not been through `verify` has nothing
            // readable, which is the point of it.
            Ty::Credential(_) => Vec::new(),
            Ty::Claims(record) | Ty::Record(record) => self
                .credential(record)
                .map(|d| {
                    d.fields
                        .iter()
                        .map(|f| Member {
                            name: f.name.clone(),
                            ty: written(&self.resolve(&f.ty)),
                            what: if matches!(ty.inner(), Ty::Claims(_)) { "claim" } else { "field" },
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Ty::Enum(e) => self
                .p
                .enums
                .iter()
                .find(|x| &x.name == e)
                .map(|d| {
                    d.members
                        .iter()
                        .map(|m| Member { name: m.clone(), ty: e.clone(), what: "member" })
                        .collect()
                })
                .unwrap_or_default(),
            // A list is consumed by the combinators and by nothing else, which
            // is the same closed set the checker refuses anything outside of.
            Ty::List(item) => ["map", "filter", "fold", "any", "all", "count", "first"]
                .iter()
                .map(|name| Member {
                    name: (*name).to_string(),
                    ty: match *name {
                        "fold" | "count" => "int".to_string(),
                        "map" | "filter" => format!("List<{item}>"),
                        "first" => format!("{item}?"),
                        _ => "bool".to_string(),
                    },
                    what: "combinator",
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The roots the host answers rather than the program: `state` and `next`
    /// are the fields this program declares, and `context` is what every action
    /// is handed. Neither is a value with a type, so neither can be reached
    /// through `walk_path` — and both are the first thing anybody types.
    fn root_members(&self, path: &[String]) -> Option<Vec<Member>> {
        let head = path.first()?;
        if (head == "state" || head == "next") && path.len() == 1 {
            return Some(
                self.p
                    .state
                    .iter()
                    .map(|f| Member {
                        name: f.name.clone(),
                        ty: self.resolve(&f.ty).to_string(),
                        what: "state",
                    })
                    .collect(),
            );
        }
        if head == "context" {
            // `time` is a namespace and `now` is what is under it, which is why
            // `context.time.now` reads the way it does.
            let members: &[(&str, &str)] = match path.len() {
                1 => &[("time", "the clock the host answers"), ("uuid", "string")],
                2 if path[1] == "time" => &[("now", "datetime")],
                _ => return None,
            };
            return Some(
                members
                    .iter()
                    .map(|(n, t)| Member { name: (*n).to_string(), ty: (*t).to_string(), what: "context" })
                    .collect(),
            );
        }
        None
    }

    /// A written path, from the names in scope at the cursor to what it is.
    fn walk_path(&self, scope: &[HashMap<String, Typed>], path: &[String]) -> Option<Ty> {
        let head = path.first()?;

        if head == "state" || head == "next" {
            return self.state_type(&path[1..]);
        }

        let mut ty = match scope.iter().rev().find_map(|s| s.get(head)) {
            Some(t) => t.ty.clone(),
            // Not a value but a type: `Tier.` offers the enum's members.
            None if self.p.enums.iter().any(|e| &e.name == head) => Ty::Enum(head.clone()),
            None => return None,
        };
        for seg in &path[1..] {
            ty = self.member_ty(&ty, seg)?;
        }
        Some(ty)
    }

    fn member(&mut self, obj: &Expr, name: &str, span: Span) -> Typed {
        // `context.time.now`, `state.…`, `next.…` are host-shaped roots.
        if let Some(path) = obj.path() {
            if path == "context" || path.starts_with("context.") {
                return Typed::plain(match name {
                    "now" => Ty::DateTime,
                    "uuid" => Ty::Str,
                    _ => Ty::Unknown,
                });
            }
            if path == "state" || path == "next" {
                let ty = self
                    .p
                    .state
                    .iter()
                    .find(|f| f.name == name)
                    .map(|f| self.resolve(&f.ty));
                let Some(ty) = ty else {
                    self.err(span, format!("`state` has no field `{name}`"));
                    return Typed::unknown();
                };
                // Self-asserted: state descends from the chain of records, and
                // from no issuer, so its provenance is empty and a proof over it
                // says so.
                return Typed::plain(ty);
            }
            if let Some(rest) = path.strip_prefix("state.").or_else(|| path.strip_prefix("next.")) {
                let head = rest.split('.').next().unwrap_or(rest);
                if let Some(f) = self.p.state.iter().find(|f| f.name == head) {
                    if self.resolve(&f.ty).inner() != &self.resolve(&f.ty) && !self.narrowed(&format!("state.{head}")) {
                        // Reported by the shape check in `check.rs`; not repeated.
                    }
                }
                let mut segs: Vec<String> = rest.split('.').map(str::to_string).collect();
                segs.push(name.to_string());
                if let Some(ty) = self.state_type(&segs) {
                    return Typed::plain(ty);
                }
                return Typed::unknown();
            }
        }

        let base = self.expr(obj);
        match &base.ty {
            // The one rule that makes the paradigm hold: an issuer's words are
            // behind `Verified<P>`, and there is no other way to reach them.
            Ty::Verified(policy) => {
                let subject = self.policy(policy).map(|t| t.subject_type.clone()).unwrap_or_default();
                match name {
                    "claims" => Typed::with(Ty::Claims(subject), base.from.clone()),
                    "signature" | "status" | "holder" => {
                        self.err(
                            span,
                            format!("`{name}` is readable in `trust` and in `verify`, and nowhere else. An application deciding for itself whether a signature is good enough is the thing `trust` exists to stop"),
                        );
                        Typed::unknown()
                    }
                    other => {
                        self.err(span, format!("a verified credential has `claims`, not `{other}`"));
                        Typed::unknown()
                    }
                }
            }
            Ty::Credential(held) => {
                if name == "claims" {
                    self.err(
                        span,
                        format!("`{held}` is held but not verified, so its claims are out of reach. Pass it through a `verify` block first — that is the only thing that produces a `Verified<…>`, and it is why the check cannot be forgotten"),
                    );
                } else if !matches!(name, "signature" | "status" | "holder") {
                    self.err(span, format!("a credential has `claims`, not `{name}`"));
                }
                Typed::unknown()
            }
            Ty::Claims(cred) => {
                let Some(decl) = self.credential(cred) else { return Typed::unknown() };
                match decl.fields.iter().find(|f| f.name == name) {
                    Some(f) => {
                        let ty = self.resolve(&f.ty);
                        Typed { ty, from: base.from.clone(), origins: base.origins.clone() }
                    }
                    None => {
                        self.err(span, format!("`{cred}` has no claim called `{name}`"));
                        Typed::unknown()
                    }
                }
            }
            Ty::Record(cred) | Ty::Enum(cred) if !cred.is_empty() => {
                let cred = cred.clone();
                if let Some(decl) = self.credential(&cred) {
                    if let Some(f) = decl.fields.iter().find(|f| f.name == name) {
                        let ty = self.resolve(&f.ty);
                        return Typed::with(ty, base.from.clone());
                    }
                    // The type is known, so a field it does not have is a
                    // mistake and not an unknown — saying nothing here is how a
                    // typo reaches a customer.
                    self.err(span, format!("`{cred}` has no field called `{name}`"));
                    return Typed::unknown();
                }
                if let Some(en) = self.p.enums.iter().find(|e| e.name == cred) {
                    if en.members.iter().any(|m| m == name) {
                        return Typed::plain(Ty::Enum(cred.clone()));
                    }
                    self.err(span, format!("`{cred}` has no member `{name}`"));
                }
                Typed::unknown()
            }
            _ => Typed { ty: Ty::Unknown, from: base.from.clone(), origins: base.origins.clone() },
        }
    }
}

fn describe_provenance(p: &Provenance) -> String {
    if p.is_empty() {
        "nothing verified — it is self-asserted".into()
    } else {
        format!("{{ {} }}", p.iter().cloned().collect::<Vec<_>>().join(", "))
    }
}

/// The receivers of the calls `check.rs` refuses for determinism. Named here so
/// one mistake produces one sentence.
fn is_nondeterministic_root(name: &str) -> bool {
    matches!(name, "Date" | "Math")
}

/// The shape of an effect call, not a list of which capabilities exist.
///
/// Which ones a host offers is a document the host publishes — see
/// `interface.rs`. What is needed here is only to tell `credential.issue(…)`
/// from `receipts.fold(…)`: the second is a list walked by one of the closed set
/// of combinators, and everything else dotted is a call to a capability.
fn is_effect_call(call: &str) -> bool {
    match call.split_once('.') {
        Some((_, method)) => !is_combinator(method),
        None => false,
    }
}

fn is_combinator(name: &str) -> bool {
    matches!(name, "map" | "filter" | "fold" | "any" | "all" | "count" | "first")
}

/// The closed set (§3). Adding to it is a language change, deliberately.
fn is_builtin(name: &str) -> bool {
    // `phrase` is not a function anybody calls at run time — it is flattened
    // into the node it sits on, and its words are checked against the signed
    // bundle. It is known here because a component argument is typed before
    // expansion, which is the one place a phrase is still a call.
    matches!(name, "duration" | "min" | "max" | "abs" | crate::expand::PHRASE)
}

fn builtin_type(name: &str) -> Ty {
    match name {
        crate::expand::PHRASE => Ty::Str,
        "duration" => Ty::Int,
        _ => Ty::Int,
    }
}

/// Whether a value of one type may stand where another is declared.
///
/// Deliberately forgiving: `Unknown` is what a query answers with, and a screen
/// that refused every value the host had not described would refuse most real
/// applications. What it catches is the mistake somebody makes — an int where a
/// sentence goes.
fn fits(got: &Ty, want: &Ty) -> bool {
    if matches!(got, Ty::Unknown) || matches!(want, Ty::Unknown) {
        return true;
    }
    match (got, want) {
        // A record literal carries no name — `{ padding: 12 }` is written where
        // a `CardStyle` goes and is one. Matching them by name would refuse
        // every literal, and checking their fields is a different pass than
        // this one.
        (Ty::Record(_), Ty::Record(_)) => true,
        (_, Ty::Optional(inner)) => matches!(got, Ty::Optional(_)) || fits(got, inner),
        (Ty::Optional(inner), _) => fits(inner, want),
        (Ty::List(a), Ty::List(b)) => fits(a, b),
        _ => got == want,
    }
}
