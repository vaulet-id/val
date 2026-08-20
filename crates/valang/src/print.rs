//! The program, written back out.
//!
//! One shape per thing, so that a file has a form somebody's editor can produce
//! and a reviewer can diff. That is the visible half; the useful half is that a
//! printer makes the parser testable against itself — `print(parse(print(parse
//! (x))))` and `print(parse(x))` have to be the same text, and a production the
//! parser reads but the printer cannot write is a production one of them does
//! not really have.
//!
//! Every match here is exhaustive on purpose. A node added to the language
//! stops this compiling, which is the only way a printer stays complete.

use crate::ast::*;

const INDENT: &str = "  ";

pub fn print(p: &Program) -> String {
    let mut out = String::new();

    if let Some(app) = &p.app {
        out.push_str(&format!("app {}\n", quoted(app)));
    }
    if let Some(v) = &p.version {
        out.push_str(&format!("version {v}\n"));
    }
    for h in &p.hosts {
        out.push_str(&format!("host {}\n", quoted(h)));
    }
    for i in &p.imports {
        out.push_str(&format!("\nimport {} {{ {} }}\n", quoted(&i.package), i.names.join(", ")));
    }

    out.push_str("\ncapabilities {\n");
    for c in &p.capabilities {
        out.push_str(&format!("{INDENT}{}", c.name));
        if !c.args.is_empty() {
            out.push_str(&format!("({})", args(&c.args)));
        }
        out.push('\n');
    }
    out.push_str("}\n");

    for e in &p.enums {
        out.push_str(&format!("\nenum {} {{ {} }}\n", e.name, e.members.join(", ")));
    }
    for c in &p.credentials {
        out.push_str(&format!("\ncredential {} {}", c.name, fields(&c.fields, 0)));
    }
    for t in &p.types {
        out.push_str(&format!("\ntype {} {}", t.name, fields(&t.fields, 0)));
    }
    if !p.state.is_empty() {
        out.push_str(&format!("\nstate {}", fields(&p.state, 0)));
    }
    for t in &p.trusts {
        out.push_str(&trust(t));
    }
    for f in &p.functions {
        out.push_str(&function(f));
    }
    for a in &p.actions {
        out.push_str(&action(a));
    }
    for c in &p.components {
        out.push_str(&component(c));
    }
    for s in &p.screens {
        out.push_str(&screen(s));
    }

    out
}

// ------------------------------------------------------------- declarations

fn trust(t: &TrustDecl) -> String {
    let mut out = format!("\ntrust {}", t.name);
    if !t.subject.is_empty() {
        out.push_str(&format!("({}: {})", t.subject, t.subject_type));
    }
    if let Some(r) = &t.refines {
        out.push_str(&format!(" refines {r}"));
    }
    out.push_str(" {\n");
    if let Some(a) = &t.anchor {
        out.push_str(&format!("{INDENT}anchor: {}\n", quoted(a)));
    }
    if !t.requires.is_empty() {
        out.push_str(&format!("{INDENT}require {{\n"));
        for r in &t.requires {
            out.push_str(&format!("{INDENT}{INDENT}{}\n", expr(r)));
        }
        out.push_str(&format!("{INDENT}}}\n"));
    }
    out.push_str("}\n");
    out
}

fn function(f: &FunctionDecl) -> String {
    let ret = f.ret.as_ref().map(|t| format!(": {}", ty(t))).unwrap_or_default();
    format!("\nfunction {}({}){} {}", f.name, params(&f.params), ret, block(&f.body, 0))
}

fn action(a: &ActionDecl) -> String {
    let mut out = format!("\naction {} {{\n", a.name);
    for (i, b) in a.phases.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("{INDENT}{} {}", b.phase.name(), block(&b.stmts, 1)));
    }
    out.push_str("}\n");
    out
}

fn component(c: &ComponentDecl) -> String {
    let word = if c.exported { "export component" } else { "component" };
    format!("\n{word} {}({}) {}", c.name, params(&c.params), tree(&c.tree, 0))
}

fn screen(s: &ScreenDecl) -> String {
    let mut out = String::from("\n");
    for d in &s.directives {
        out.push_str(&format!("@{}", d.name));
        if !d.args.is_empty() {
            out.push_str(&format!("({})", args(&d.args)));
        }
        out.push('\n');
    }
    out.push_str(&format!("screen {}", s.name));
    if !s.params.is_empty() {
        out.push_str(&format!("({})", params(&s.params)));
    }
    for a in &s.settings {
        out.push_str(&format!(" {}: {}", a.name.clone().unwrap_or_default(), expr(&a.value)));
    }
    out.push_str(" {\n");

    if let Some(t) = &s.title {
        let value = t.args.first().map(|a| expr(&a.value)).unwrap_or_default();
        out.push_str(&format!("{INDENT}title: {value}\n\n"));
    }
    if !s.data.is_empty() {
        out.push_str(&format!("{INDENT}data {{\n"));
        for d in &s.data {
            out.push_str(&format!("{INDENT}{INDENT}{}: {}\n", d.name, source(&d.source)));
        }
        out.push_str(&format!("{INDENT}}}\n\n"));
    }
    if !s.compute.is_empty() {
        out.push_str(&format!("{INDENT}compute {}\n", block(&s.compute, 1)));
    }
    for n in &s.tree {
        out.push_str(&node(n, 1));
    }
    out.push_str("}\n");
    out
}

