//! The checks. Each one exists to produce a sentence, and the sentences are the
//! ones in `examples/rejected.val` — that file is the checklist, written before
//! this crate and not by the same reasoning.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diag::Diagnostic;

pub fn check(p: &Program) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    effects_only_in_execute(p, &mut d);
    functions_are_pure(p, &mut d);
    capabilities_declared_and_used(p, &mut d);
    at_most_one_disclosure(p, &mut d);
    switches_are_exhaustive(p, &mut d);
    no_unreachable_arms(p, &mut d);
    no_floats(p, &mut d);
    deterministic(p, &mut d);
    call_graph_is_acyclic(p, &mut d);
    narrowing_before_use(p, &mut d);
    patches_have_no_index(p, &mut d);
    updates_take_paths(p, &mut d);
    nothing_is_declared_twice(p, &mut d);
    d
}

/// Two declarations of one name is the kind of mistake that produces a program
/// which runs and is not the program anybody read — the later one wins silently,
/// and which one is later depends on file order in a package with several files.
fn nothing_is_declared_twice(p: &Program, d: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<String, crate::diag::Span> = HashMap::new();
    let mut check = |kind: &str, name: &str, span, d: &mut Vec<Diagnostic>| {
        let key = format!("{kind}:{name}");
        match seen.get(&key) {
            Some(first) => d.push(Diagnostic::error(
                span,
                format!("`{name}` is already declared as a {kind}, on line {}", first.line),
            )),
            None => {
                seen.insert(key, span);
            }
        }
    };

    for e in &p.enums {
        check("type", &e.name, e.span, d);
        let mut members = HashSet::new();
        for m in &e.members {
            if !members.insert(m) {
                d.push(Diagnostic::error(e.span, format!("`{}` lists `{m}` twice", e.name)));
            }
        }
    }
    for c in p.credentials.iter().chain(&p.types) {
        check("type", &c.name, c.span, d);
        let mut fields = HashSet::new();
        for f in &c.fields {
            if !fields.insert(&f.name) {
                d.push(Diagnostic::error(f.span, format!("`{}` has two claims called `{}`", c.name, f.name)));
            }
        }
    }
    for t in &p.trusts {
        check("trust policy", &t.name, t.span, d);
    }
    for f in &p.functions {
        check("function", &f.name, f.span, d);
    }
    for a in &p.actions {
        check("action", &a.name, a.span, d);
        // Phases may be omitted but not repeated: two `compute` blocks is two
        // places to look for what an action computes.
        let mut phases = HashSet::new();
        for b in &a.phases {
            if !phases.insert(b.phase) {
                d.push(Diagnostic::error(
                    b.span,
                    format!("`{}` has two `{}` blocks. A phase appears once", a.name, b.phase.name()),
                ));
            }
        }
    }
    for s in &p.screens {
        check("screen", &s.name, s.span, d);
    }

    let mut fields = HashSet::new();
    for f in &p.state {
        if !fields.insert(&f.name) {
            d.push(Diagnostic::error(f.span, format!("`state` has two fields called `{}`", f.name)));
        }
    }

    let mut caps = HashSet::new();
    for c in &p.capabilities {
        let key = capability_key(&c.name, c.args.first().and_then(|a| a.value.path()).as_deref());
        if !caps.insert(key.clone()) {
            d.push(Diagnostic::error(c.span, format!("`{key}` is declared twice")));
        }
    }
}

const EFFECT_ROOTS: &[&str] = &["credential", "payment", "storage", "message", "network", "disclosure"];

fn is_effect(name: &str) -> bool {
    name == "present"
        || name == "disclose"
        || name == "prove"
        || (name.contains('.') && EFFECT_ROOTS.contains(&name.split('.').next().unwrap_or("")))
}

