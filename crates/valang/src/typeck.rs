//! Type checking, narrowing and provenance.
//!
//! The rule that does the work is small: **`.claims` is readable on a
//! `Verified<P>` and on nothing else.** Everything a credential says is behind
//! it, so an application cannot reach an issuer's words without having said,
//! in `verify`, which policy it checked them against — and the type it gets
//! back names that policy, so a stricter and a laxer check cannot be confused.

use std::collections::HashMap;

use crate::ast::*;
use crate::diag::{Diagnostic, Span};
use crate::types::{Provenance, Ty, Typed};

pub fn check_types(p: &Program) -> Vec<Diagnostic> {
    let mut cx = Cx::new(p);
    for f in &p.functions {
        cx.function(f);
    }
    for a in &p.actions {
        cx.action(a);
    }
    for s in &p.screens {
        cx.screen(s);
    }
    cx.refinements();
    cx.diagnostics
}

struct Cx<'a> {
    p: &'a Program,
    scope: Vec<HashMap<String, Typed>>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Cx<'a> {
    /// The origin a query answers from is the audience fixed in the manifest,
    /// not the head of the call — `broker.quotes(…)` is an operation on
    /// `broker.co.th`, and reporting the operation as the party would name the
    /// wrong thing in the one place a person is being told who saw their data.
    fn audience_for(&self, head: &str) -> String {
        let declared: Vec<String> = self
            .p
            .capabilities
            .iter()
            .filter(|c| c.name == "api.query")
            .filter_map(|c| {
                c.args.iter().find(|a| a.name.as_deref() == Some("audience")).and_then(|a| match &a.value {
                    Expr::Str { value, .. } => Some(value.clone()),
                    _ => None,
                })
            })
            .collect();
        match declared.as_slice() {
            [only] => only.clone(),
            _ => head.to_string(),
        }
    }

    fn new(p: &'a Program) -> Self {
        Cx { p, scope: vec![HashMap::new()], diagnostics: Vec::new() }
    }

    fn err(&mut self, span: Span, msg: impl Into<String>) {
        self.diagnostics.push(Diagnostic::error(span, msg));
    }

    fn push(&mut self) {
        self.scope.push(HashMap::new());
    }
    fn pop(&mut self) {
        self.scope.pop();
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
                    Typed::from_origin(Ty::List(Box::new(Ty::Unknown)), &self.audience_for(audience))
                }
                DataSource::Unknown => Typed::unknown(),
            };
            self.bind(&d.name, t);
        }
        for st in &s.compute {
            self.stmt(st, None);
        }
        self.pop();
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
        match s {
            Stmt::Binding { name, ty, .. } => {
                let t = self.resolve(ty);
                self.bind(name, Typed::plain(t));
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
                        Typed::from_origin(Ty::List(Box::new(Ty::Unknown)), &self.audience_for(audience))
                    }
                    DataSource::Unknown => Typed::unknown(),
                };
                self.bind(name, t);
            }
            Stmt::Let { name, value, .. } => {
                let t = self.expr(value);
                self.bind(name, t);
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
            Expr::Float { .. } => Typed::unknown(),
            Expr::Str { .. } => Typed::plain(Ty::Str),
            Expr::Bool { .. } => Typed::plain(Ty::Bool),
            Expr::Error { .. } => Typed::unknown(),

            Expr::Ident { name, span } => {
                if name == "state" || name == "context" || name == "next" || is_effect_root(name) {
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
                self.err(*span, format!("no name `{name}` is in scope here"));
                Typed::unknown()
            }

            Expr::Member { obj, name, span } => self.member(obj, name, *span),

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
                if name.split('.').next().is_some_and(is_effect_root) {
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

fn is_effect_root(name: &str) -> bool {
    matches!(name, "credential" | "payment" | "storage" | "message" | "network" | "disclosure")
}

/// The closed set (§3). Adding to it is a language change, deliberately.
fn is_builtin(name: &str) -> bool {
    matches!(name, "duration" | "min" | "max" | "abs")
}

fn builtin_type(name: &str) -> Ty {
    match name {
        "duration" => Ty::Int,
        _ => Ty::Int,
    }
}