fn source(s: &DataSource) -> String {
    match s {
        DataSource::Credentials { ty, policy, limit } => {
            let mut out = format!("credentials of {ty}");
            if let Some(p) = policy {
                out.push_str(&format!(" verified with {p}"));
            }
            if let Some(n) = limit {
                out.push_str(&format!(" limit {n}"));
            }
            out
        }
        DataSource::Query { audience } => format!("query {audience}"),
        DataSource::Unknown => "query".to_string(),
    }
}

// --------------------------------------------------------------- statements

fn block(stmts: &[Stmt], depth: usize) -> String {
    let pad = INDENT.repeat(depth);
    let mut out = String::from("{\n");
    for s in stmts {
        out.push_str(&stmt(s, depth + 1));
    }
    out.push_str(&format!("{pad}}}\n"));
    out
}

fn stmt(s: &Stmt, depth: usize) -> String {
    let pad = INDENT.repeat(depth);
    match s {
        Stmt::Let { name, value, mutable, .. } => {
            let word = if *mutable { "let" } else { "const" };
            format!("{pad}{word} {name} = {}\n", expr(value))
        }
        Stmt::Assign { name, value, .. } => format!("{pad}{name} = {}\n", expr(value)),
        Stmt::Destructure { names, value, mutable, .. } => {
            let word = if *mutable { "let" } else { "const" };
            format!("{pad}{word} {{ {} }} = {}\n", names.join(", "), expr(value))
        }
        Stmt::Expr { value, .. } => format!("{pad}{}\n", expr(value)),
        Stmt::Patch { path, value, .. } => {
            format!("{pad}{}: {}\n", path.join("."), expr(value))
        }
        Stmt::Binding { name, ty: t, .. } => format!("{pad}{name}: {}\n", ty(t)),
        Stmt::Effect { name, args: a, body, .. } => {
            let mut out = format!("{pad}{name}");
            if !a.is_empty() {
                out.push_str(&format!("({})", args(a)));
            }
            if body.is_empty() {
                out.push('\n');
            } else {
                out.push(' ');
                out.push_str(&block(body, depth));
            }
            out
        }
        Stmt::Return { value, .. } => format!("{pad}return {}\n", expr(value)),
        Stmt::Data { name, source: s, .. } => format!("{pad}const {name} = {}\n", source(s)),
        Stmt::If { cond, then, other, .. } => {
            let mut out = format!("{pad}if ({}) {}", expr(cond), block(then, depth));
            if !other.is_empty() {
                // The trailing newline of the `then` block, replaced by ` else`.
                out.pop();
                out.push_str(&format!(" else {}", block(other, depth)));
            }
            out
        }
        Stmt::Refuse { key, .. } => format!("{pad}refuse {}\n", quoted(key)),
    }
}

// ----------------------------------------------------------------- the tree

fn tree(nodes: &[UiNode], depth: usize) -> String {
    let pad = INDENT.repeat(depth);
    let mut out = String::from("{\n");
    for n in nodes {
        out.push_str(&node(n, depth + 1));
    }
    out.push_str(&format!("{pad}}}\n"));
    out
}

