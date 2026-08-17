//! Recursive descent for the shell, Pratt for expressions.
//!
//! The shell may invent; the expression layer may not (§2). That shows up here
//! as a large, boring set of block parsers over a small operator table.

use crate::ast::*;
use crate::diag::Diagnostic;
use crate::lex::{Kind, Lexer, Token};

pub struct Parser {
    toks: Vec<Token>,
    i: usize,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(src: &str) -> (Program, Vec<Diagnostic>) {
    let (toks, lex_diags) = Lexer::new(src).run();
    let mut p = Parser { toks, i: 0, diagnostics: lex_diags };
    let program = p.program();
    (program, p.diagnostics)
}

impl Parser {
    // ------------------------------------------------------------- plumbing

    fn peek(&self) -> &Token {
        self.toks.get(self.i).unwrap_or(self.toks.last().unwrap())
    }
    fn peek_at(&self, n: usize) -> &Token {
        self.toks.get(self.i + n).unwrap_or(self.toks.last().unwrap())
    }
    fn at(&self, s: &str) -> bool {
        self.peek().is(s)
    }
    fn eof(&self) -> bool {
        self.peek().kind == Kind::Eof
    }
    fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if !self.eof() {
            self.i += 1;
        }
        t
    }
    fn eat(&mut self, s: &str) -> bool {
        if self.at(s) {
            self.bump();
            true
        } else {
            false
        }
    }
    /// Newlines separate statements; everywhere else they are noise.
    fn skip_newlines(&mut self) {
        while self.peek().kind == Kind::Newline {
            self.bump();
        }
    }
    fn expect(&mut self, s: &str) -> bool {
        self.skip_newlines();
        if self.eat(s) {
            return true;
        }
        let t = self.peek().clone();
        let found = if t.kind == Kind::Eof { "end of file".to_string() } else { format!("`{}`", t.text) };
        self.diagnostics.push(Diagnostic::error(t.span, format!("expected `{s}`, found {found}")));
        false
    }
    fn ident(&mut self) -> String {
        self.skip_newlines();
        if self.peek().kind == Kind::Ident {
            self.bump().text
        } else {
            let t = self.peek().clone();
            self.diagnostics.push(Diagnostic::error(t.span, format!("expected a name, found `{}`", t.text)));
            String::new()
        }
    }

    /// Consume a balanced block, for recovery after an error inside one.
    fn skip_block(&mut self) {
        if !self.eat("{") {
            return;
        }
        let mut depth = 1;
        while !self.eof() && depth > 0 {
            if self.at("{") {
                depth += 1;
            } else if self.at("}") {
                depth -= 1;
            }
            self.bump();
        }
    }

    // -------------------------------------------------------------- program

    fn program(&mut self) -> Program {
        let mut p = Program::default();
        loop {
            self.skip_newlines();
            if self.eof() {
                break;
            }
            let t = self.peek().clone();
            match t.text.as_str() {
                "app" => {
                    self.bump();
                    self.skip_newlines();
                    if self.peek().kind == Kind::Str {
                        p.app = Some(self.bump().text);
                    } else {
                        let bad = self.bump();
                        self.diagnostics.push(Diagnostic::error(
                            bad.span,
                            "the application identifier is a quoted string: a reverse-DNS name and a field access are the same shape",
                        ));
                    }
                }
                "version" => {
                    self.bump();
                    p.version = Some(self.bump().text);
                }
                "capabilities" => {
                    self.bump();
                    p.capabilities = self.capabilities();
                }
                "enum" => p.enums.push(self.enum_decl()),
                "credential" => p.credentials.push(self.credential_decl()),
                "state" => {
                    self.bump();
                    p.state = self.fields();
                }
                "trust" => p.trusts.push(self.trust_decl()),
                "function" => p.functions.push(self.function_decl()),
                "action" => p.actions.push(self.action_decl()),
                "screen" => p.screens.push(self.screen_decl()),
                _ => {
                    let bad = self.bump();
                    if bad.kind != Kind::Eof {
                        self.diagnostics.push(Diagnostic::error(
                            bad.span,
                            format!("`{}` does not start a declaration", bad.text),
                        ));
                        self.skip_block();
                    }
                }
            }
        }
        p
    }

