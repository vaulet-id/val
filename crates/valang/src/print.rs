//! The program, written back out.
//!
//! One shape per thing, so that a file has a form somebody's editor can produce
//! and a reviewer can diff. That is the visible half; the useful half is that a
//! printer makes the parser testable against itself — `print(parse(print(parse
//! (x))))` and `print(parse(x))` have to be the same text, and a production the
//! parser reads but the printer cannot write is a production one of them does
//! not really have.
//!
//! Two rules it keeps that a formatter is judged on: **declarations stay in the
//! order they were written**, because reordering a file is rewriting it, and
//! **comments stay where they were**, because a comment is the reasoning that
//! was expensive to recover.
//!
//! Every match here is exhaustive on purpose. A node added to the language
//! stops this compiling, which is the only way a printer stays complete.

use crate::ast::*;
use crate::diag::Span;

const INDENT: &str = "  ";

pub fn print(p: &Program) -> String {
    Printer {
        comments: p.comments.as_slice(),
        next: 0,
        out: String::new(),
        last: 0,
        blanks: &p.blank_lines,
    }
    .program(p)
}

struct Printer<'a> {
    comments: &'a [crate::lex::Comment],
    next: usize,
    out: String,
    /// The source line of the last thing printed, so that a blank line the
    /// author left between two groups is left there. A formatter that closed
    /// every gap would be rewriting the shape of the file rather than its
    /// punctuation.
    last: u32,
    /// The lines that held nothing but space.
    blanks: &'a std::collections::BTreeSet<u32>,
}