fn node(n: &UiNode, depth: usize) -> String {
    let pad = INDENT.repeat(depth);

    if n.kind == "if" {
        let cond = n.args.first().map(|a| expr(&a.value)).unwrap_or_default();
        let mut out = format!("{pad}if ({cond}) {}", tree(&n.children, depth));
        if !n.otherwise.is_empty() {
            out.pop();
            out.push_str(&format!(" else {}", tree(&n.otherwise, depth)));
        }
        return out;
    }
    if n.kind == "for" {
        let over = n.args.first().map(|a| expr(&a.value)).unwrap_or_default();
        let bind = n.lambda.clone().unwrap_or_default();
        return format!("{pad}for ({bind} in {over}) {}", tree(&n.children, depth));
    }

    // A phrase was flattened into `text` and its slots; written back out it is
    // the sentence and the values beside it, which is what somebody wrote.
    let (props, positional): (Vec<&Arg>, Vec<&Arg>) =
        n.args.iter().partition(|a| a.name.is_some());

    let mut head = format!("{pad}{}", n.kind);
    if !positional.is_empty() {
        let inner: Vec<String> = positional.iter().map(|a| expr(&a.value)).collect();
        head.push_str(&format!("({})", inner.join(", ")));
    }

    if props.is_empty() && n.children.is_empty() && n.lambda.is_none() {
        head.push('\n');
        return head;
    }

    head.push_str(" {\n");
    if let Some(bind) = &n.lambda {
        head.push_str(&format!("{pad}{INDENT}{bind} ->\n"));
    }
    for a in props {
        head.push_str(&format!(
            "{pad}{INDENT}{}: {}\n",
            a.name.clone().unwrap_or_default(),
            expr(&a.value)
        ));
    }
    for c in &n.children {
        head.push_str(&node(c, depth + 1));
    }
    head.push_str(&format!("{pad}}}\n"));
    head
}

// -------------------------------------------------------------- expressions

fn expr(e: &Expr) -> String {
    match e {
        Expr::Num { value, .. } => value.to_string(),
        Expr::Float { text, .. } => text.clone(),
        Expr::Str { value, .. } => quoted(value),
        Expr::Bool { value, .. } => value.to_string(),
        Expr::Ident { name, .. } => name.clone(),
        Expr::Member { obj, name, optional, .. } => {
            format!("{}{}.{name}", expr(obj), if *optional { "?" } else { "" })
        }
        Expr::Call { callee, args: a, .. } => {
            // A function written in place goes after the parentheses, which is
            // where the language puts it: `xs.fold(0) { sum, x -> … }`.
            match a.split_last() {
                Some((Arg { value: Expr::Lambda { params: ps, body, .. }, .. }, rest)) => format!(
                    "{}({}) {{ {} -> {} }}",
                    expr(callee),
                    args(rest),
                    ps.join(", "),
                    expr(body)
                ),
                _ => format!("{}({})", expr(callee), args(a)),
            }
        }
        Expr::Unary { op, rhs, .. } => format!("{op}{}", expr(rhs)),
        Expr::Binary { op, lhs, rhs, .. } => format!("({} {op} {})", expr(lhs), expr(rhs)),
        Expr::Ternary { cond, then, other, .. } => {
            format!("({} ? {} : {})", expr(cond), expr(then), expr(other))
        }
        Expr::With { subject, policy, .. } => format!("{} with {policy}", expr(subject)),
        Expr::Exists { subject, .. } => format!("{} exists", expr(subject)),
        Expr::Elvis { subject, other, .. } => format!("({} ?: {})", expr(subject), expr(other)),
        Expr::Record { spread, fields: f, .. } => {
            let mut parts = Vec::new();
            if let Some(s) = spread {
                parts.push(format!("...{}", expr(s)));
            }
            for (k, v) in f {
                parts.push(format!("{k}: {}", expr(v)));
            }
            format!("{{ {} }}", parts.join(", "))
        }
        Expr::List { items, .. } => {
            format!("[{}]", items.iter().map(expr).collect::<Vec<_>>().join(", "))
        }
        Expr::Switch { subject, arms, .. } => {
            let mut out = format!("switch ({}) {{ ", expr(subject));
            for a in arms {
                let pattern = match &a.pattern {
                    ArmPattern::Default => "default".to_string(),
                    ArmPattern::Value(v) => expr(v),
                    ArmPattern::Compare { op, rhs } => format!("{op} {}", expr(rhs)),
                };
                out.push_str(&format!("{pattern} => {}, ", expr(&a.body)));
            }
            out.push('}');
            out
        }
        Expr::Lambda { params: ps, body, .. } => {
            format!("{{ {} -> {} }}", ps.join(", "), expr(body))
        }
        Expr::From { value, policies, .. } => {
            format!("{} from {{ {} }}", expr(value), policies.join(" "))
        }
        Expr::Error { .. } => "?".to_string(),
    }
}

fn args(a: &[Arg]) -> String {
    a.iter()
        .map(|x| {
            let spread = if x.spread { "..." } else { "" };
            match &x.name {
                Some(n) => format!("{spread}{n}: {}", expr(&x.value)),
                None => format!("{spread}{}", expr(&x.value)),
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

fn fields(f: &[Field], depth: usize) -> String {
    let pad = INDENT.repeat(depth);
    let mut out = String::from("{\n");
    for x in f {
        out.push_str(&format!("{pad}{INDENT}{}: {}", x.name, ty(&x.ty)));
        if let Some(d) = &x.default {
            out.push_str(&format!(" default {}", expr(d)));
        }
        out.push('\n');
    }
    out.push_str(&format!("{pad}}}\n"));
    out
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