    fn capabilities(&mut self) -> Vec<Capability> {
        let mut out = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            let span = self.peek().span;
            let name = self.dotted();
            let args = if self.at("(") { self.args() } else { Vec::new() };
            if name.is_empty() {
                self.bump();
                continue;
            }
            out.push(Capability { name, args, span });
        }
        out
    }

    fn dotted(&mut self) -> String {
        self.skip_newlines();
        if self.peek().kind != Kind::Ident {
            return String::new();
        }
        let mut s = self.bump().text;
        while self.at(".") && self.peek_at(1).kind == Kind::Ident {
            self.bump();
            s.push('.');
            s.push_str(&self.bump().text);
        }
        s
    }

    fn enum_decl(&mut self) -> EnumDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        let mut members = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            if self.peek().kind == Kind::Ident {
                members.push(self.bump().text);
            } else {
                self.bump();
            }
        }
        EnumDecl { name, members, span }
    }

    fn credential_decl(&mut self) -> CredentialDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        let fields = self.fields();
        CredentialDecl { name, fields, span }
    }

    fn fields(&mut self) -> Vec<Field> {
        let mut out = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            let span = self.peek().span;
            if self.peek().kind != Kind::Ident {
                self.bump();
                continue;
            }
            let name = self.bump().text;
            if !self.eat(":") {
                continue;
            }
            let ty = self.type_ref();
            // `default 0`, never `= 0` — `=` appears nowhere in this language.
            let default = if self.at("default") {
                self.bump();
                Some(self.expr(0))
            } else if self.at("=") {
                let bad = self.bump().span;
                self.diagnostics.push(Diagnostic::error(
                    bad,
                    "a state field declares its starting value with `default`, not `=`. `=` appears nowhere in this language",
                ));
                Some(self.expr(0))
            } else {
                None
            };
            out.push(Field { name, ty, default, span });
        }
        out
    }

    fn type_ref(&mut self) -> TypeRef {
        self.skip_newlines();
        let name = if self.peek().kind == Kind::Ident { self.bump().text } else { String::new() };
        let mut args = Vec::new();
        if self.at("<") {
            self.bump();
            loop {
                if self.eof() || self.eat(">") {
                    break;
                }
                if self.eat(",") {
                    continue;
                }
                args.push(self.type_ref());
            }
        }
        let optional = self.eat("?");
        TypeRef { name, args, optional }
    }

    fn trust_decl(&mut self) -> TrustDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        let (mut subject, mut subject_type) = (String::new(), String::new());
        if self.eat("(") {
            subject = self.ident();
            if self.eat(":") {
                subject_type = self.ident();
            }
            self.expect(")");
        }
        let refines = if self.at("refines") {
            self.bump();
            Some(self.ident())
        } else {
            None
        };

        let mut anchor = None;
        let mut requires = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            if self.at("anchor") {
                self.bump();
                if self.eat(":") {
                    self.skip_newlines();
                    anchor = Some(self.bump().text);
                } else if self.at("{") {
                    let bad = self.peek().span;
                    self.diagnostics.push(Diagnostic::error(bad, "`anchor:` is a field, not a block"));
                    self.skip_block();
                }
                continue;
            }
            if self.at("require") {
                self.bump();
                self.expect("{");
                loop {
                    self.skip_newlines();
                    if self.eof() || self.eat("}") {
                        break;
                    }
                    requires.push(self.expr(0));
                }
                continue;
            }
            self.bump();
        }
        TrustDecl { name, subject, subject_type, refines, anchor, requires, span }
    }

    fn function_decl(&mut self) -> FunctionDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        let mut params = Vec::new();
        if self.eat("(") {
            loop {
                self.skip_newlines();
                if self.eof() || self.eat(")") {
                    break;
                }
                if self.eat(",") {
                    continue;
                }
                let pspan = self.peek().span;
                let pname = self.ident();
                self.eat(":");
                let ty = self.type_ref();
                params.push(Field { name: pname, ty, default: None, span: pspan });
            }
        }
        let ret = if self.eat(":") { Some(self.type_ref()) } else { None };
        let body = self.stmt_block();
        FunctionDecl { name, params, ret, body, span }
    }

    fn action_decl(&mut self) -> ActionDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        let mut phases = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            let pspan = self.peek().span;
            let word = self.peek().text.clone();
            match Phase::from_str(&word) {
                Some(phase) => {
                    self.bump();
                    let stmts = self.phase_stmts(phase);
                    phases.push(PhaseBlock { phase, stmts, span: pspan });
                }
                None => {
                    self.bump();
                    self.diagnostics.push(Diagnostic::error(
                        pspan,
                        format!(
                            "`{word}` is not a phase. An action is input → require → verify → compute → update → execute, and may omit but not reorder them"
                        ),
                    ));
                    self.skip_block();
                }
            }
        }
        ActionDecl { name, phases, span }
    }

    fn phase_stmts(&mut self, phase: Phase) -> Vec<Stmt> {
        let mut out = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            let before = self.i;
            if let Some(s) = self.stmt(Some(phase)) {
                out.push(s);
            }
            if self.i == before {
                self.bump();
            }
        }
        out
    }

    fn stmt_block(&mut self) -> Vec<Stmt> {
        let mut out = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            let before = self.i;
            if let Some(s) = self.stmt(None) {
                out.push(s);
            }
            if self.i == before {
                self.bump();
            }
        }
        out
    }

    fn stmt(&mut self, phase: Option<Phase>) -> Option<Stmt> {
        self.skip_newlines();
        let span = self.peek().span;

        if self.at("const") {
            self.bump();
            let name = self.ident();
            self.expect("=");
            // An action may re-declare the data a screen declared, because it
            // can be reached from somewhere other than that screen.
            if self.at("credentials") || self.at("query") {
                let source = self.data_source();
                return Some(Stmt::Data { name, source, span });
            }
            let value = self.expr(0);
            return Some(Stmt::Let { name, value, span });
        }
        if self.at("let") || self.at("var") {
            let word = self.bump().text;
            self.diagnostics.push(Diagnostic::error(
                span,
                format!("bindings use `const`; there is no `{word}` and nothing here is reassigned"),
            ));
            let name = self.ident();
            self.expect("=");
            let value = self.expr(0);
            return Some(Stmt::Let { name, value, span });
        }
        if self.at("return") {
            self.bump();
            let value = self.expr(0);
            return Some(Stmt::Return { value, span });
        }

        // `input` binds names to types; every other phase holds expressions.
        if phase == Some(Phase::Input) && self.peek().kind == Kind::Ident && self.peek_at(1).is(":") {
            let name = self.bump().text;
            self.bump();
            let ty = self.type_ref();
            return Some(Stmt::Binding { name, ty, span });
        }

        // `member.tier: tier` — a patch path, only in `update`.
        if self.peek().kind == Kind::Ident {
            let mut k = 0;
            let mut path = vec![self.peek().text.clone()];
            let mut indexed = false;
            loop {
                if self.peek_at(k + 1).is(".") && self.peek_at(k + 2).kind == Kind::Ident {
                    path.push(self.peek_at(k + 2).text.clone());
                    k += 2;
                } else if self.peek_at(k + 1).is("[") {
                    // Swallow it so the message can be about the path rather
                    // than about a bracket the expression parser tripped over.
                    indexed = true;
                    let mut j = k + 2;
                    while !self.peek_at(j).is("]") && self.peek_at(j).kind != Kind::Eof {
                        j += 1;
                    }
                    k = j;
                } else {
                    break;
                }
            }
            if self.peek_at(k + 1).is(":") {
                for _ in 0..=k + 1 {
                    self.bump();
                }
                let value = self.expr(0);
                if indexed {
                    self.diagnostics.push(Diagnostic::error(
                        span,
                        "a patch path may not contain a list index. That is where a patch would need an optics story and it does not have one: build the new list in `compute` and name it here in one line",
                    ));
                }
                return Some(Stmt::Patch { path, value, span });
            }
            if self.peek_at(k + 1).is("=") && phase == Some(Phase::Update) {
                for _ in 0..=k + 1 {
                    self.bump();
                }
                let value = self.expr(0);
                self.diagnostics.push(Diagnostic::error(
                    span,
                    format!(
                        "there is no assignment in this language. `update` is a patch: write `{}: …`, a colon, because the line describes the next state rather than changing this one — and `{}` is still readable on the right of it",
                        path.join(".").trim_start_matches("state."),
                        if path[0] == "state" { path.join(".") } else { format!("state.{}", path.join(".")) }
                    ),
                ));
                return Some(Stmt::Patch { path, value, span });
            }
        }

        // An effect, or `present { … }`.
        if self.peek().kind == Kind::Ident {
            let save = self.i;
            let name = self.dotted();
            let is_effect = name == "present"
                || matches!(
                    name.split('.').next().unwrap_or(""),
                    "credential" | "payment" | "storage" | "message" | "network" | "disclosure"
                ) && name.contains('.');
            if is_effect {
                let args = if self.at("(") { self.args() } else { Vec::new() };
                let body = if self.at("{") { self.stmt_block() } else { Vec::new() };
                return Some(Stmt::Effect { name, args, body, span });
            }
            // `disclose x` / `prove x` inside a `present` block.
            if name == "disclose" || name == "prove" {
                let value = self.expr(0);
                return Some(Stmt::Effect {
                    name,
                    args: vec![Arg { name: None, value, span }],
                    body: Vec::new(),
                    span,
                });
            }
            self.i = save;
        }

        let value = self.expr(0);
        Some(Stmt::Expr { value, span })
    }

    // ---------------------------------------------------------------- screen

    fn screen_decl(&mut self) -> ScreenDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        let mut data = Vec::new();
        let mut compute = Vec::new();
        let mut tree = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            if self.at("data") && self.peek_at(1).is("{") {
                self.bump();
                data = self.data_block();
                continue;
            }
            if self.at("compute") && self.peek_at(1).is("{") {
                self.bump();
                compute = self.stmt_block();
                continue;
            }
            let before = self.i;
            if let Some(n) = self.ui_node() {
                tree.push(n);
            }
            if self.i == before {
                self.bump();
            }
        }
        ScreenDecl { name, data, compute, tree, span }
    }

    fn data_block(&mut self) -> Vec<DataDecl> {
        let mut out = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            let span = self.peek().span;
            if self.peek().kind != Kind::Ident || !self.peek_at(1).is(":") {
                self.bump();
                continue;
            }
            let name = self.bump().text;
            self.bump();
            self.skip_newlines();

            let source = self.data_source();
            out.push(DataDecl { name, source, span });
        }
        out
    }

    fn data_source(&mut self) -> DataSource {
        self.skip_newlines();
        if self.at("credentials") {
                self.bump();
                self.eat("of");
                let ty = self.ident();
                let mut policy = None;
                let mut limit = None;
                loop {
                    if self.at("verified") {
                        self.bump();
                        self.eat("with");
                        policy = Some(self.ident());
                    } else if self.at("order") {
                        self.bump();
                        self.eat("by");
                        self.bump();
                        self.eat("desc");
                        self.eat("asc");
                    } else if self.at("limit") {
                        self.bump();
                        limit = self.bump().text.replace('_', "").parse().ok();
                    } else {
                        break;
                    }
                }
                DataSource::Credentials { ty, policy, limit }
            } else if self.at("query") {
                self.bump();
                let audience = self.dotted();
                if self.at("(") {
                    self.args();
                }
                if self.eat("as") {
                    self.type_ref();
                }
                DataSource::Query { audience }
            } else {
                self.expr(0);
                DataSource::Unknown
            }
    }

    fn ui_node(&mut self) -> Option<UiNode> {
        self.skip_newlines();
        if self.peek().kind != Kind::Ident {
            return None;
        }
        let span = self.peek().span;
        let kind = self.bump().text;
        let args = if self.at("(") { self.args() } else { Vec::new() };
        let mut children = Vec::new();
        if self.at("{") {
            self.bump();
            // `list(xs) { r -> … }`
            if self.peek().kind == Kind::Ident && self.peek_at(1).is("->") {
                self.bump();
                self.bump();
            }
            loop {
                self.skip_newlines();
                if self.eof() || self.eat("}") {
                    break;
                }
                let before = self.i;
                if let Some(c) = self.ui_node() {
                    children.push(c);
                }
                if self.i == before {
                    self.bump();
                }
            }
        }
        Some(UiNode { kind, args, children, span })
    }

    // ------------------------------------------------------------ expressions

    fn args(&mut self) -> Vec<Arg> {
        let mut out = Vec::new();
        self.expect("(");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat(")") {
                break;
            }
            if self.eat(",") {
                continue;
            }
            let span = self.peek().span;
            let name = if self.peek().kind == Kind::Ident && self.peek_at(1).is(":") {
                let n = self.bump().text;
                self.bump();
                Some(n)
            } else {
                None
            };
            let value = self.expr(0);
            out.push(Arg { name, value, span });
        }
        out
    }

    fn binding_power(op: &str) -> Option<u8> {
        Some(match op {
            "||" => 1,
            "&&" => 2,
            "==" | "!=" => 3,
            "<" | "<=" | ">" | ">=" => 4,
            "+" | "-" => 5,
            "*" | "/" | "%" => 6,
            _ => return None,
        })
    }

    fn expr(&mut self, min_bp: u8) -> Expr {
        self.skip_newlines();
        let mut lhs = self.unary();

        loop {
            if self.at("with") && self.peek_at(1).kind == Kind::Ident {
                let span = self.peek().span;
                self.bump();
                let policy = self.bump().text;
                lhs = Expr::With { subject: Box::new(lhs), policy, span };
                continue;
            }
            if self.at("exists") {
                let span = self.bump().span;
                lhs = Expr::Exists { subject: Box::new(lhs), span };
                continue;
            }
            if self.at("from") && self.peek_at(1).is("{") {
                let span = self.bump().span;
                self.bump();
                let mut policies = Vec::new();
                loop {
                    self.skip_newlines();
                    if self.eof() || self.eat("}") {
                        break;
                    }
                    if self.peek().kind == Kind::Ident {
                        policies.push(self.bump().text);
                    } else {
                        self.bump();
                    }
                }
                lhs = Expr::From { value: Box::new(lhs), policies, span };
                continue;
            }

            let op = self.peek().text.clone();
            let Some(bp) = Self::binding_power(&op) else { break };
            if bp < min_bp {
                break;
            }
            let span = self.bump().span;
            let rhs = self.expr(bp + 1);
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }

        if min_bp == 0 && self.at("?") {
            let span = self.bump().span;
            let then = self.expr(0);
            self.expect(":");
            let other = self.expr(0);
            return Expr::Ternary { cond: Box::new(lhs), then: Box::new(then), other: Box::new(other), span };
        }
        lhs
    }

    fn unary(&mut self) -> Expr {
        self.skip_newlines();
        if self.at("-") || self.at("!") {
            let t = self.bump();
            let rhs = self.unary();
            return Expr::Unary { op: t.text, rhs: Box::new(rhs), span: t.span };
        }
        let primary = self.primary();
        self.postfix(primary)
    }

    fn postfix(&mut self, mut base: Expr) -> Expr {
        loop {
            if self.at(".") && self.peek_at(1).kind == Kind::Ident {
                let span = self.bump().span;
                let name = self.bump().text;
                base = Expr::Member { obj: Box::new(base), name, span };
                continue;
            }
            if self.at("(") {
                let span = self.peek().span;
                let args = self.args();
                base = Expr::Call { callee: Box::new(base), args, span };
                continue;
            }
            if self.at("[") {
                let span = self.bump().span;
                let _index = self.expr(0);
                self.expect("]");
                self.diagnostics.push(Diagnostic::error(
                    span,
                    "a list has no index in this language. It is consumed by `map`, `filter`, `fold`, `any`, `all`, `count` and `first` — an index is where a bound stops being checkable, and the bound is what makes this total",
                ));
                base = Expr::Error { span };
                continue;
            }
            // `xs.fold(0) { sum, x -> … }` — a trailing block is one more argument.
            if self.at("{") && matches!(base, Expr::Call { .. }) {
                let lam = self.lambda();
                if let Expr::Call { callee, mut args, span } = base {
                    args.push(Arg { name: None, span: lam.span(), value: lam });
                    base = Expr::Call { callee, args, span };
                }
                continue;
            }
            break;
        }
        base
    }

    fn lambda(&mut self) -> Expr {
        let span = self.peek().span;
        self.expect("{");
        let mut params = Vec::new();
        let save = self.i;
        while self.peek().kind == Kind::Ident {
            params.push(self.bump().text);
            if !self.eat(",") {
                break;
            }
        }
        if !self.eat("->") {
            self.i = save;
            params.clear();
        }
        let body = self.expr(0);
        self.skip_newlines();
        self.expect("}");
        Expr::Lambda { params, body: Box::new(body), span }
    }

    fn primary(&mut self) -> Expr {
        self.skip_newlines();
        let t = self.peek().clone();

        match t.kind {
            Kind::Num => {
                self.bump();
                if t.text.contains('.') {
                    return Expr::Float { text: t.text, span: t.span };
                }
                let value = t.text.replace('_', "").parse().unwrap_or(0);
                Expr::Num { value, span: t.span }
            }
            Kind::Str => {
                self.bump();
                Expr::Str { value: t.text, span: t.span }
            }
            _ if t.is("(") => {
                self.bump();
                let inner = self.expr(0);
                self.expect(")");
                inner
            }
            _ if t.is("switch") => self.switch_expr(),
            _ if t.is("{") => self.record(),
            Kind::Ident if t.text == "true" || t.text == "false" => {
                self.bump();
                Expr::Bool { value: t.text == "true", span: t.span }
            }
            Kind::Ident => {
                self.bump();
                // `LoyaltyMember { … }` — a named record. Unambiguous only
                // because `if` and `switch` parenthesise their conditions.
                if self.at("{") && t.text.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                    let rec = self.record();
                    if let Expr::Record { spread, fields, span } = rec {
                        return Expr::Call {
                            callee: Box::new(Expr::Ident { name: t.text, span: t.span }),
                            args: vec![Arg { name: None, span, value: Expr::Record { spread, fields, span } }],
                            span: t.span,
                        };
                    }
                }
                Expr::Ident { name: t.text, span: t.span }
            }
            _ => {
                self.bump();
                self.diagnostics.push(Diagnostic::error(t.span, format!("`{}` does not start an expression", t.text)));
                Expr::Error { span: t.span }
            }
        }
    }

    fn record(&mut self) -> Expr {
        let span = self.peek().span;
        self.expect("{");
        let mut spread = None;
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            if self.eat("...") {
                spread = Some(Box::new(self.expr(0)));
            } else if self.peek().kind == Kind::Ident && self.peek_at(1).is(":") {
                let name = self.bump().text;
                self.bump();
                fields.push((name, self.expr(0)));
            } else {
                self.bump();
            }
            self.eat(",");
        }
        Expr::Record { spread, fields, span }
    }

    fn switch_expr(&mut self) -> Expr {
        let span = self.peek().span;
        self.bump();
        self.expect("(");
        let subject = self.expr(0);
        self.expect(")");
        self.expect("{");
        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            let aspan = self.peek().span;
            let pattern = if self.at("default") {
                self.bump();
                ArmPattern::Default
            } else if matches!(self.peek().text.as_str(), ">=" | ">" | "<=" | "<" | "==" | "!=") {
                let op = self.bump().text;
                ArmPattern::Compare { op, rhs: self.expr(0) }
            } else {
                ArmPattern::Value(self.expr(0))
            };
            self.expect("=>");
            let body = self.expr(0);
            self.eat(",");
            arms.push(SwitchArm { pattern, body, span: aspan });
        }
        Expr::Switch { subject: Box::new(subject), arms, span }
    }
}