impl Printer<'_> {
    // ------------------------------------------------------------- comments

    /// Everything written above this line and not put back yet, at this indent.
    ///
    /// A comment belongs to a position; which node it is about is a guess, and
    /// the guess made here is the one a reader makes — it belongs to whatever
    /// comes next.
    fn before(&mut self, line: u32, depth: usize) {
        let pad = INDENT.repeat(depth);
        // A blank line above stays a blank line above.
        //
        // Asked as "was the line above this one empty", not "how far apart were
        // they": the printer changes how many lines a thing takes, so a
        // distance measured in the source is a different distance in the
        // output, and printing twice gave two texts. This question survives
        // printing, because the printer writes exactly one blank line where the
        // answer was yes.
        let first = self
            .comments
            .get(self.next)
            .filter(|c| c.span.line < line)
            .map(|c| c.span.line)
            .unwrap_or(line);
        if self.last > 0 && self.blank_above(first) && !self.out.ends_with("{\n") {
            self.out.push('\n');
        }
        if first > 0 {
            self.last = first;
        }
        while self.next < self.comments.len() && self.comments[self.next].span.line < line {
            let c = &self.comments[self.next];
            // A blank comment line stays blank rather than becoming `//` with
            // trailing space.
            self.out.push_str(&format!("{pad}{}\n", c.text));
            self.last = c.span.line;
            self.next += 1;
        }
        // And a blank line between the comments and what follows them, where
        // the author left one: a note about the whole file sits apart from the
        // first declaration, and a note about that declaration sits against it.
        if self.last > 0 && line > self.last && self.blank_above(line) && !self.out.ends_with("{\n")
        {
            self.out.push('\n');
        }
        if line > 0 {
            self.last = line;
        }
    }

    /// A comment written after code on the same line, appended to what was just
    /// printed rather than pushed onto a line of its own.
    fn trailing(&mut self, line: u32) {
        while self.next < self.comments.len()
            && self.comments[self.next].span.line == line
            && self.comments[self.next].trailing
        {
            let text = self.comments[self.next].text.clone();
            if self.out.ends_with('\n') {
                self.out.pop();
            }
            self.out.push_str(&format!("  {text}\n"));
            self.next += 1;
        }
    }

    /// Whether the line above this one held nothing but space.
    fn blank_above(&self, line: u32) -> bool {
        line > 1 && self.blanks.contains(&(line - 1))
    }

    fn rest(&mut self) {
        while self.next < self.comments.len() {
            let text = self.comments[self.next].text.clone();
            self.out.push_str(&format!("\n{text}\n"));
            self.next += 1;
        }
    }

    // ---------------------------------------------------------- the program

    fn program(mut self, p: &Program) -> String {
        let mut items: Vec<(Span, Item)> = Vec::new();

        if p.app.is_some() {
            items.push((p.app_span, Item::App));
        }
        if p.version.is_some() {
            items.push((p.version_span, Item::Version));
        }
        for (i, _) in p.hosts.iter().enumerate() {
            items.push((Span::default(), Item::Host(i)));
        }
        for (i, d) in p.imports.iter().enumerate() {
            items.push((d.span, Item::Import(i)));
        }
        // Only where one was written. A package of several files declares them
        // once, and printing an empty block into the other file would be the
        // formatter adding a declaration.
        if p.capabilities_span != Span::default() {
            items.push((p.capabilities_span, Item::Capabilities));
        }
        if !p.admits.is_empty() {
            items.push((p.admits_span, Item::Admits));
        }
        for (i, d) in p.enums.iter().enumerate() {
            items.push((d.span, Item::Enum(i)));
        }
        for (i, d) in p.credentials.iter().enumerate() {
            items.push((d.span, Item::Credential(i)));
        }
        for (i, d) in p.types.iter().enumerate() {
            items.push((d.span, Item::Type(i)));
        }
        if !p.state.is_empty() {
            items.push((p.state_span, Item::State));
        }
        for (i, d) in p.trusts.iter().enumerate() {
            items.push((d.span, Item::Trust(i)));
        }
        for (i, d) in p.functions.iter().enumerate() {
            items.push((d.span, Item::Function(i)));
        }
        for (i, d) in p.actions.iter().enumerate() {
            items.push((d.span, Item::Action(i)));
        }
        for (i, d) in p.components.iter().enumerate() {
            items.push((d.span, Item::Component(i)));
        }
        for (i, d) in p.screens.iter().enumerate() {
            // A directive is written above the screen, so the screen starts
            // where its first directive does — otherwise a blank line above
            // `@main` is asked about the line `@main` itself is on, which is
            // never blank the second time round.
            let at = d.directives.first().map(|x| x.span).unwrap_or(d.span);
            items.push((at, Item::Screen(i)));
        }
        items.sort_by_key(|(span, _)| (span.line, span.col));

        for (span, item) in &items {
            // Spacing is `before`'s: a gap in the source is a gap here, and a
            // comment above a declaration belongs to the declaration.
            self.before(span.line, 0);
            self.item(p, *item, span.line);
        }
        self.rest();
        self.out
    }


    fn item(&mut self, p: &Program, item: Item, line: u32) {
        match item {
            Item::App => {
                let app = p.app.clone().unwrap_or_default();
                self.out.push_str(&format!("app {}\n", quoted(&app)));
            }
            Item::Version => {
                let v = p.version.clone().unwrap_or_default();
                self.out.push_str(&format!("version {v}\n"));
            }
            Item::Host(i) => self.out.push_str(&format!("host {}\n", quoted(&p.hosts[i]))),
            Item::Import(i) => {
                let d = &p.imports[i];
                self.out.push_str(&format!(
                    "import {} {{ {} }}\n",
                    quoted(&d.package),
                    d.names.join(", ")
                ));
            }
            Item::Capabilities => {
                self.out.push_str("capabilities {\n");
                for c in &p.capabilities {
                    self.before(c.span.line, 1);
                    self.out.push_str(&format!("{INDENT}{}", c.name));
                    if !c.args.is_empty() {
                        self.out.push_str(&format!("({})", args(&c.args)));
                    }
                    self.out.push('\n');
                    self.trailing(c.span.line);
                }
                self.out.push_str("}\n");
            }
            Item::Admits => {
                self.out.push_str("admits {\n");
                for a in &p.admits {
                    self.before(a.span.line, 1);
                    self.out.push_str(&format!(
                        "{INDENT}{} with {} else {}\n",
                        a.credential,
                        a.policy,
                        quoted(&a.phrase)
                    ));
                    self.trailing(a.span.line);
                }
                self.out.push_str("}\n");
            }
            Item::Enum(i) => {
                let d = &p.enums[i];
                self.out.push_str(&format!("enum {} {{ {} }}\n", d.name, d.members.join(", ")));
            }
            Item::Credential(i) => {
                let d = &p.credentials[i];
                // The `vct` is part of the declaration, and a printer that
                // dropped it would print a program that no longer compiles.
                if d.vct.is_empty() {
                    self.out.push_str(&format!("credential {} ", d.name));
                } else {
                    self.out.push_str(&format!("credential {} as {:?} ", d.name, d.vct));
                }
                self.fields(&d.fields, 0);
            }
            Item::Type(i) => {
                let d = &p.types[i];
                self.out.push_str(&format!("type {} ", d.name));
                self.fields(&d.fields, 0);
            }
            Item::State => {
                self.out.push_str("state ");
                self.fields(&p.state, 0);
            }
            Item::Trust(i) => self.trust(&p.trusts[i]),
            Item::Function(i) => self.function(&p.functions[i]),
            Item::Action(i) => self.action(&p.actions[i]),
            Item::Component(i) => self.component(&p.components[i]),
            Item::Screen(i) => self.screen(&p.screens[i]),
        }
        self.trailing(line);
    }

    fn fields(&mut self, f: &[Field], depth: usize) {
        let pad = INDENT.repeat(depth);
        // Names padded to the longest, so the types line up. A field list is a
        // table and reads like one; this is the one place the printer spends a
        // space on how it looks.
        let width = f.iter().map(|x| x.name.chars().count()).max().unwrap_or(0);
        self.out.push_str("{\n");
        for x in f {
            self.before(x.span.line, depth + 1);
            let gap = " ".repeat(width - x.name.chars().count());
            self.out.push_str(&format!("{pad}{INDENT}{}:{gap} {}", x.name, ty(&x.ty)));
            if let Some(d) = &x.default {
                self.out.push_str(&format!(" default {}", expr(d)));
            }
            self.out.push('\n');
            self.trailing(x.span.line);
        }
        self.out.push_str(&format!("{pad}}}\n"));
    }

    fn trust(&mut self, t: &TrustDecl) {
        self.out.push_str(&format!("trust {}", t.name));
        if !t.subject.is_empty() {
            self.out.push_str(&format!("({}: {})", t.subject, t.subject_type));
        }
        if let Some(r) = &t.refines {
            self.out.push_str(&format!(" refines {r}"));
        }
        self.out.push_str(" {\n");
        if let Some(a) = &t.anchor {
            self.before(t.anchor_span.line, 1);
            self.out.push_str(&format!("{INDENT}anchor: {}\n", quoted(a)));
            self.trailing(t.anchor_span.line);
        }
        if !t.requires.is_empty() {
            self.out.push_str(&format!("{INDENT}require {{\n"));
            for r in &t.requires {
                self.before(r.span().line, 2);
                self.out.push_str(&format!("{INDENT}{INDENT}{}\n", expr(r)));
                self.trailing(r.span().line);
            }
            self.out.push_str(&format!("{INDENT}}}\n"));
        }
        self.out.push_str("}\n");
    }

    fn function(&mut self, f: &FunctionDecl) {
        let ret = f.ret.as_ref().map(|t| format!(": {}", ty(t))).unwrap_or_default();
        self.out.push_str(&format!("function {}({}){} ", f.name, params(&f.params), ret));
        self.block(&f.body, 0);
    }

    fn action(&mut self, a: &ActionDecl) {
        self.out.push_str(&format!("action {} {{\n", a.name));
        for b in &a.phases {
            // No separator of its own: a blank line between phases is one the
            // author left, and `before` puts it back.
            self.before(b.span.line, 1);
            self.out.push_str(&format!("{INDENT}{} ", b.phase.name()));
            self.block(&b.stmts, 1);
        }
        self.out.push_str("}\n");
    }

    fn component(&mut self, c: &ComponentDecl) {
        let word = if c.exported { "export component" } else { "component" };
        self.out.push_str(&format!("{word} {}({}) ", c.name, params(&c.params)));
        self.tree(&c.tree, 0);
    }

    fn screen(&mut self, s: &ScreenDecl) {
        for d in &s.directives {
            self.out.push_str(&format!("@{}", d.name));
            if !d.args.is_empty() {
                self.out.push_str(&format!("({})", args(&d.args)));
            }
            self.out.push('\n');
        }
        self.out.push_str(&format!("screen {}", s.name));
        if !s.params.is_empty() {
            self.out.push_str(&format!("({})", params(&s.params)));
        }
        for a in &s.settings {
            self.out
                .push_str(&format!(" {}: {}", a.name.clone().unwrap_or_default(), expr(&a.value)));
        }
        self.out.push_str(" {\n");

        if let Some(t) = &s.title {
            let value = t.args.first().map(|a| expr(&a.value)).unwrap_or_default();
            self.out.push_str(&format!("{INDENT}title: {value}\n"));
        }
        if !s.data.is_empty() {
            self.out.push_str(&format!("{INDENT}data {{\n"));
            for d in &s.data {
                self.before(d.span.line, 2);
                self.out.push_str(&format!("{INDENT}{INDENT}{}: {}\n", d.name, source(&d.source)));
                self.trailing(d.span.line);
            }
            self.out.push_str(&format!("{INDENT}}}\n"));
        }
        if !s.compute.is_empty() {
            self.before(s.compute.first().map(|x| x.span().line).unwrap_or(0), 1);
            self.out.push_str(&format!("{INDENT}compute "));
            self.block(&s.compute, 1);
        }
        for n in &s.tree {
            self.node(n, 1);
        }
        self.out.push_str("}\n");
    }

    // -------------------------------------------------------------- bodies

    fn block(&mut self, stmts: &[Stmt], depth: usize) {
        let pad = INDENT.repeat(depth);
        // A run of patches or of input bindings is a table, like a field list,
        // and lines up like one.
        let width = stmts
            .iter()
            .filter_map(|s| match s {
                Stmt::Patch { path, .. } => Some(path.join(".").chars().count()),
                Stmt::Binding { name, .. } => Some(name.chars().count()),
                _ => None,
            })
            .max()
            .unwrap_or(0);

        self.out.push_str("{\n");
        for s in stmts {
            self.before(s.span().line, depth + 1);
            self.stmt(s, depth + 1, width);
            self.trailing(s.span().line);
        }
        self.out.push_str(&format!("{pad}}}\n"));
    }

    fn stmt(&mut self, s: &Stmt, depth: usize, width: usize) {
        let pad = INDENT.repeat(depth);
        match s {
            Stmt::Let { name, value, mutable, .. } => {
                let word = if *mutable { "let" } else { "const" };
                self.out.push_str(&format!("{pad}{word} {name} = {}\n", expr_at(value, depth)));
            }
            Stmt::Assign { name, value, .. } => {
                self.out.push_str(&format!("{pad}{name} = {}\n", expr_at(value, depth)));
            }
            Stmt::Destructure { names, value, mutable, .. } => {
                let word = if *mutable { "let" } else { "const" };
                self.out.push_str(&format!(
                    "{pad}{word} {{ {} }} = {}\n",
                    names.join(", "),
                    expr_at(value, depth)
                ));
            }
            Stmt::Expr { value, .. } => self.out.push_str(&format!("{pad}{}\n", expr_at(value, depth))),
            Stmt::Patch { path, value, .. } => {
                let written = path.join(".");
                let gap = " ".repeat(width.saturating_sub(written.chars().count()));
                self.out
                    .push_str(&format!("{pad}{written}:{gap} {}\n", expr_at(value, depth)));
            }
            Stmt::Binding { name, ty: t, .. } => {
                let gap = " ".repeat(width.saturating_sub(name.chars().count()));
                self.out.push_str(&format!("{pad}{name}:{gap} {}\n", ty(t)));
            }
            Stmt::Effect { name, args: a, body, .. } => {
                // `disclose x` and `prove x` are written without parentheses,
                // and so is `navigate Home`: they are lines of a `present`
                // block rather than calls.
                if matches!(name.as_str(), "disclose" | "prove" | "navigate") && body.is_empty() {
                    let one = a.first().map(|x| expr_at(&x.value, depth)).unwrap_or_default();
                    self.out.push_str(&format!("{pad}{name} {one}\n"));
                    return;
                }
                self.out.push_str(&format!("{pad}{name}"));
                if !a.is_empty() {
                    self.out.push_str(&format!("({})", args_at(a, depth)));
                }
                if body.is_empty() {
                    self.out.push('\n');
                } else {
                    self.out.push(' ');
                    self.block(body, depth);
                }
            }
            Stmt::Return { value, .. } => {
                self.out.push_str(&format!("{pad}return {}\n", expr_at(value, depth)));
            }
            Stmt::Data { name, source: s, .. } => {
                self.out.push_str(&format!("{pad}const {name} = {}\n", source(s)));
            }
            Stmt::If { cond, then, other, .. } => {
                self.out.push_str(&format!("{pad}if ({}) ", expr_at(cond, depth)));
                self.block(then, depth);
                if !other.is_empty() {
                    self.out.pop();
                    self.out.push_str(" else ");
                    self.block(other, depth);
                }
            }
            Stmt::Refuse { key, .. } => {
                self.out.push_str(&format!("{pad}refuse {}\n", quoted(key)));
            }
        }
    }

    fn tree(&mut self, nodes: &[UiNode], depth: usize) {
        let pad = INDENT.repeat(depth);
        self.out.push_str("{\n");
        for n in nodes {
            self.node(n, depth + 1);
        }
        self.out.push_str(&format!("{pad}}}\n"));
    }

    fn node(&mut self, n: &UiNode, depth: usize) {
        let pad = INDENT.repeat(depth);
        self.before(n.span.line, depth);

        if n.kind == "if" {
            let cond = n.args.first().map(|a| expr(&a.value)).unwrap_or_default();
            self.out.push_str(&format!("{pad}if ({cond}) "));
            self.tree(&n.children, depth);
            if !n.otherwise.is_empty() {
                self.out.pop();
                self.out.push_str(" else ");
                self.tree(&n.otherwise, depth);
            }
            return;
        }
        if n.kind == "for" {
            let over = n.args.first().map(|a| expr(&a.value)).unwrap_or_default();
            let bind = n.lambda.clone().unwrap_or_default();
            self.out.push_str(&format!("{pad}for ({bind} in {over}) "));
            self.tree(&n.children, depth);
            return;
        }

        let (props, positional): (Vec<&Arg>, Vec<&Arg>) =
            n.args.iter().partition(|a| a.name.is_some());

        self.out.push_str(&format!("{pad}{}", n.kind));
        if !positional.is_empty() {
            let inner: Vec<String> = positional.iter().map(|a| expr(&a.value)).collect();
            self.out.push_str(&format!("({})", inner.join(", ")));
        }

        if props.is_empty() && n.children.is_empty() && n.lambda.is_none() {
            self.out.push('\n');
            self.trailing(n.span.line);
            return;
        }

        // A node with nothing under it and one thing to say says it on one
        // line. `card(text: …)` and `card { text: … }` are the same node, and a
        // printer has to pick one — this is the one the language is written in.
        if n.children.is_empty() && n.lambda.is_none() && props.len() == 1 {
            let only = props[0];
            let line = format!(
                "{}: {}",
                only.name.clone().unwrap_or_default(),
                expr_at(&only.value, depth)
            );
            let inline = positional.is_empty()
                && pad.len() + n.kind.chars().count() + line.chars().count() + 2 <= 96;
            if inline {
                self.out.push_str(&format!("({line})\n"));
                self.trailing(n.span.line);
                return;
            }
        }

        self.out.push_str(" {\n");
        self.trailing(n.span.line);
        if let Some(bind) = &n.lambda {
            self.out.push_str(&format!("{pad}{INDENT}{bind} ->\n"));
        }
        for a in props {
            self.before(a.span.line, depth + 1);
            self.out.push_str(&format!(
                "{pad}{INDENT}{}: {}\n",
                a.name.clone().unwrap_or_default(),
                expr(&a.value)
            ));
            self.trailing(a.span.line);
        }
        for c in &n.children {
            self.node(c, depth + 1);
        }
        self.out.push_str(&format!("{pad}}}\n"));
    }
}

