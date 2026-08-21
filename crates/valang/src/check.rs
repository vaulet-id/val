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
    effects_do_not_read_each_other(p, &mut d);
    defaults_are_written_out(p, &mut d);
    lists_walked_have_a_bound(p, &mut d);
    refusals_come_before_effects(p, &mut d);
    policies_name_an_anchor(p, &mut d);
    navigation_goes_somewhere(p, &mut d);
    every_action_is_reachable(p, &mut d);
    admits_can_be_answered(p, &mut d);
    credentials_say_what_they_are(p, &mut d);
    d
}

/// A credential names the type a wallet knows it by.
///
/// `EmployeeBadge` is a name this package chose and no wallet has ever heard.
/// Without the `vct`, a package could declare a credential and nothing in the
/// world could say which of somebody's cards it meant — so an application that
/// reads, checks or issues one would be an application no host can answer.
fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push('-');
        }
        out.extend(ch.to_lowercase());
    }
    out
}

fn credentials_say_what_they_are(p: &Program, d: &mut Vec<Diagnostic>) {
    for c in &p.credentials {
        if c.vct.is_empty() {
            d.push(Diagnostic::error(
                c.span,
                format!("`{}` does not say what it is. A wallet knows credentials by their `vct`, never by the name a package chose for one: `credential {} as \"https://org.vaulet.id/your-org/credential/{}\"`", c.name, c.name, kebab(&c.name)),
            ));
            continue;
        }
        // `https` and absolute, for the same reason every other address in this
        // language is: a type somebody can answer over the wire is a type
        // somebody on the wire can answer.
        if !c.vct.starts_with("https://") {
            d.push(Diagnostic::error(
                c.span,
                format!("`{}` is a credential type, and one is an absolute `https` URL — `{}` is not", c.name, c.vct),
            ));
        }
    }
    // Two names for one card is two halves of an application disagreeing about
    // what the person is holding.
    let mut seen: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for c in &p.credentials {
        if c.vct.is_empty() {
            continue;
        }
        if let Some(first) = seen.insert(&c.vct, &c.name) {
            d.push(Diagnostic::error(
                c.span,
                format!("`{}` and `{}` are the same credential type. One card cannot be two things to one application", first, c.name),
            ));
        }
    }
}

/// A gate names a credential this package declared and a policy over that same
/// credential.
///
/// Both halves have been wrong in the same file: a policy over the wrong
/// subject type checks a signature on something else and passes, which is a
/// door that opens for the wrong badge rather than one that fails to open.
fn admits_can_be_answered(p: &Program, d: &mut Vec<Diagnostic>) {
    for a in &p.admits {
        if !p.credentials.iter().any(|c| c.name == a.credential) {
            d.push(Diagnostic::error(
                a.span,
                format!("`{}` is not a credential this package declares. A door names what it opens for in this package's own words", a.credential),
            ));
            continue;
        }
        let Some(policy) = p.trusts.iter().find(|t| t.name == a.policy) else {
            d.push(Diagnostic::error(
                a.span,
                format!("`{}` is not a `trust` policy in this package", a.policy),
            ));
            continue;
        };
        if policy.subject_type != a.credential {
            d.push(Diagnostic::error(
                a.span,
                format!(
                    "`{}` is a policy over `{}`, and this gate is about `{}`. Checked against it, a `{}` would be admitted by the signature on something else",
                    a.policy, policy.subject_type, a.credential, a.credential
                ),
            ));
        }
    }
}

