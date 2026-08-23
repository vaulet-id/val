//! Recursive descent for the shell, Pratt for expressions.
//!
//! The shell may invent; the expression layer may not (§2). That shows up here
//! as a large, boring set of block parsers over a small operator table.

use crate::ast::*;
use crate::diag::{Diagnostic, Span};

/// The language's own words. A dot is always field access and a keyword is
/// never a name, so that reading a declaration never depends on knowing what
/// else the package declared.
pub const RESERVED: &[&str] = &[
    // The shell.
    "app", "version", "capabilities", "admits", "enum", "credential", "type", "state",
    "trust", "anchor", "refines", "function", "action", "screen", "data",
    "input", "require", "verify", "compute", "update", "execute", "present",
    "component", "host",
    // Expressions.
    "const", "let", "var", "if", "else", "switch", "default", "return", "with",
    "exists", "from", "of", "as", "order", "by", "limit", "desc", "asc", "in",
    "for",
    // Effects written as syntax rather than as calls.
    "disclose", "prove",
    // Types.
    "string", "int", "bool", "date", "datetime", "bytes", "List", "Credential",
    "Verified", "Proof",
    // What crosses a package boundary.
    "export", "import",
];

/// Words nothing uses yet.
///
/// A package is signed, published, and then run by hosts on their own schedule,
/// so a word that turns into a keyword after the fact breaks a package its
/// author is no longer editing. Held before anything needs them, because
/// refusing them costs nothing today and refusing them later costs somebody a
/// build they cannot fix.
const HELD: &[&str] = &[];
use crate::lex::{Kind, Lexer, Token};

pub struct Parser {
    toks: Vec<Token>,
    i: usize,
    pub diagnostics: Vec<Diagnostic>,
    /// The blocks opened so far, closed as they end. What an editor asks
    /// instead of counting braces.
    pub scopes: Vec<Scope>,
    open: Vec<usize>,
}