fn walk_stmts(stmts: &[Stmt], f: &mut impl FnMut(&Stmt)) {
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

// -------------------------------------------------------------------- effects

fn effects_only_in_execute(p: &Program, d: &mut Vec<Diagnostic>) {
    for a in &p.actions {
        for block in &a.phases {
            if block.phase == Phase::Execute {
                continue;
            }
            walk_stmts(&block.stmts, &mut |s| {
                if let Stmt::Effect { name, span, .. } = s {
                    d.push(Diagnostic::error(
                        *span,
                        format!(
                            "`{name}` is an effect and `{}` is pure. Effects may only appear in `execute`",
                            block.phase.name()
                        ),
                    ));
                }
            });
            // A call to a known effect can also hide inside an expression.
            walk_stmts(&block.stmts, &mut |s| {
                if let Stmt::Let { value, .. } | Stmt::Expr { value, .. } = s {
                    value.walk(&mut |e| {
                        if let Expr::Call { callee, span, .. } = e {
                            if let Some(path) = callee.path() {
                                if is_effect(&path) {
                                    d.push(Diagnostic::error(
                                        *span,
                                        format!(
                                            "`{path}` is an effect and `{}` is pure. Effects may only appear in `execute`",
                                            block.phase.name()
                                        ),
                                    ));
                                }
                            }
                        }
                    });
                }
            });
        }
    }
}

fn functions_are_pure(p: &Program, d: &mut Vec<Diagnostic>) {
    for f in &p.functions {
        walk_stmts(&f.body, &mut |s| {
            let bad = match s {
                Stmt::Effect { name, span, .. } => Some((name.clone(), *span)),
                Stmt::Let { value, .. } | Stmt::Expr { value, .. } | Stmt::Return { value, .. } => {
                    let mut found = None;
                    value.walk(&mut |e| {
                        if let Expr::Call { callee, span, .. } = e {
                            if let Some(path) = callee.path() {
                                if is_effect(&path) {
                                    found = Some((path, *span));
                                }
                            }
                        }
                    });
                    found
                }
                _ => None,
            };
            if let Some((name, span)) = bad {
                d.push(Diagnostic::error(
                    span,
                    format!(
                        "functions are pure, so `{name}` cannot appear in one. There is no effectful function in this language, which is why \"what can this action do\" is one block rather than a call graph"
                    ),
                ));
            }
        });
    }
}

fn at_most_one_disclosure(p: &Program, d: &mut Vec<Diagnostic>) {
    for a in &p.actions {
        for block in &a.phases {
            let mut count = 0;
            walk_stmts(&block.stmts, &mut |s| {
                if let Stmt::Effect { name, .. } = s {
                    if name == "present" {
                        count += 1;
                    }
                }
            });
            if count > 1 {
                d.push(Diagnostic::error(
                    block.span,
                    format!(
                        "`{}` performs {count} disclosures. An action performs at most one: the effects here are one batch the host takes or refuses whole, and a disclosure cannot be taken back, so a second cannot be conditional on a batch the first has already completed",
                        a.name
                    ),
                ));
            }
        }
    }
}

// --------------------------------------------------------------- capabilities

/// A capability is its name *and* its argument. Comparing only the name lets an
/// application declare `credential.read(LoyaltyMember)`, read a passport, and
/// pass — which is not least privilege in any sense that matters, and is the
/// hole parameterised capabilities were added to close.
fn capability_key(name: &str, arg: Option<&str>) -> String {
    match arg {
        Some(a) => format!("{name}({a})"),
        None => name.to_string(),
    }
}

fn capabilities_declared_and_used(p: &Program, d: &mut Vec<Diagnostic>) {
    let mut used: HashSet<String> = HashSet::new();

    for a in &p.actions {
        for block in &a.phases {
            walk_stmts(&block.stmts, &mut |s| {
                if let Stmt::Effect { name, args, .. } = s {
                    if name == "present" || name == "disclose" || name == "prove" {
                        used.insert("disclosure.present".into());
                    } else if name == "credential.issue" {
                        // `credential.issue(LoyaltyMember { … })` — the type is
                        // the callee, and it is the argument the capability has
                        // to have named.
                        let issued = args.first().and_then(|a| match &a.value {
                            Expr::Call { callee, .. } => callee.path(),
                            other => other.path(),
                        });
                        used.insert(capability_key(name, issued.as_deref()));
                    } else {
                        used.insert(name.clone());
                    }
                }
                if let Stmt::Binding { ty, .. } = s {
                    if ty.name == "Credential" {
                        used.insert(capability_key("credential.read", ty.args.first().map(|a| a.name.as_str())));
                    }
                }
                if let Stmt::Data { source, .. } = s {
                    match source {
                        DataSource::Credentials { ty, .. } => {
                            used.insert(capability_key("credential.read", Some(ty)));
                        }
                        DataSource::Query { .. } => {
                            used.insert("api.query".into());
                        }
                        DataSource::Unknown => {}
                    }
                }
            });
        }
    }
    for s in &p.screens {
        for dd in &s.data {
            match &dd.source {
                DataSource::Credentials { ty, .. } => {
                    used.insert(capability_key("credential.read", Some(ty)));
                }
                DataSource::Query { .. } => {
                    used.insert("api.query".into());
                }
                DataSource::Unknown => {}
            }
        }
    }

    let declared: HashMap<String, &Capability> = p
        .capabilities
        .iter()
        .map(|c| {
            let arg = c.args.first().and_then(|a| a.value.path());
            (capability_key(&c.name, arg.as_deref()), c)
        })
        .collect();

    for (key, c) in &declared {
        if !used.contains(key) {
            // Named the wrong thing, or nothing? Those are different mistakes.
            let same_name: Vec<&String> = used.iter().filter(|u| u.starts_with(&format!("{}(", c.name))).collect();
            if !same_name.is_empty() {
                d.push(Diagnostic::error(
                    c.span,
                    format!(
                        "`{key}` is declared, and what this application actually does is {}. A capability is its name and its argument: declaring one type and reading another is not least privilege, it is a different permission",
                        same_name.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", ")
                    ),
                ));
                continue;
            }
            d.push(Diagnostic::error(
                c.span,
                format!(
                    "`{key}` is declared and never used. Consent asked for something unused is consent spent on nothing, and it trains people to say yes"
                ),
            ));
        }
    }
    for u in &used {
        if !declared.contains_key(u) && !declared.contains_key(u.split('(').next().unwrap_or(u)) {
            // Point at the effect that needed it.
            let mut at = None;
            for a in &p.actions {
                for block in &a.phases {
                    walk_stmts(&block.stmts, &mut |s| {
                        if let Stmt::Effect { name, span, .. } = s {
                            let want = if name == "present" || name == "disclose" || name == "prove" {
                                "disclosure.present"
                            } else {
                                name.as_str()
                            };
                            if want == u && at.is_none() {
                                at = Some(*span);
                            }
                        }
                    });
                }
            }
            let span = at.unwrap_or_default();
            d.push(Diagnostic::error(
                span,
                format!(
                    "`{u}` is used and never declared. Capabilities are in the manifest the person consented to, so adding one is a new version, not an edit"
                ),
            ));
        }
    }
}