#[derive(Clone, Copy)]
enum Item {
    App,
    Version,
    Host(usize),
    Import(usize),
    Capabilities,
    Admits,
    Enum(usize),
    Credential(usize),
    Type(usize),
    State,
    Trust(usize),
    Function(usize),
    Action(usize),
    Component(usize),
    Screen(usize),
}

fn source(s: &DataSource) -> String {
    match s {
        DataSource::Credentials { ty, policy, order, limit } => {
            let mut out = format!("credentials of {ty}");
            if let Some(p) = policy {
                out.push_str(&format!(" verified with {p}"));
            }
            if let Some((claim, descending)) = order {
                out.push_str(&format!(
                    " order by {claim} {}",
                    if *descending { "desc" } else { "asc" }
                ));
            }
            if let Some(n) = limit {
                out.push_str(&format!(" limit {n}"));
            }
            out
        }
        DataSource::Query { audience } if audience.is_empty() => "query".to_string(),
        DataSource::Query { audience } => format!("query {audience}"),
        // Only reachable in a program that did not parse, and one of those is
        // never printed: the formatter refuses a file whose shape nobody knows.
        DataSource::Unknown => "query".to_string(),
    }
}

// -------------------------------------------------------------- expressions

fn expr(e: &Expr) -> String {
    expr_at(e, 0)
}