/// An action is reached by a press. Declaring one binds nothing — if no screen
/// names it, it sits there, and the only trigger this language defines has
/// nothing to do with it.
///
/// A warning rather than an error. A file may be a library — actions declared in
/// one file and pressed by a screen in another — which this already allows,
/// because a package is one scope and this looks at all of it. And a host may
/// one day define a second way in.
///
/// What it must not be is silence: **the capabilities that action needs are
/// still on the consent sheet**, and a person agreed to something that cannot
/// happen.
///
/// A package with no screens at all is skipped entirely: that is a fragment
/// waiting for the rest of its package, not an unreachable action.
fn every_action_is_reachable(p: &Program, d: &mut Vec<Diagnostic>) {
    if p.screens.is_empty() {
        return;
    }

    // Which props hold an action is the registry's answer, and without one this
    // check has no way to tell a press from any other argument. Asked anyway,
    // it would call every action unreachable — so it does not ask.
    if p.handlers.is_empty() {
        return;
    }

    let mut pressed: HashSet<String> = HashSet::new();
    for s in &p.screens {
        for n in s.title.iter().chain(s.tree.iter()) {
            walk_ui(n, &mut |node| {
                for a in &node.args {
                    // Every prop that holds an action, which the registry
                    // named and this pass was told.
                    if a.name.as_deref().is_some_and(|n| p.handlers.iter().any(|h| h == n)) {
                        if let Some(target) = a.value.path() {
                            pressed.insert(target);
                        }
                    }
                }
            });
        }
    }
    for a in &p.actions {
        for block in &a.phases {
            walk_stmts(&block.stmts, &mut |s| {
                if let Stmt::Effect { name, args, .. } = s {
                    if name == "navigate" {
                        if let Some(t) = args.first().and_then(|x| x.value.path()) {
                            pressed.insert(t);
                        }
                    }
                }
            });
        }
    }

    for a in &p.actions {
        if !pressed.contains(&a.name) {
            d.push(Diagnostic::warning(
                a.span,
                format!(
                    "no screen names `{}`, so nothing can reach it — and the capabilities it needs are still on the consent sheet a person agreed to",
                    a.name
                ),
            ));
        }
    }
}

/// An application declines before it asks, never after. By `execute` the batch
/// has been built and the host is about to be offered it — a refusal there is a
/// decision taken too late to be one.
fn refusals_come_before_effects(p: &Program, d: &mut Vec<Diagnostic>) {
    for a in &p.actions {
        for block in &a.phases {
            if block.phase != Phase::Execute {
                continue;
            }
            walk_stmts(&block.stmts, &mut |s| {
                if let Stmt::Refuse { span, .. } = s {
                    d.push(Diagnostic::error(
                        *span,
                        "`refuse` belongs before `execute`. By here the batch is built and the host is about to be offered it, so declining is a decision taken too late to be one — put the rule in `require`, in `verify`, or in `compute`",
                    ));
                }
            });
        }
    }
}

/// A trust policy without an anchor trusts nobody in particular, which reads
/// like a policy and is not one. Resolving the anchor is the host's — the
/// language cannot fetch a chain — but *having* one is checkable here, and a
/// policy that forgot is a policy that will be discovered by a credential it
/// should have refused.
fn policies_name_an_anchor(p: &Program, d: &mut Vec<Diagnostic>) {
    for t in &p.trusts {
        match &t.anchor {
            None => d.push(Diagnostic::error(
                t.span,
                format!(
                    "`{}` names no anchor, so it says which predicates must hold and nothing about who may satisfy them. A signature that is valid is a signature by somebody",
                    t.name
                ),
            )),
            Some(a) if a.trim().is_empty() => d.push(Diagnostic::error(
                t.span,
                format!("`{}` has an empty anchor", t.name),
            )),
            Some(_) => {}
        }
        if t.requires.is_empty() {
            d.push(Diagnostic::warning(
                t.span,
                format!(
                    "`{}` requires nothing, so verifying against it establishes only that the credential exists",
                    t.name
                ),
            ));
        }
    }
}

/// `navigate` sends somebody somewhere. Somewhere has to be a screen this
/// package declares — a name that resolves at run time or not at all is how an
/// application takes a person to a blank frame.
fn navigation_goes_somewhere(p: &Program, d: &mut Vec<Diagnostic>) {
    let screens: HashSet<&str> = p.screens.iter().map(|s| s.name.as_str()).collect();
    let mut check = |args: &[Arg], span| {
        let Some(target) = args.first().and_then(|a| a.value.path()) else {
            d.push(Diagnostic::error(span, "`navigate` names a screen"));
            return;
        };
        if !screens.contains(target.as_str()) {
            d.push(Diagnostic::error(
                span,
                format!("`{target}` is not a screen this package declares"),
            ));
        }
    };
    for a in &p.actions {
        for block in &a.phases {
            walk_stmts(&block.stmts, &mut |s| {
                if let Stmt::Effect { name, args, span, .. } = s {
                    if name == "navigate" {
                        check(args, *span);
                    }
                }
            });
        }
    }
    for s in &p.screens {
        for n in s.title.iter().chain(s.tree.iter()) {
            walk_ui(n, &mut |node| {
                for a in &node.args {
                    if a.name.as_deref().is_some_and(|n| p.handlers.iter().any(|h| h == n)) {
                        if let Some(target) = a.value.path() {
                            // A handler names an action or a screen; either has to
                            // exist, and which one it is decides what happens.
                            // A dotted target is a capability's operation —
                            // `navigation.back(with: …)`. Which operations exist
                            // is the host's document, so it is checked where the
                            // interfaces are in reach rather than guessed here.
                            if !target.contains('.')
                                && !screens.contains(target.as_str())
                                && !p.actions.iter().any(|x| x.name == target)
                            {
                                d.push(Diagnostic::error(
                                    a.span,
                                    format!(
                                        "`{target}` is neither an action nor a screen this package declares. A press names one of them and nothing else"
                                    ),
                                ));
                            }
                        }
                    }
                }
            });
        }
    }
}