pub fn parse(src: &str) -> (Program, Vec<Diagnostic>) {
    let (toks, lex_diags, comments, blanks) = Lexer::new(src).run();
    let mut p = Parser { toks, i: 0, diagnostics: lex_diags, scopes: Vec::new(), open: Vec::new() };
    let mut program = p.program();
    program.comments = comments;
    program.blank_lines = blanks;
    program.scopes = std::mem::take(&mut p.scopes);
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
    /// The token just consumed, for a diagnostic that has to reach back over
    /// what it read.
    fn previous(&self) -> &Token {
        self.toks.get(self.i.saturating_sub(1)).unwrap_or(self.peek())
    }

    /// A block starts here. Paired with `close`, and left open if the file ends
    /// first — which is what a program being typed looks like.
    fn open(&mut self, kind: ScopeKind, name: impl Into<String>) {
        let from = self.peek().span;
        self.scopes.push(Scope { kind, name: name.into(), from, to: Span { line: u32::MAX, col: 0, len: 0 } });
        self.open.push(self.scopes.len() - 1);
    }

    /// A block ends here — unless the file ended first.
    ///
    /// A block the author never closed runs to the end of the file, because
    /// that is where their cursor is: closing it at the last token read would
    /// put the position they are typing at outside every block, which is the
    /// one moment an editor most needs an answer. It is still an error, and
    /// reported once, at the brace that opened it.
    fn close(&mut self) {
        let Some(i) = self.open.pop() else { return };
        if self.eof() && !self.previous().is("}") {
            self.diagnostics.push(Diagnostic::error(
                self.scopes[i].from,
                "this block is never closed — the file ends inside it",
            ));
            return;
        }
        self.scopes[i].to = self.previous().span;
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
    /// `skip_newlines`, remembering whether it skipped any — for a list whose
    /// members may be separated by a line as well as by a comma.
    fn skip_newlines_marking(&mut self, seen: &mut bool) {
        while self.peek().kind == Kind::Newline {
            *seen = true;
            self.i += 1;
        }
    }

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
            let t = self.bump();
            let word = t.text.as_str();
            if RESERVED.contains(&word) {
                self.diagnostics.push(Diagnostic::error(
                    t.span,
                    format!("`{word}` is a keyword, and a keyword is never a name"),
                ));
            } else if HELD.contains(&word) {
                self.diagnostics.push(Diagnostic::error(
                    t.span,
                    format!(
                        "`{word}` is held for a feature this language does not have yet. A name that becomes a keyword later is a package that stops building on somebody else's release"
                    ),
                ));
            }
            t.text
        } else {
            let t = self.peek().clone();
            self.diagnostics.push(Diagnostic::error(t.span, format!("expected a name, found `{}`", t.text)));
            String::new()
        }
    }

    /// Consume the rest of a line, for recovery inside a block whose lines are
    /// independent — one bad line should not take the others with it.
    fn skip_line(&mut self) {
        while !self.eof() && self.peek().kind != Kind::Newline && !self.at("}") {
            self.bump();
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
            let directives = self.directives();

            let t = self.peek().clone();
            for d in &directives {
                if t.text != "screen" {
                    self.diagnostics.push(Diagnostic::error(
                        d.span,
                        format!(
                            "`@{}` marks a screen, and `{}` is not one",
                            d.name, t.text
                        ),
                    ));
                }
            }
            match t.text.as_str() {
                "app" => {
                    let at = self.peek().span;
                    self.bump();
                    self.skip_newlines();
                    if self.peek().kind == Kind::Str {
                        let name = self.bump().text;
                        if let Some(first) = &p.app {
                            if *first != name {
                                self.diagnostics.push(Diagnostic::error(
                                    at,
                                    format!("this package already calls itself `{first}`, and this file calls it `{name}`"),
                                ));
                            }
                        }
                        p.app = Some(name);
                        p.app_span = at;
                    } else {
                        let bad = self.bump();
                        self.diagnostics.push(Diagnostic::error(
                            bad.span,
                            "the application identifier is a quoted string: a reverse-DNS name and a field access are the same shape",
                        ));
                    }
                }
                "version" => {
                    p.version_span = self.peek().span;
                    self.bump();
                    // **A version is a version, not a counter.** `7` on an
                    // install sheet is a number a person cannot place: the
                    // seventh what, and is the one they hold older or newer? A
                    // quoted `"1.2.0"` reads as a release, sorts the way every
                    // other piece of software sorts, and leaves room to say a
                    // change was small.
                    let token = self.bump();
                    p.version = Some(token.text.clone());
                    let looks_like_a_release = token.text.split('.').count() == 3
                        && token.text.split('.').all(|part| {
                            !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())
                        });
                    if !looks_like_a_release {
                        self.diagnostics.push(Diagnostic::error(
                            token.span,
                            "a version is three numbers in quotes, as `version \"1.0.0\"` — a bare counter tells a person nothing about whether what they hold is older than what they are being offered",
                        ));
                    }
                }
                "capabilities" => {
                    let at = self.peek().span;
                    self.bump();
                    self.open(ScopeKind::Capabilities, "capabilities");
                    let more = self.capabilities();
                    self.close();
                    // One block per package. A package is several files sharing
                    // one scope, and "which file says what this app may do" is
                    // exactly the question that has to have one answer —
                    // merging them quietly would answer it in whatever order
                    // the files were read.
                    if p.capabilities.is_empty() {
                        p.capabilities = more;
                        p.capabilities_span = at;
                    } else {
                        self.diagnostics.push(Diagnostic::error(
                            at,
                            "a package declares its capabilities once. This is the second block, and a person consented to a list rather than to a sum of lists",
                        ));
                    }
                }
                "admits" => {
                    let at = self.peek().span;
                    self.bump();
                    self.open(ScopeKind::Capabilities, "admits");
                    let more = self.admits();
                    self.close();
                    // One block, for the same reason `capabilities` is one: a
                    // door with two lists of who may come through has two
                    // answers to one question.
                    if p.admits.is_empty() {
                        p.admits = more;
                        p.admits_span = at;
                    } else {
                        self.diagnostics.push(Diagnostic::error(
                            at,
                            "a package says who it opens for once. This is the second block, and a door has one list",
                        ));
                    }
                }
                "enum" => p.enums.push(self.enum_decl()),
                "credential" => p.credentials.push(self.credential_decl()),
                "type" => {
                    let mut t = self.credential_decl();
                    t.span = self.peek().span;
                    p.types.push(t);
                }
                "state" => {
                    p.state_span = self.peek().span;
                    self.bump();
                    p.state = self.fields();
                }
                "trust" => p.trusts.push(self.trust_decl()),
                "function" => p.functions.push(self.function_decl()),
                "action" => p.actions.push(self.action_decl()),
                "screen" => {
                    let mut s = self.screen_decl();
                    s.directives = directives;
                    p.screens.push(s);
                }
                "component" => p.components.push(self.component_decl(false)),
                "export" => {
                    let at = self.peek().span;
                    self.bump();
                    if self.at("component") {
                        p.components.push(self.component_decl(true));
                    } else {
                        let bad = self.peek().clone();
                        self.diagnostics.push(Diagnostic::error(
                            at,
                            format!("`export` marks a component, and `{}` is not one. What leaves a package is a way of arranging the host's catalogue — state, actions and credentials belong to the package that declared them", bad.text),
                        ));
                        self.skip_block();
                    }
                }
                "import" => p.imports.push(self.import_decl()),
                "host" => {
                    self.bump();
                    self.skip_newlines();
                    if self.peek().kind == Kind::Str {
                        p.hosts.push(self.bump().text);
                    } else {
                        let at = self.peek().span;
                        self.diagnostics.push(Diagnostic::error(
                            at,
                            "a host is named and versioned: `host \"id.vaulet.wallet/1\"`"
                                .to_string(),
                        ));
                    }
                }
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

    /// `EmployeeBadge with EmployeeOfAcme else "needBadge"`, one per line.
    ///
    /// Every part is required. The policy, because a gate that accepted
    /// anything shaped like a badge accepts a badge somebody made; the phrase,
    /// because a door that closes without saying why leaves the person with a
    /// fault report instead of an instruction.
    fn admits(&mut self) -> Vec<Admit> {
        let mut out = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            let start = self.peek().span;
            if self.peek().kind != Kind::Ident {
                self.diagnostics.push(Diagnostic::error(
                    start,
                    "this block names a credential, so a line begins with one",
                ));
                self.bump();
                continue;
            }
            let credential = self.bump().text;
            if !self.eat("with") {
                self.diagnostics.push(Diagnostic::error(
                    self.peek().span,
                    format!("`{credential}` has to be checked against a policy: `{credential} with SomePolicy`. A gate that accepted anything of the right shape accepts one somebody made"),
                ));
                self.skip_line();
                continue;
            }
            let policy = if self.peek().kind == Kind::Ident {
                self.bump().text
            } else {
                self.diagnostics.push(Diagnostic::error(
                    self.peek().span,
                    "`with` names a `trust` policy declared in this package",
                ));
                self.skip_line();
                continue;
            };
            if !self.eat("else") {
                self.diagnostics.push(Diagnostic::error(
                    self.peek().span,
                    format!("say what the person is told when they hold no {credential}: `else \"someKey\"`, a key in the text bundle. A door that closes without saying why is a fault report"),
                ));
                self.skip_line();
                continue;
            }
            let end = self.peek().span;
            let phrase = if self.peek().kind == Kind::Str {
                self.bump().text
            } else {
                self.diagnostics.push(Diagnostic::error(
                    end,
                    "`else` names a key in the text bundle, in quotes — never a sentence written here, which is a sentence in one language that nobody reviewed",
                ));
                self.skip_line();
                continue;
            };
            out.push(Admit { credential, policy, phrase, span: start.to(end) });
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

    /// `enum Tier { bronze, silver, gold }`
    ///
    /// Separated, because two names side by side is a comma somebody forgot and
    /// reading it as two members is the compiler deciding what they meant.
    fn enum_decl(&mut self) -> EnumDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        self.open(ScopeKind::Enum, name.clone());
        let mut members: Vec<String> = Vec::new();
        self.expect("{");
        // A comma or a line between members, and nothing else: two names side
        // by side is a comma somebody forgot, and reading it as two members is
        // the compiler deciding what they meant.
        let mut separated = true;
        loop {
            self.skip_newlines_marking(&mut separated);
            if self.eof() || self.eat("}") {
                break;
            }
            if self.eat(",") {
                separated = true;
                continue;
            }
            if self.peek().kind == Kind::Ident {
                let at = self.peek().span;
                let word = self.bump().text;
                if !separated {
                    self.diagnostics.push(Diagnostic::error(
                        at,
                        format!("`{}` and `{word}` need a comma or a line between them", members.last().cloned().unwrap_or_default()),
                    ));
                }
                members.push(word);
                separated = false;
            } else {
                self.bump();
            }
        }
        self.close();
        EnumDecl { name, members, span }
    }

    /// `credential EmployeeBadge as "https://…/credential/employee-badge" { … }`
    ///
    /// A `type` takes the same shape without the `as`: it is a record nobody
    /// signed, so there is no card in anybody's wallet for it to be.
    fn credential_decl(&mut self) -> CredentialDecl {
        let span = self.peek().span;
        let is_credential = self.peek().text == "credential";
        self.bump();
        let name = self.ident();
        let mut vct = String::new();
        if self.at("as") {
            let at = self.peek().span;
            self.bump();
            if self.peek().kind == Kind::Str {
                vct = self.bump().text;
            } else {
                self.diagnostics.push(Diagnostic::error(
                    at,
                    "`as` names the credential type a wallet knows this by, in quotes — `as \"https://your-domain.example/credential/employee-badge\"`, on whatever domain issues it",
                ));
            }
            if !is_credential {
                self.diagnostics.push(Diagnostic::error(
                    at,
                    format!("`{name}` is a record and not a credential, so there is no card in anybody's wallet for it to be. Declare it as `credential` if that is what it is"),
                ));
            }
        }
        let fields = self.fields();
        CredentialDecl { name, vct, fields, span }
    }

    fn fields(&mut self) -> Vec<Field> {
        self.open(ScopeKind::Fields, "");
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
            if ty.name.is_empty() {
                // `name:` and then nothing that is a type. **The field is not
                // recorded**, for the same reason a nameless type argument is
                // not: it prints as a name, a colon and nothing, and reading
                // that back takes the next line's name as this one's type. A
                // printer whose output does not parse to what it printed is a
                // printer nothing else can be checked against.
                self.diagnostics.push(Diagnostic::error(
                    span,
                    format!("`{name}` has no type. Every field of a credential, a record and of `state` says what it holds"),
                ));
                continue;
            }
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
        self.close();
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
                // A type that is not a name consumes nothing, so without this
                // the loop spins on it forever. `List(int)` — the wrong bracket
                // — hung the compiler rather than reporting anything, and in
                // the editor that is a tab that stops responding.
                let before = self.i;
                let arg = self.type_ref();
                if self.i == before {
                    // Nothing there to be a type. Skipping the token is the
                    // recovery; **keeping the argument is not** — an argument
                    // with no name prints as nothing between two commas, and
                    // reading that back gives a shorter list than the one
                    // printed. A printer whose output does not parse to what it
                    // printed is a printer nothing else can be checked against.
                    self.bump();
                    continue;
                }
                args.push(arg);
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

        self.open(ScopeKind::Trust, name.clone());
        let mut anchor = None;
        let mut anchor_span = Span::default();
        let mut requires = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            if self.at("anchor") {
                anchor_span = self.peek().span;
                self.bump();
                if self.eat(":") {
                    self.skip_newlines();
                    // Quoted, like every other external name in this language:
                    // `app`, `host` and `import` all name something outside the
                    // package with a string. Written bare it was read as one
                    // token and `shop.example.com` became `shop` — and an
                    // anchor is the root a policy trusts, which is the one
                    // string here that may not be approximated.
                    if self.peek().kind == Kind::Str {
                        anchor = Some(self.bump().text);
                    } else {
                        let at = self.peek().span;
                        let written = self.dotted();
                        let at = at.to(self.previous().span);
                        self.diagnostics.push(Diagnostic::error(
                            at,
                            format!("an anchor is quoted: `anchor: \"{written}\"`. It names something outside this package, as `app`, `host` and `import` do"),
                        ));
                        anchor = Some(written);
                    }
                } else if self.at("{") {
                    let bad = self.peek().span;
                    self.diagnostics.push(Diagnostic::error(bad, "`anchor:` is a field, not a block"));
                    self.skip_block();
                }
                continue;
            }
            if self.at("require") {
                self.open(ScopeKind::Requires, "require");
                self.bump();
                self.expect("{");
                loop {
                    self.skip_newlines();
                    if self.eof() || self.eat("}") {
                        break;
                    }
                    requires.push(self.expr(0));
                }
                self.close();
                continue;
            }
            self.bump();
        }
        self.close();
        TrustDecl { name, subject, subject_type, refines, anchor, anchor_span, requires, span }
    }

    fn function_decl(&mut self) -> FunctionDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        let params = self.param_list();
        let ret = if self.eat(":") { Some(self.type_ref()) } else { None };
        self.open(ScopeKind::Function, name.clone());
        let body = self.stmt_block();
        self.close();
        FunctionDecl { name, params, ret, body, span }
    }

    /// `(name: type, …)`, or nothing. Shared by functions and components so the
    /// two cannot drift into accepting different parameter lists.
    fn param_list(&mut self) -> Vec<Field> {
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
                let before = self.i;
                let pname = self.ident();
                self.eat(":");
                let ty = self.type_ref();
                // `note: string default "none"` — the same word a state field
                // uses, because it is the same thing: a value for when nobody
                // supplied one.
                let default = if self.at("default") {
                    self.bump();
                    Some(self.expr(0))
                } else {
                    None
                };
                params.push(Field { name: pname, ty, default, span: pspan });
                // Neither a name nor a type consumes anything when it is
                // neither, and a parameter list that spins is a hang rather
                // than a message.
                if self.i == before {
                    self.bump();
                }
            }
        }
        params
    }

    fn action_decl(&mut self) -> ActionDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        self.open(ScopeKind::Action, name.clone());
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
        self.close();
        ActionDecl { name, phases, span }
    }

    fn phase_stmts(&mut self, phase: Phase) -> Vec<Stmt> {
        self.open(ScopeKind::Phase, phase.name());
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
        self.close();
        out
    }

    fn stmt_block(&mut self) -> Vec<Stmt> {
        self.open(ScopeKind::Statements, "");
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
        self.close();
        out
    }

    fn stmt(&mut self, phase: Option<Phase>) -> Option<Stmt> {
        self.skip_newlines();
        let span = self.peek().span;

        if self.at("const") || self.at("let") {
            let mutable = self.at("let");
            self.bump();

            // `const { merchant, amount } = row` — the fields, by their own
            // names. One statement, so the right-hand side is read once.
            if self.at("{") {
                self.bump();
                let mut names = Vec::new();
                loop {
                    self.skip_newlines();
                    if self.eof() || self.eat("}") {
                        break;
                    }
                    if self.eat(",") {
                        continue;
                    }
                    if self.peek().kind == Kind::Ident {
                        names.push(self.bump().text);
                    } else {
                        let bad = self.bump();
                        self.diagnostics.push(Diagnostic::error(
                            bad.span,
                            format!("`{}` is not a field name", bad.text),
                        ));
                    }
                }
                self.expect("=");
                let value = self.expr(0);
                if names.is_empty() {
                    self.diagnostics.push(Diagnostic::error(
                        span,
                        "this takes nothing out of the record. Name the fields you want",
                    ));
                }
                return Some(Stmt::Destructure { names, value, mutable, span });
            }

            let name = self.ident();
            self.expect("=");
            // An action may re-declare the data a screen declared, because it
            // can be reached from somewhere other than that screen.
            if self.at("credentials") || self.at("query") {
                let source = self.data_source();
                return Some(Stmt::Data { name, source, span });
            }
            let value = self.expr(0);
            return Some(Stmt::Let { name, value, mutable, span });
        }
        if self.at("var") {
            self.bump();
            self.diagnostics.push(Diagnostic::error(
                span,
                "a variable is `let`; there is no `var`".to_string(),
            ));
            let name = self.ident();
            self.expect("=");
            let value = self.expr(0);
            return Some(Stmt::Let { name, value, mutable: true, span });
        }

        // `x = …`. A name on its own followed by `=` is an assignment and
        // nothing else — `=` appears in no expression this language has.
        if self.peek().kind == Kind::Ident && self.peek_at(1).is("=") {
            let name = self.bump().text;
            self.bump();
            let value = self.expr(0);
            return Some(Stmt::Assign { name, value, span });
        }
        if self.at("refuse") {
            self.bump();
            self.skip_newlines();
            let t = self.peek().clone();
            if t.kind != Kind::Str {
                self.diagnostics.push(Diagnostic::error(
                    t.span,
                    "`refuse` names a key in the text bundle. A sentence assembled here is a sentence nobody signed, and this one is read by the person the application is declining",
                ));
                return Some(Stmt::Refuse { key: String::new(), span });
            }
            self.bump();
            return Some(Stmt::Refuse { key: t.text, span });
        }
        if self.at("if") {
            self.bump();
            // Parenthesised, which is what removes every ambiguity between a
            // block and a record literal (§2).
            self.expect("(");
            let cond = self.expr(0);
            self.expect(")");
            let then = self.stmt_block();
            let mut other = Vec::new();
            self.skip_newlines();
            if self.eat("else") {
                other = if self.at("if") {
                    self.stmt(phase).into_iter().collect()
                } else {
                    self.stmt_block()
                };
            }
            return Some(Stmt::If { cond, then, other, span });
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
            // `navigate Home` — an effect the host performs, and the only one
            // that changes what somebody is looking at.
            if name == "navigate" {
                let args = if self.at("(") {
                    self.args()
                } else {
                    let span = self.peek().span;
                    let value = self.expr(0);
                    vec![Arg { name: None, value, spread: false, span }]
                };
                return Some(Stmt::Effect { name, args, body: Vec::new(), span });
            }
            // `disclose x` / `prove x` inside a `present` block.
            if name == "disclose" || name == "prove" {
                let value = self.expr(0);
                return Some(Stmt::Effect {
                    name,
                    args: vec![Arg { name: None, value, spread: false, span }],
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

    /// `@main`, `@name(argument)` — zero or more, each on its own line above a
    /// declaration.
    ///
    /// The set is closed and lives here rather than in a host's registry: a
    /// directive says which screen a *package* opens at, which is a fact about
    /// the package and the same in every wallet that runs it.
    fn directives(&mut self) -> Vec<Directive> {
        // name, and how many arguments it takes. Arguments are parsed for every
        // directive, so the first one that needs an argument is a row here
        // rather than a second syntax bolted on beside this one.
        const KNOWN: &[(&str, usize)] = &[("main", 0)];

        let mut out = Vec::new();
        while self.at("@") {
            let span = self.peek().span;
            self.bump();
            if self.peek().kind != Kind::Ident {
                let bad = self.bump();
                self.diagnostics.push(Diagnostic::error(
                    span,
                    format!("`@` introduces a directive, and `{}` is not a name", bad.text),
                ));
                continue;
            }
            let name = self.bump().text;
            let args = if self.at("(") { self.args() } else { Vec::new() };

            match KNOWN.iter().find(|(k, _)| *k == name) {
                None => self.diagnostics.push(Diagnostic::error(
                    span,
                    format!(
                        "`@{name}` is not a directive this language has. They are: {}",
                        KNOWN.iter().map(|(k, _)| format!("`@{k}`")).collect::<Vec<_>>().join(", ")
                    ),
                )),
                Some((_, arity)) if args.len() != *arity => {
                    self.diagnostics.push(Diagnostic::error(
                        span,
                        if *arity == 0 {
                            format!("`@{name}` marks a declaration and takes nothing")
                        } else {
                            format!("`@{name}` takes {arity} argument(s), and this gives {}", args.len())
                        },
                    ));
                }
                Some(_) => {}
            }

            out.push(Directive { name, args, span });
            self.skip_newlines();
        }
        out
    }

    fn screen_decl(&mut self) -> ScreenDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        self.open(ScopeKind::Screen, name.clone());
        let params = self.param_list();

        // Settings sit between the name and the body: `screen Confirm(x: int)
        // present: sheet { … }`. The parser knows the shape and none of the
        // words. Which screen opens the application is not one of them — that
        // is `@main`, above the declaration, because it is a fact about the
        // application rather than a property of the screen.
        let mut settings = Vec::new();
        while self.peek().kind == Kind::Ident && self.peek_at(1).is(":") && !self.at("title") {
            let at = self.peek().span;
            let name = self.bump().text;
            self.bump();
            let value = self.expr(0);
            settings.push(Arg { name: Some(name), value, spread: false, span: at });
            self.skip_newlines();
        }

        let mut data = Vec::new();
        let mut compute = Vec::new();
        let mut tree = Vec::new();
        let mut title = None;
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            if self.at("title") && self.peek_at(1).is(":") {
                let at = self.peek().span;
                self.bump();
                self.bump();
                let value = self.expr(0);
                title = Some(UiNode {
                    kind: "title".to_string(),
                    args: vec![Arg { name: Some("text".into()), value, spread: false, span: at }],
                    lambda: None,
                    children: Vec::new(),
                    slots: Vec::new(),
                    otherwise: Vec::new(),
                    span: at,
                });
                continue;
            }
            if self.at("data") && self.peek_at(1).is("{") {
                self.bump();
                data = self.data_block();
                continue;
            }
            if self.at("compute") && self.peek_at(1).is("{") {
                self.open(ScopeKind::Compute, "compute");
                self.bump();
                compute = self.stmt_block();
                self.close();
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
        self.close();
        ScreenDecl { name, directives: Vec::new(), params, settings, title, data, compute, tree, span }
    }

    /// `component Name(param: type, …) { …catalogue… }`
    ///
    /// No `data` and no `compute`: a component draws what it is handed. A
    /// component that could declare data of its own would be a screen, and the
    /// rule that a screen resolves its data before anything is drawn would then
    /// hold for only some of what is on it.
    /// `import "org.vaulet.ui/1.0.0" { MoneyCard, Chip }`
    fn import_decl(&mut self) -> ImportDecl {
        let span = self.peek().span;
        self.bump();
        self.skip_newlines();

        let package = if self.peek().kind == Kind::Str {
            self.bump().text
        } else {
            let bad = self.peek().clone();
            self.diagnostics.push(Diagnostic::error(
                bad.span,
                "a package is imported by name and version: `import \"org.vaulet.ui/1.0.0\" { … }`. A reverse-DNS name and a field access are the same shape, so the name is quoted".to_string(),
            ));
            String::new()
        };

        // Listed rather than opened wholesale. What crossed into this package is
        // then one line to read — and everything that crossed is signed as part
        // of it.
        let mut names = Vec::new();
        self.skip_newlines();
        if self.eat("{") {
            loop {
                self.skip_newlines();
                if self.eof() || self.eat("}") {
                    break;
                }
                if self.eat(",") {
                    continue;
                }
                if self.peek().kind == Kind::Ident {
                    names.push(self.bump().text);
                } else {
                    self.bump();
                }
            }
        } else {
            self.diagnostics.push(Diagnostic::error(
                span,
                "an import lists what it takes: `import \"org.vaulet.ui/1.0.0\" { MoneyCard }`. A package that opened all of another one would carry names nobody wrote".to_string(),
            ));
        }

        ImportDecl { package, names, span }
    }

    fn component_decl(&mut self, exported: bool) -> ComponentDecl {
        let span = self.peek().span;
        self.bump();
        let name = self.ident();
        self.open(ScopeKind::Component, name.clone());
        let params = self.param_list();

        let mut tree = Vec::new();
        self.expect("{");
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            for word in ["data", "compute"] {
                if self.at(word) && self.peek_at(1).is("{") {
                    let bad = self.peek().span;
                    self.diagnostics.push(Diagnostic::error(
                        bad,
                        format!("a component has no `{word}` block — it draws what it is handed"),
                    ));
                }
            }
            let before = self.i;
            if let Some(n) = self.ui_node() {
                tree.push(n);
            }
            if self.i == before {
                self.bump();
            }
        }
        self.close();
        ComponentDecl { name, exported, params, tree, span }
    }

    fn data_block(&mut self) -> Vec<DataDecl> {
        self.open(ScopeKind::ScreenData, "data");
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
        self.close();
        out
    }

    fn data_source(&mut self) -> DataSource {
        self.skip_newlines();
        if self.at("credentials") {
                self.bump();
                self.eat("of");
                let ty = self.ident();
                let mut policy = None;
                let mut order = None;
                let mut limit = None;
                loop {
                    // `verified with`, `order by` and `limit` are usually
                    // written on their own lines under the declaration they
                    // belong to. A newline separates statements everywhere
                    // else, so it is stepped over here and put back when what
                    // follows turns out to be the next statement.
                    let before = self.i;
                    self.skip_newlines();
                    if !(self.at("verified") || self.at("order") || self.at("limit")) {
                        self.i = before;
                        break;
                    }
                    if self.at("verified") {
                        self.bump();
                        self.eat("with");
                        policy = Some(self.ident());
                    } else if self.at("order") {
                        self.bump();
                        self.eat("by");
                        let claim = self.ident();
                        let descending = self.at("desc");
                        self.eat("desc");
                        self.eat("asc");
                        order = Some((claim, descending));
                    } else if self.at("limit") {
                        self.bump();
                        limit = self.bump().text.replace('_', "").parse().ok();
                    } else {
                        break;
                    }
                }
                DataSource::Credentials { ty, policy, order, limit }
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
                // Anything else is discarded, so it cannot be checked, printed
                // or reported. A screen declares where its data comes from and
                // there are two answers; a third was read and thrown away.
                let at = self.peek().span;
                self.expr(0);
                self.diagnostics.push(Diagnostic::error(
                    at,
                    "a screen's data comes from `credentials of …` or from `query …`. The host resolves it before anything is drawn, which is why it is declared rather than fetched",
                ));
                DataSource::Unknown
            }
    }

    fn ui_node(&mut self) -> Option<UiNode> {
        self.skip_newlines();
        if self.peek().kind != Kind::Ident {
            return None;
        }
        let span = self.peek().span;

        // `if (state.member exists) { … } else { … }` — a screen that shows one
        // thing or the other. The condition is settled while the screen is being
        // resolved, before anything is drawn, so what a host receives is a tree
        // with no condition left in it.
        if self.at("if") && self.peek_at(1).is("(") {
            self.bump();
            self.expect("(");
            let cond = self.expr(0);
            self.expect(")");
            let children = self.ui_block();
            let save = self.i;
            self.skip_newlines();
            let otherwise = if self.eat("else") {
                self.ui_block()
            } else {
                // A newline that was skipped looking for `else` belongs to
                // whatever comes next, which may be another node.
                self.i = save;
                Vec::new()
            };
            return Some(UiNode {
                kind: "if".to_string(),
                args: vec![Arg { name: None, value: cond, spread: false, span }],
                lambda: None,
                children,
                slots: Vec::new(),
                otherwise,
                span,
            });
        }

        // `for (r in rows) { … }` — the body once per item, and nothing drawn
        // around them. `list(rows) { r -> … }` is the wallet's list, with its
        // separators and its empty state; this is repetition and no more.
        if self.at("for") && self.peek_at(1).is("(") {
            self.bump();
            self.expect("(");
            let bind = self.ident();
            if !self.eat("in") {
                let bad = self.peek().clone();
                self.diagnostics.push(Diagnostic::error(
                    bad.span,
                    format!("a loop reads `for (row in rows)`, and this says `{}`", bad.text),
                ));
                // Eaten, so that what follows is read as the list rather than
                // as a syntax error three tokens later. One mistake that spills
                // messages down the line buries the one that taught the rule.
                if bad.kind == Kind::Ident {
                    self.bump();
                }
            }
            let over = self.expr(0);
            self.expect(")");
            let children = self.ui_block();
            return Some(UiNode {
                kind: "for".to_string(),
                args: vec![Arg { name: None, value: over, spread: false, span }],
                lambda: Some(bind),
                children,
                slots: Vec::new(),
                otherwise: Vec::new(),
                span,
            });
        }

        // `wallet.avatar` — a host's own component, written under the host's
        // name. One token would have made it two nodes, the second of which is
        // not a component anybody declared.
        let mut kind = self.bump().text;
        while self.at(".") && self.peek_at(1).kind == Kind::Ident {
            self.bump();
            kind.push('.');
            kind.push_str(&self.bump().text);
        }
        let mut args = if self.at("(") { self.args() } else { Vec::new() };
        let mut children = Vec::new();
        let mut lambda = None;
        let opened = self.at("{");
        if self.at("{") {
            self.open(ScopeKind::Node, kind.clone());
            self.bump();
            // `list(xs) { r -> … }`, and the same written over two lines. A
            // brace emits a newline now, so without stepping over it the binder
            // was not seen — and the row a list is drawn from lost its name
            // silently, which is worse than not reading the form at all.
            let save = self.i;
            self.skip_newlines();
            if !(self.peek().kind == Kind::Ident && self.peek_at(1).is("->")) {
                self.i = save;
            }
            if self.peek().kind == Kind::Ident && self.peek_at(1).is("->") {
                lambda = Some(self.bump().text);
                self.bump();
            }
            loop {
                self.skip_newlines();
                if self.eof() || self.eat("}") {
                    break;
                }
                // A block holds both: `name: value` is a prop of this node, and
                // anything else is a child. One block rather than a bracket for
                // props and a brace for children, because a screen is read far
                // more often than it is written and two grouping characters on
                // one line is one more thing to hold.
                if self.peek().kind == Kind::Ident && self.peek_at(1).is(":") {
                    let at = self.peek().span;
                    let name = self.bump().text;
                    self.bump();
                    let value = self.expr(0);
                    args.push(Arg { name: Some(name), value, spread: false, span: at });
                    self.eat(",");
                    continue;
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
        if opened {
            self.close();
        }
        Some(UiNode { kind, args, lambda, children, slots: Vec::new(), otherwise: Vec::new(), span })
    }

    /// `{ …nodes… }` — the branches of an `if`, which hold children only.
    fn ui_block(&mut self) -> Vec<UiNode> {
        self.open(ScopeKind::Tree, "");
        let mut out = Vec::new();
        self.skip_newlines();
        if !self.eat("{") {
            self.close();
            return out;
        }
        loop {
            self.skip_newlines();
            if self.eof() || self.eat("}") {
                break;
            }
            let before = self.i;
            if let Some(n) = self.ui_node() {
                out.push(n);
            }
            if self.i == before {
                self.bump();
            }
        }
        self.close();
        out
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
            if self.eat("...") {
                let value = self.expr(0);
                out.push(Arg { name: None, value, spread: true, span });
                continue;
            }
            let name = if self.peek().kind == Kind::Ident && self.peek_at(1).is(":") {
                let n = self.bump().text;
                self.bump();
                Some(n)
            } else {
                None
            };
            let value = self.expr(0);
            out.push(Arg { name, value, spread: false, span });
        }
        out
    }

    fn binding_power(op: &str) -> Option<u8> {
        binding_power_of(op)
    }
}

/// How tightly an operator binds. Public because the printer parenthesises by
/// it: two copies of this table is how a printer and a parser come to disagree
/// about what a line means.
pub fn binding_power_of(op: &str) -> Option<u8> {
    Some(match op {
        "||" => 1,
        "&&" => 2,
        // `0...10`. Below the comparisons so that `0...n` reads as the range of
        // `n`, and above them nothing — a range of a comparison is not a thing
        // anybody means.
        "..." => 3,
        "==" | "!=" => 4,
        "<" | "<=" | ">" | ">=" => 5,
        "+" | "-" => 6,
        "*" | "/" | "%" => 7,
        _ => return None,
    })
}

impl Parser {
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

        // `a ?: b`. Before the ternary, which is the same first token.
        if min_bp == 0 && self.at("?") && self.peek_at(1).is(":") {
            let span = self.bump().span;
            self.bump();
            let other = self.expr(0);
            return Expr::Elvis { subject: Box::new(lhs), other: Box::new(other), span };
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
            // `a.b`, and `a?.b` — the whole path is nothing when the left
            // side is. Written as one token pair rather than as a check the
            // author repeats, because the check somebody forgets is the one
            // that matters.
            let optional = self.at("?") && self.peek_at(1).is(".") && self.peek_at(2).kind == Kind::Ident;
            if optional {
                self.bump();
            }
            if self.at(".") && self.peek_at(1).kind == Kind::Ident {
                self.bump();
                let at = self.peek().span;
                let name = self.bump().text;
                // From the start of what is being read to the end of the field
                // being read out of it. The dot's own position is where the
                // punctuation is, not where the path is.
                let span = base.span().to(at);
                base = Expr::Member { obj: Box::new(base), name, optional, span };
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
            // `xs.fold(0) { sum, x -> … }` — a trailing block is one more
            // argument. `xs.map { x -> … }` too: a combinator that takes only a
            // lambda has no parentheses to put it after, and requiring `map()`
            // would be punctuation for the parser's benefit.
            if self.at("{")
                && self.peek_at(1).kind == Kind::Ident
                && (self.peek_at(2).is("->") || self.peek_at(2).is(","))
                && matches!(base, Expr::Call { .. } | Expr::Member { .. })
            {
                let lam = self.lambda();
                base = match base {
                    Expr::Call { callee, mut args, span } => {
                        args.push(Arg { name: None, spread: false, span: lam.span(), value: lam });
                        Expr::Call { callee, args, span }
                    }
                    member => {
                        let span = member.span();
                        Expr::Call {
                            callee: Box::new(member),
                            args: vec![Arg { name: None, spread: false, span: lam.span(), value: lam }],
                            span,
                        }
                    }
                };
                continue;
            }
            break;
        }
        base
    }

    /// Whether the block starting at the cursor has a `->` at its own level.
    fn block_holds_an_arrow(&self) -> bool {
        let mut depth = 0i32;
        let mut k = 0usize;
        loop {
            let t = self.peek_at(k);
            if t.kind == Kind::Eof {
                return false;
            }
            if t.is("{") || t.is("(") || t.is("[") {
                depth += 1;
            } else if t.is("}") || t.is(")") || t.is("]") {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            } else if depth == 1 && t.is("->") {
                return true;
            }
            k += 1;
        }
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

        if t.kind == Kind::Template {
            self.bump();
            return self.template(&t);
        }

        // `["merchant", "amount"]` — a list written out. A list still has no
        // index; this is how one is said, not a way to reach into it.
        if t.is("[") {
            self.bump();
            let mut items = Vec::new();
            loop {
                self.skip_newlines();
                if self.eof() || self.eat("]") {
                    break;
                }
                if self.eat(",") {
                    continue;
                }
                let before = self.i;
                items.push(self.expr(0));
                if self.i == before {
                    self.bump();
                }
            }
            return Expr::List { items, span: t.span };
        }

        match t.kind {
            Kind::Num => {
                self.bump();
                if t.text.contains('.') {
                    return Expr::Float { text: t.text, span: t.span };
                }
                let value = t.text.replace('_', "").parse().unwrap_or(0);
                Expr::Num { value, text: t.text, span: t.span }
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
            // `{ sum, h -> … }` in an argument position is a function, not a
            // record. Told apart by looking for a `->` before the brace closes:
            // a record's contents are `name: value` and a function's are not,
            // and reading one as the other produced an empty record and no
            // complaint.
            _ if t.is("{") && self.block_holds_an_arrow() => self.lambda(),
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
                            args: vec![Arg { name: None, spread: false, span, value: Expr::Record { spread, fields, span } }],
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
        self.open(ScopeKind::Record, "");
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
        self.close();
        Expr::Record { spread, fields, span }
    }

    /// `` `you have ${state.points} points` `` becomes
    /// `phrase("you have {points} points", points: state.points)`.
    ///
    /// Sugar, and deliberately nothing more: the template and its values travel
    /// to the host separately so that the host formats the number — Thai
    /// digits, the thousands separator, the currency position — for every
    /// application at once. A template that joined them here would format them
    /// in the application, differently in each one.
    fn template(&mut self, t: &Token) -> Expr {
        let span = t.span;
        let mut words = String::new();
        let mut args: Vec<Arg> = Vec::new();
        let raw = t.text.clone();
        let bytes = raw.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] == b'$' && bytes.get(i + 1) == Some(&b'{') {
                let mut depth = 1;
                let mut j = i + 2;
                // A brace inside a string is not a brace. `${ f("}") }` ended
                // the interpolation at the wrong place and the rest of the
                // template was read as source.
                let mut in_string = false;
                while j < bytes.len() && depth > 0 {
                    match bytes[j] {
                        b'"' => in_string = !in_string,
                        b'\\' if in_string => j += 1,
                        b'{' if !in_string => depth += 1,
                        b'}' if !in_string => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                if depth != 0 {
                    self.diagnostics.push(Diagnostic::error(span, "a `${` in this string is never closed"));
                    break;
                }
                let inner = &raw[i + 2..j - 1];
                let name = slot_name(inner, &args);
                words.push('{');
                words.push_str(&name);
                words.push('}');

                // The expression is parsed by this same parser, so `${a ?: b}`
                // and `${xs.count()}` mean in a string exactly what they mean
                // outside one.
                let (value, mut d) = parse_expr(inner, span);
                self.diagnostics.append(&mut d);
                args.push(Arg { name: Some(name), value, spread: false, span });
                i = j;
                continue;
            }
            let ch = raw[i..].chars().next().unwrap_or('\0');
            words.push(ch);
            i += ch.len_utf8();
        }

        let mut all = vec![Arg {
            name: None,
            value: Expr::Str { value: words, span },
            spread: false,
            span,
        }];
        all.extend(args);
        Expr::Call {
            callee: Box::new(Expr::Ident { name: crate::expand::PHRASE.to_string(), span }),
            args: all,
            span,
        }
    }

    fn switch_expr(&mut self) -> Expr {
        let span = self.peek().span;
        self.open(ScopeKind::Switch, "switch");
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
        self.close();
        Expr::Switch { subject: Box::new(subject), arms, span }
    }
}

/// What to call the slot a `${…}` fills.
///
/// The last segment of a path, because `${state.member.points}` is about points
/// and a bundle for a second language is read by somebody who was not here. A
/// name already used, or an expression that is not a path, falls back to its
/// position — two slots with one name would be one slot.
fn slot_name(inner: &str, taken: &[Arg]) -> String {
    let trimmed = inner.trim();
    let last = trimmed.rsplit('.').next().unwrap_or("");
    let usable = !last.is_empty()
        && last.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && last.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    let already = |n: &str| taken.iter().any(|a| a.name.as_deref() == Some(n));
    if usable && !already(last) {
        return last.to_string();
    }
    let mut n = taken.len();
    loop {
        let candidate = format!("v{n}");
        if !already(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// One expression, parsed on its own. The inside of a `${…}`.
fn parse_expr(src: &str, span: crate::diag::Span) -> (Expr, Vec<Diagnostic>) {
    let (toks, mut d, _, _) = Lexer::new(src).run();
    let mut p = Parser { toks, i: 0, diagnostics: Vec::new(), scopes: Vec::new(), open: Vec::new() };
    let e = p.expr(0);
    // Positions inside a template are the template's own: it is lexed as one
    // token, and a column from a second lexer would point into a string nobody
    // can see.
    for mut x in p.diagnostics {
        x.span = span;
        d.push(x);
    }
    for x in d.iter_mut() {
        x.span = span;
    }
    (e, d)
}