/// The same, told what column it is being printed at — which a switch needs and
/// nothing else does. Taken from the printer's own indent rather than from
/// where the expression was in the source: the source position is where it used
/// to be, and printing it there is why printing twice gave two texts.
fn expr_at(e: &Expr, depth: usize) -> String {
    match e {
        Expr::Num { text, .. } => text.clone(),
        Expr::Float { text, .. } => text.clone(),
        Expr::Str { value, .. } => quoted(value),
        Expr::Bool { value, .. } => value.to_string(),
        Expr::Ident { name, .. } => name.clone(),
        Expr::Member { obj, name, optional, .. } => {
            format!("{}{}.{name}", expr_at(obj, depth), if *optional { "?" } else { "" })
        }
        Expr::Call { callee, args: a, .. } => {
            // A function written in place goes after the parentheses, which is
            // where the language puts it: `xs.fold(0) { sum, x -> … }`.
            match a.split_last() {
                Some((Arg { value: Expr::Lambda { params: ps, body, .. }, .. }, rest)) => format!(
                    "{}({}) {{ {} -> {} }}",
                    expr_at(callee, depth),
                    args(rest),
                    ps.join(", "),
                    expr_at(body, depth)
                ),
                _ => format!("{}({})", expr_at(callee, depth), args_at(a, depth)),
            }
        }
        Expr::Unary { op, rhs, .. } => format!("{op}{}", expr_at(rhs, depth)),
        Expr::Binary { op, lhs, rhs, .. } => {
            // Parenthesised only where the parser would read it differently
            // without them. Wrapping every operator is correct and unreadable,
            // and a formatter is read far more often than it is run.
            let bp = binding_power(op);
            format!("{} {op} {}", at_least(lhs, bp, depth), at_least(rhs, bp + 1, depth))
        }
        Expr::Ternary { cond, then, other, .. } => {
            format!("{} ? {} : {}", at_least(cond, 1, depth), expr_at(then, depth), expr_at(other, depth))
        }
        Expr::With { subject, policy, .. } => format!("{} with {policy}", expr_at(subject, depth)),
        Expr::Exists { subject, .. } => format!("{} exists", expr_at(subject, depth)),
        Expr::Elvis { subject, other, .. } => format!("{} ?: {}", at_least(subject, 1, depth), expr_at(other, depth)),
        Expr::Record { spread, fields: f, .. } => {
            let mut parts = Vec::new();
            if let Some(s) = spread {
                parts.push(format!("...{}", expr_at(s, depth)));
            }
            for (k, v) in f {
                parts.push(format!("{k}: {}", expr_at(v, depth)));
            }
            format!("{{ {} }}", parts.join(", "))
        }
        Expr::List { items, .. } => {
            format!("[{}]", items.iter().map(|x| expr_at(x, depth)).collect::<Vec<_>>().join(", "))
        }
        Expr::Switch { subject, arms, span } => {
            // One arm to a line, and the arrows lined up. A switch reads as a
            // table — that is why it is an expression here rather than a chain
            // of ternaries — and a table on one line is a list.
            let patterns: Vec<String> = arms
                .iter()
                .map(|a| match &a.pattern {
                    ArmPattern::Default => "default".to_string(),
                    ArmPattern::Value(v) => expr_at(v, depth),
                    ArmPattern::Compare { op, rhs } => format!("{op} {}", expr_at(rhs, depth)),
                })
                .collect();
            let width = patterns.iter().map(|p| p.chars().count()).max().unwrap_or(0);
            // The indent is the switch's own column, which is where the line it
            // sits on starts: a nested one lines up under itself.
            let _ = span;
            let pad = INDENT.repeat(depth);
            let mut out = format!("switch ({}) {{\n", expr_at(subject, depth));
            for (p, a) in patterns.iter().zip(arms) {
                let gap = " ".repeat(width - p.chars().count());
                out.push_str(&format!("{pad}  {p}{gap} => {},\n", expr(&a.body)));
            }
            out.push_str(&format!("{pad}}}"));
            out
        }
        Expr::Lambda { params: ps, body, .. } => {
            format!("{{ {} -> {} }}", ps.join(", "), expr_at(body, depth))
        }
        Expr::From { value, policies, .. } => {
            format!("{} from {{ {} }}", expr_at(value, depth), policies.join(" "))
        }
        Expr::Error { .. } => "?".to_string(),
    }
}