fn walk_ui(n: &UiNode, f: &mut impl FnMut(&UiNode)) {
    n.walk(f);
}

/// The effects in `execute` are one batch, offered together. There is no moment
/// between them for one to read what another produced — and an application
/// written as if there were is one whose author believes the state commits
/// halfway, which is the belief §5 exists to remove.
fn effects_do_not_read_each_other(p: &Program, d: &mut Vec<Diagnostic>) {
    for a in &p.actions {
        for block in &a.phases {
            if block.phase != Phase::Execute {
                continue;
            }
            let mut bound: HashSet<String> = HashSet::new();
            // Branches included: an effect written inside an `if` is still an
            // effect in this batch, and a line reading its result is still
            // reading something that does not exist yet.
            let mut stmts: Vec<&Stmt> = Vec::new();
            flatten(&block.stmts, &mut stmts);
            for s in stmts {
                match s {
                    Stmt::Let { name, value, span, .. } => {
                        let mut reads_effect = None;
                        value.walk(&mut |e| {
                            if let Expr::Call { callee, .. } = e {
                                if let Some(path) = callee.path() {
                                    if is_effect(&path) {
                                        reads_effect = Some(path);
                                    }
                                }
                            }
                        });
                        if let Some(path) = reads_effect {
                            d.push(Diagnostic::error(
                                *span,
                                format!(
                                    "`{path}` is requested, not performed, so there is nothing here to bind. The effects in `execute` are one batch the host takes or refuses whole — if one genuinely depends on another's outcome, that is two actions, and the person gets to see both"
                                ),
                            ));
                        }
                        bound.insert(name.clone());
                    }
                    Stmt::Effect { args, span, .. } => {
                        for arg in args {
                            arg.value.walk(&mut |e| {
                                if let Expr::Ident { name, .. } = e {
                                    if bound.contains(name) {
                                        d.push(Diagnostic::error(
                                            *span,
                                            format!(
                                                "`{name}` was bound from an earlier effect in this block, and an effect has no result until the host has taken the whole batch"
                                            ),
                                        ));
                                    }
                                }
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }
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

/// Every key the program names, against the bundle it ships with.
///
/// A missing key is a screen that says `missing key "balance"` to somebody, and
/// a missing locale is a market where the application is unusable — both are
/// failed builds rather than bug reports, because both are knowable now.
pub fn check_bundle(
    p: &Program,
    bundle: &crate::TextBundle,
    locales: &[String],
    hosts: &crate::capability::Hosts,
) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let mut want: Vec<(String, crate::diag::Span, &'static str)> = Vec::new();

    for a in &p.actions {
        for block in &a.phases {
            walk_stmts(&block.stmts, &mut |s| {
                if let Stmt::Refuse { key, span } = s {
                    want.push((key.clone(), *span, "refusal"));
                }
            });
        }
    }
    // What a person is told at a door that did not open. The one sentence in
    // the package somebody may read before they have read anything else, so it
    // is held to the same rule as every other: a key, in every locale.
    for a in &p.admits {
        want.push((a.phrase.clone(), a.span, "refusal"));
    }
    // Key to the slots the node fills it with, so the two can be compared.
    let mut filled: Vec<(String, crate::diag::Span, Vec<String>)> = Vec::new();
    for s in &p.screens {
        for n in s.title.iter().chain(s.tree.iter()) {
            walk_ui(n, &mut |node| {
                let common = hosts.common();
                for a in &node.args {
                    // Every prop the registry says holds a sentence, not the
                    // one called `text`. A key written in `detail:` was checked
                    // in no language at all, because only one name was ever
                    // looked at.
                    let declared = a.name.as_ref().and_then(|name| {
                        hosts
                            .find(&node.kind)
                            .and_then(|(_, cap)| cap.props.get(name).cloned())
                            .or_else(|| common.get(name).cloned())
                    });
                    let is_text = match &declared {
                        Some(ty) => ty.trim_end_matches('?') == "Text",
                        // Without a registry to ask — `analyse` on its own —
                        // the name is all there is to go on.
                        None => {
                            a.name.as_deref() == Some("text")
                                || (a.name.is_none() && node.kind == "tab")
                        }
                    };
                    if !is_text {
                        continue;
                    }
                    if let Expr::Str { value, span } = &a.value {
                        want.push((value.clone(), *span, "screen"));
                        filled.push((value.clone(), *span, node.slots.clone()));
                    }
                }
            });
        }
    }

    // A sentence names its values in braces, and the code supplies them by
    // name. Neither half is allowed to be a guess: a placeholder nothing fills
    // renders as itself, and a slot the sentence does not name is a value
    // nobody reads — usually a rename that stopped halfway.
    for (key, span, slots) in &filled {
        // Words written in place carry their own slots, checked against
        // themselves. A key carries the bundle's, checked in every language.
        let templates: Vec<(String, String)> = match bundle.get(key) {
            Some(entry) => entry.iter().map(|(l, t)| (l.clone(), t.clone())).collect(),
            None => vec![("this line".to_string(), key.clone())],
        };
        for (locale, template) in &templates {
            let named = placeholders(template);
            for want in &named {
                if !slots.contains(want) {
                    d.push(Diagnostic::error(
                        *span,
                        format!("`{key}` says `{{{want}}}` in {locale}, and nothing fills it"),
                    ));
                }
            }
            for have in slots {
                if !named.contains(have) {
                    d.push(Diagnostic::error(
                        *span,
                        format!("`{key}` has no `{{{have}}}` in {locale}"),
                    ));
                }
            }
        }
    }

    // One language needs no bundle at all: the words on the screen are the
    // words, and 80% of applications will never have a second one. A key only
    // becomes something to know about when a package promises two languages,
    // and that is where the error appears.
    let translated = locales.len() > 1;

    // Everything the code reads, so that what the bundle holds and nothing
    // reads can be named. A package is signed whole: a key nobody reads is a
    // sentence somebody will translate, pay for, and never see.
    let read: std::collections::HashSet<String> = want.iter().map(|(k, _, _)| k.clone()).collect();

    for (key, span, what) in want {
        let Some(entry) = bundle.get(&key) else {
            if translated {
                d.push(Diagnostic::error(
                    span,
                    format!(
                        "`{key}` is written here as words, and this package promises {}. Move it to text.json as a key, or ship one language",
                        locales.join(" and ")
                    ),
                ));
            }
            let _ = what;
            continue;
        };
        for locale in locales {
            if !entry.contains_key(locale) {
                d.push(Diagnostic::error(
                    span,
                    format!(
                        "`{key}` has no {locale}. A market's language missing is a failed build, not a bug report"
                    ),
                ));
            }
        }
    }
    // Said rather than refused. An unused capability is consent somebody gave
    // for nothing, which is why that one fails a build; an unread key is waste,
    // and a package may carry sentences its webview tier reads or its next
    // screen will. Worth knowing, not worth stopping for.
    //
    // Only where a package promises more than one language: in one, the words
    // on the screen are the words and the bundle is optional.
    if translated {
        for key in bundle.keys() {
            if !read.contains(key) {
                d.push(Diagnostic::warning(
                    crate::diag::Span { line: 0, col: 0, len: 0 },
                    format!("`{key}` is in the text bundle and nothing reads it. Somebody translated a sentence nobody sees"),
                ));
            }
        }
    }

    d
}

/// The names a template asks for, in the order they appear. `{amount}` is one;
/// `{{` is not — a sentence that wants a brace escapes it the way every other
/// template language does.
fn placeholders(template: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes: Vec<char> = template.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != '{' {
            i += 1;
            continue;
        }
        if bytes.get(i + 1) == Some(&'{') {
            i += 2;
            continue;
        }
        let start = i + 1;
        let Some(end) = (start..bytes.len()).find(|&j| bytes[j] == '}') else { break };
        let name: String = bytes[start..end].iter().collect();
        if !name.is_empty() {
            out.push(name.trim().to_string());
        }
        i = end + 1;
    }
    out
}

/// The shape of an effect, for the checks that run before a host's interfaces
/// are in reach: `capability.operation`. Which capabilities exist is the host's
/// answer — see `interface.rs` — and this is only what a call looks like.
///
/// `present`, `disclose` and `prove` are syntax rather than calls, which is why
/// they are named here and the rest are not.
fn is_effect(name: &str) -> bool {
    if matches!(name, "present" | "disclose" | "prove") {
        return true;
    }
    // `receipts.fold(…)` walks a list; `credential.issue(…)` calls a
    // capability. The combinators are a closed set, and everything else dotted
    // is a call to something the host offers.
    match name.split_once('.') {
        Some((_, method)) => {
            !matches!(method, "map" | "filter" | "fold" | "any" | "all" | "count" | "first")
        }
        None => false,
    }
}

/// What a statement contains, and what a node contains, are questions with one
/// answer each — `Stmt::walk` and `UiNode::walk`, beside the types. These are
/// the shapes this file needs them in.
fn flatten<'a>(stmts: &'a [Stmt], out: &mut Vec<&'a Stmt>) {
    Stmt::flatten(stmts, out)
}

fn walk_stmts(stmts: &[Stmt], f: &mut impl FnMut(&Stmt)) {
    for s in stmts {
        s.walk(f);
    }
}

// -------------------------------------------------------------------- effects

/// A list somebody computes over says how long it may be.
///
/// `limit` bounds the work, which is what lets a total over a list compile to a
/// circuit — and what makes the cost of a proof the bound rather than the data,
/// so that it does not leak how much the person holds. Without one, a `fold`
/// over what the wallet answered with is work nobody wrote a number for.
fn lists_walked_have_a_bound(p: &Program, d: &mut Vec<Diagnostic>) {
    // Every name bound to credentials, and whether it said how many.
    let mut unbounded: HashMap<String, crate::diag::Span> = HashMap::new();
    let mut note = |name: &str, source: &DataSource, span: crate::diag::Span| {
        if let DataSource::Credentials { limit: None, .. } = source {
            unbounded.insert(name.to_string(), span);
        }
    };
    for s in &p.screens {
        for decl in &s.data {
            note(&decl.name, &decl.source, decl.span);
        }
    }
    for a in &p.actions {
        for block in &a.phases {
            walk_stmts(&block.stmts, &mut |st| {
                if let Stmt::Data { name, source, span } = st {
                    note(name, source, *span);
                }
            });
        }
    }
    if unbounded.is_empty() {
        return;
    }

    let mut said: HashSet<String> = HashSet::new();
    let mut walked = |e: &Expr, d: &mut Vec<Diagnostic>| {
        e.walk(&mut |inner| {
            let Expr::Call { callee, .. } = inner else { return };
            let Expr::Member { obj, name: method, .. } = callee.as_ref() else { return };
            // `count` and `first` read the list without walking it.
            if !matches!(method.as_str(), "map" | "filter" | "fold" | "any" | "all") {
                return;
            }
            let Some(over) = obj.path() else { return };
            let Some(at) = unbounded.get(&over) else { return };
            if !said.insert(over.clone()) {
                return;
            }
            d.push(Diagnostic::error(
                *at,
                format!("`{over}` is computed over and says no `limit`. A bound is what makes the work knowable before it runs — and what makes a proof over it cost the bound rather than the data"),
            ));
        });
    };

    for s in &p.screens {
        for st in &s.compute {
            if let Stmt::Let { value, .. } = st {
                walked(value, d);
            }
        }
        for n in &s.tree {
            n.walk(&mut |node| {
                for a in &node.args {
                    walked(&a.value, d);
                }
            });
        }
    }
    for a in &p.actions {
        for block in &a.phases {
            let mut flat: Vec<&Stmt> = Vec::new();
            flatten(&block.stmts, &mut flat);
            for st in flat {
                match st {
                    Stmt::Let { value, .. }
                    | Stmt::Expr { value, .. }
                    | Stmt::Patch { value, .. }
                    | Stmt::Return { value, .. }
                    | Stmt::Assign { value, .. }
                    | Stmt::Destructure { value, .. } => walked(value, d),
                    Stmt::If { cond, .. } => walked(cond, d),
                    Stmt::Effect { args, .. } => {
                        for a in args {
                            walked(&a.value, d);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// A state field starts where its `default` says, and nowhere else.
///
/// It is read before anything has run: there is no scope, no previous version
/// to consult and no action to have produced a value. Anything but a value
/// written out is a name that resolves to nothing, quietly — which is a state
/// field starting as null on a phone rather than as the number it says.
fn defaults_are_written_out(p: &Program, d: &mut Vec<Diagnostic>) {
    fn written_out(e: &Expr, p: &Program) -> bool {
        match e {
            Expr::Num { .. } | Expr::Str { .. } | Expr::Bool { .. } => true,
            Expr::Unary { rhs, .. } => written_out(rhs, p),
            Expr::List { items, .. } => items.iter().all(|x| written_out(x, p)),
            Expr::Record { spread, fields, .. } => {
                spread.is_none() && fields.iter().all(|(_, v)| written_out(v, p))
            }
            // `Tier.bronze` — an enum member, which is a value written out
            // under another name.
            Expr::Member { obj, .. } => match obj.as_ref() {
                Expr::Ident { name, .. } => p.enums.iter().any(|e| e.name == *name),
                _ => false,
            },
            _ => false,
        }
    }

    for f in &p.state {
        let Some(value) = &f.default else { continue };
        if !written_out(value, p) {
            d.push(Diagnostic::error(
                f.span,
                format!("`{}` starts at a value written out here. A default is read before anything has run — there is no scope to read a name from, and no previous version to ask", f.name),
            ));
        }
    }
}

fn effects_only_in_execute(p: &Program, d: &mut Vec<Diagnostic>) {
    // A screen derives and does not act, so its `compute` is checked by the
    // same rule as an action's — it was checked by nothing, and a screen could
    // issue a credential while it was being drawn.
    for s in &p.screens {
        effects_in_pure(&s.compute, "a screen's `compute`", d);
    }
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

/// Effects in a block that has none, named by what the block is.
fn effects_in_pure(stmts: &[Stmt], what: &str, d: &mut Vec<Diagnostic>) {
    walk_stmts(stmts, &mut |s| {
        if let Stmt::Effect { name, span, .. } = s {
            d.push(Diagnostic::error(
                *span,
                format!("`{name}` is an effect and {what} is pure. Effects may only appear in `execute`"),
            ));
        }
        // And one hiding inside an expression, which is how it got past the
        // rule that only looked at statements.
        if let Stmt::Let { value, .. } | Stmt::Expr { value, .. } = s {
            value.walk(&mut |e| {
                if let Expr::Call { callee, span, .. } = e {
                    if let Some(path) = callee.path() {
                        if is_effect(&path) {
                            d.push(Diagnostic::error(
                                *span,
                                format!("`{path}` is an effect and {what} is pure. Effects may only appear in `execute`"),
                            ));
                        }
                    }
                }
            });
        }
    });
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

    // A gate looks at a credential and does not read it, so it is a `check` and
    // not a `read` — the difference the sheet turns into two different
    // sentences about the same application.
    for a in &p.admits {
        used.insert(capability_key("credential.check", Some(&a.credential)));
    }

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

    // Drawing something privileged is a use of the capability it needs, so a
    // package that declares `media.video` because a screen shows a video is not
    // declaring something it never uses. Which component needs what is the
    // host's document, read by the pass that has it.
    used.extend(p.uses.iter().cloned());

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
                        // `state.member?.name` is the author saying it may
                        // not be there, which is what the narrowing rule is
                        // asking them to say. Refusing it too would leave them
                        // no way to say it at all.
                        if let Expr::Member { obj, optional: written, .. } = inner {
                            if *written {
                                return;
                            }
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
                    Stmt::Let { value, .. }
                    | Stmt::Assign { value, .. }
                    | Stmt::Destructure { value, .. }
                    | Stmt::Expr { value, .. }
                    | Stmt::Return { value, .. } => check_expr(value),
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
                    Stmt::Binding { .. } | Stmt::Data { .. } | Stmt::Refuse { .. } => {}
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
            let mut stmts: Vec<&Stmt> = Vec::new();
            flatten(&block.stmts, &mut stmts);
            for s in stmts {
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
            let mut stmts: Vec<&Stmt> = Vec::new();
            flatten(&block.stmts, &mut stmts);
            for s in stmts {
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
            Stmt::Let { value, .. }
            | Stmt::Assign { value, .. }
            | Stmt::Destructure { value, .. }
            | Stmt::Expr { value, .. }
            | Stmt::Return { value, .. }
            | Stmt::Patch { value, .. } => {
                value.walk(f)
            }
            Stmt::Effect { args, .. } => {
                for a in args {
                    a.value.walk(f)
                }
            }
            Stmt::If { cond, .. } => cond.walk(f),
            Stmt::Binding { .. } | Stmt::Data { .. } | Stmt::Refuse { .. } => {}
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