// -------------------------------------------------------------------- switches

fn switches_are_exhaustive(p: &Program, d: &mut Vec<Diagnostic>) {
    let enums: HashMap<&str, &EnumDecl> = p.enums.iter().map(|e| (e.name.as_str(), e)).collect();
    for_each_expr(p, &mut |e| {
        let Expr::Switch { arms, span, .. } = e else { return };

        // Which enum is this over? The arms say, since a value arm is `Tier.gold`.
        let mut over: Option<&EnumDecl> = None;
        let mut seen: HashSet<String> = HashSet::new();
        for a in arms {
            if let ArmPattern::Value(Expr::Member { obj, name, .. }) = &a.pattern {
                if let Some(Expr::Ident { name: ty, .. }) = Some(obj.as_ref()) {
                    if let Some(en) = enums.get(ty.as_str()) {
                        over = Some(en);
                        seen.insert(name.clone());
                    }
                }
            }
        }
        let Some(en) = over else { return };

        if arms.iter().any(|a| matches!(a.pattern, ArmPattern::Default)) {
            d.push(Diagnostic::error(
                *span,
                format!(
                    "a `switch` over `{}` may not use `default`. Adding a member must break every program that decides something per member — that is the whole reason this is an enum and not a string",
                    en.name
                ),
            ));
            return;
        }
        let missing: Vec<&String> = en.members.iter().filter(|m| !seen.contains(*m)).collect();
        if !missing.is_empty() {
            d.push(Diagnostic::error(
                *span,
                format!(
                    "this `switch` over `{}` does not cover {}",
                    en.name,
                    missing.iter().map(|m| format!("`{}.{m}`", en.name)).collect::<Vec<_>>().join(", ")
                ),
            ));
        }
    });
}