/// The parser's table, so the printer parenthesises exactly where the parser
/// would read it differently without them. Two copies of one table is how the
/// two come to disagree, and this one is read from the same place.
fn binding_power(op: &str) -> u8 {
    crate::parse::binding_power_of(op).unwrap_or(0)
}

/// An operand, wrapped only if its own operator binds looser than the one it
/// sits under.
fn at_least(e: &Expr, want: u8, depth: usize) -> String {
    let mine = match e {
        Expr::Binary { op, .. } => binding_power(op),
        Expr::Ternary { .. } | Expr::Elvis { .. } => 1,
        _ => u8::MAX,
    };
    if mine < want {
        format!("({})", expr_at(e, depth))
    } else {
        expr_at(e, depth)
    }
}

fn args(a: &[Arg]) -> String {
    args_at(a, 0)
}

fn args_at(a: &[Arg], depth: usize) -> String {
    a.iter()
        .map(|x| {
            let spread = if x.spread { "..." } else { "" };
            match &x.name {
                Some(n) => format!("{spread}{n}: {}", expr_at(&x.value, depth)),
                None => format!("{spread}{}", expr_at(&x.value, depth)),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn params(f: &[Field]) -> String {
    f.iter()
        .map(|x| {
            let mut out = format!("{}: {}", x.name, ty(&x.ty));
            if let Some(d) = &x.default {
                out.push_str(&format!(" default {}", expr(d)));
            }
            out
        })
        .collect::<Vec<_>>()
        .join(", ")
}


fn ty(t: &TypeRef) -> String {
    let mut out = t.name.clone();
    if !t.args.is_empty() {
        out.push_str(&format!("<{}>", t.args.iter().map(ty).collect::<Vec<_>>().join(", ")));
    }
    if t.optional {
        out.push('?');
    }
    out
}

/// A string, as source. The escapes are the four the lexer reads.
fn quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