fn no_unreachable_arms(p: &Program, d: &mut Vec<Diagnostic>) {
    for_each_expr(p, &mut |e| {
        let Expr::Switch { arms, .. } = e else { return };
        // Only the shape that actually bites: `>= small` before `>= large`.
        let mut bound: Option<i64> = None;
        for a in arms {
            if let ArmPattern::Compare { op, rhs: Expr::Num { value, .. } } = &a.pattern {
                if op == ">=" || op == ">" {
                    if let Some(b) = bound {
                        if *value >= b {
                            d.push(Diagnostic::error(
                                a.span,
                                format!(
                                    "`{op} {value}` is unreachable: an arm above it matches everything this one would. Arms are tried in order, which is fine; order-dependence nobody can see is not"
                                ),
                            ));
                        }
                    }
                    bound = Some(match bound {
                        Some(b) => b.min(*value),
                        None => *value,
                    });
                }
            }
        }
    });
}

// ---------------------------------------------------------------- determinism

fn no_floats(p: &Program, d: &mut Vec<Diagnostic>) {
    for_each_expr(p, &mut |e| {
        if let Expr::Float { text, span } = e {
            d.push(Diagnostic::error(
                *span,
                format!(
                    "no floating-point type in this language: `{text}`. NaN bit patterns are the main source of nondeterminism under Wasm, and money wants integers or fixed point regardless — use satang, not baht"
                ),
            ));
        }
    });
}

fn deterministic(p: &Program, d: &mut Vec<Diagnostic>) {
    for_each_expr(p, &mut |e| {
        let Expr::Call { callee, span, .. } = e else { return };
        let Some(path) = callee.path() else { return };
        let banned = matches!(
            path.as_str(),
            "Date.now" | "Math.random" | "random" | "now" | "fetch" | "uuid"
        );
        if banned {
            d.push(Diagnostic::error(
                *span,
                format!(
                    "no such function. Time and randomness come from the runtime context, which is recorded — `context.time.now`, `context.random.uuid`. An action that cannot be replayed cannot be proved, and proving it is the entire point"
                ),
            ));
        }
    });
}

// ------------------------------------------------------------------- totality

fn call_graph_is_acyclic(p: &Program, d: &mut Vec<Diagnostic>) {
    let names: HashSet<&str> = p.functions.iter().map(|f| f.name.as_str()).collect();
    let mut edges: HashMap<&str, HashSet<String>> = HashMap::new();
    for f in &p.functions {
        let mut calls = HashSet::new();
        walk_stmts(&f.body, &mut |s| {
            if let Stmt::Let { value, .. } | Stmt::Expr { value, .. } | Stmt::Return { value, .. } = s {
                value.walk(&mut |e| {
                    if let Expr::Call { callee, .. } = e {
                        if let Some(path) = callee.path() {
                            if names.contains(path.as_str()) {
                                calls.insert(path);
                            }
                        }
                    }
                });
            }
        });
        edges.insert(f.name.as_str(), calls);
    }

    for f in &p.functions {
        let mut stack = vec![f.name.clone()];
        let mut seen = HashSet::new();
        while let Some(cur) = stack.pop() {
            if !seen.insert(cur.clone()) {
                continue;
            }
            for next in edges.get(cur.as_str()).into_iter().flatten() {
                if *next == f.name {
                    d.push(Diagnostic::error(
                        f.span,
                        format!(
                            "`{}` is recursive, and the call graph must be acyclic. This language is total: every program halts, and the compiler knows it rather than a fuel meter finding out. Use `fold`",
                            f.name
                        ),
                    ));
                    return;
                }
                stack.push(next.clone());
            }
        }
    }
}

// -------------------------------------------------------------------- shapes

fn narrowing_before_use(p: &Program, d: &mut Vec<Diagnostic>) {
    let optional: HashSet<&str> = p.state.iter().filter(|f| f.ty.optional).map(|f| f.name.as_str()).collect();
    if optional.is_empty() {
        return;
    }
    for a in &p.actions {
        let mut narrowed: HashSet<String> = HashSet::new();
        for block in &a.phases {
            if block.phase == Phase::Require {
                for s in &block.stmts {
                    if let Stmt::Expr { value, .. } = s {
                        if let Expr::Exists { subject, .. } = value {
                            if let Some(path) = subject.path() {
                                narrowed.insert(path.trim_start_matches("state.").to_string());
                            }
                        }
                    }
                }
                continue;
            }
            let mut report = |span, field: &str| {
                d.push(Diagnostic::error(
                    span,
                    format!(
                        "`state.{field}` may not exist. Say `state.{field} exists` in `require` before reading through it — narrowing is a phase, not a check scattered through the code"
                    ),
                ));
            };
            walk_stmts(&block.stmts, &mut |s| {
                let mut check_expr = |e: &Expr| {
                    e.walk(&mut |inner| {
                        if let Expr::Member { obj, .. } = inner {
                            if let Some(path) = obj.path() {
                                if let Some(field) = path.strip_prefix("state.") {
                                    let head = field.split('.').next().unwrap_or(field);
                                    if optional.contains(head) && !narrowed.contains(head) {
                                        report(inner.span(), head);
                                    }
                                }
                            }
                        }
                    })
                };
                match s {
                    Stmt::Let { value, .. } | Stmt::Expr { value, .. } | Stmt::Return { value, .. } => check_expr(value),
                    Stmt::Patch { value, path, span } => {
                        check_expr(value);
                        let head = &path[0];
                        if path.len() > 1 && optional.contains(head.as_str()) && !narrowed.contains(head) {
                            report(*span, head);
                        }
                    }
                    Stmt::Effect { args, .. } => {
                        for a in args {
                            check_expr(&a.value)
                        }
                    }
                    Stmt::If { cond, .. } => check_expr(cond),
                    Stmt::Binding { .. } | Stmt::Data { .. } => {}
                }
            });
        }
    }
}

fn patches_have_no_index(p: &Program, d: &mut Vec<Diagnostic>) {
    for a in &p.actions {
        for block in &a.phases {
            if block.phase != Phase::Update {
                continue;
            }
            for s in &block.stmts {
                if let Stmt::Patch { path, span, .. } = s {
                    if path.iter().any(|seg| seg.contains('[')) {
                        d.push(Diagnostic::error(
                            *span,
                            "a patch path may not contain a list index. That is where this would need an optics story, and it does not have one: build the new list in `compute` and name it here",
                        ));
                    }
                }
            }
        }
    }
}

fn updates_take_paths(p: &Program, d: &mut Vec<Diagnostic>) {
    for a in &p.actions {
        for block in &a.phases {
            if block.phase != Phase::Update {
                continue;
            }
            for s in &block.stmts {
                match s {
                    Stmt::Patch { value, span, .. } => {
                        if let Expr::Record { .. } = value {
                            d.push(Diagnostic::error(
                                *span,
                                "`update` takes paths, not record literals. `member.tier: tier` says this already, and two ways to write one thing is the cost this language spends its budget avoiding",
                            ));
                        }
                    }
                    Stmt::Expr { value, span } => {
                        d.push(Diagnostic::error(
                            *span,
                            match value {
                                Expr::Binary { op, .. } if op == "=" => {
                                    "there is no assignment in this language. `update` is a patch: write `member.points: 10`, a colon, because the line describes the next state rather than changing this one"
                                        .to_string()
                                }
                                _ => "every line in `update` names a field and the value it takes".to_string(),
                            },
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
}

// ------------------------------------------------------------------- walking

fn for_each_expr(p: &Program, f: &mut impl FnMut(&Expr)) {
    let visit_stmts = |stmts: &[Stmt], f: &mut dyn FnMut(&Expr)| {
        walk_stmts(stmts, &mut |s| match s {
            Stmt::Let { value, .. } | Stmt::Expr { value, .. } | Stmt::Return { value, .. } | Stmt::Patch { value, .. } => {
                value.walk(f)
            }
            Stmt::Effect { args, .. } => {
                for a in args {
                    a.value.walk(f)
                }
            }
            Stmt::If { cond, .. } => cond.walk(f),
            Stmt::Binding { .. } | Stmt::Data { .. } => {}
        })
    };
    for a in &p.actions {
        for b in &a.phases {
            visit_stmts(&b.stmts, f);
        }
    }
    for fun in &p.functions {
        visit_stmts(&fun.body, f);
    }
    for s in &p.screens {
        visit_stmts(&s.compute, f);
    }
    for t in &p.trusts {
        for r in &t.requires {
            r.walk(f);
        }
    }
    for c in &p.credentials {
        for fld in &c.fields {
            if let Some(dv) = &fld.default {
                dv.walk(f)
            }
        }
    }
    for fld in &p.state {
        if let Some(dv) = &fld.default {
            dv.walk(f)
        }
    }
}
