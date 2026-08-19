//! Tokens, per §2 of the specification.
//!
//! Two rules here are not conveniences and are commented where they are made:
//! a number keeps its decimal point so the message can be about floats, and a
//! non-ASCII identifier character is a different mistake from an unknown one.

use crate::diag::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Ident,
    Str,
    Num,
    Punct,
    /// A newline that ends a statement. Newlines inside an unclosed bracket are
    /// not emitted at all — §2, and the reason the parser never has to ask.
    Newline,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: Kind,
    pub text: String,
    pub span: Span,
}

impl Token {
    pub fn is(&self, s: &str) -> bool {
        self.text == s
    }
}

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    i: usize,
    line: u32,
    col: u32,
    depth: i32,
    pub diagnostics: Vec<Diagnostic>,
}

const PUNCT: &[&str] = &[
    "==", "!=", "<=", ">=", "&&", "||", "->", "=>", "...", "{", "}", "(", ")", "[", "]", ",", ":",
    ".", ";", "?", "+", "-", "*", "/", "%", "<", ">", "=", "!", "@",
];

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer { src, bytes: src.as_bytes(), i: 0, line: 1, col: 1, depth: 0, diagnostics: Vec::new() }
    }

    fn span(&self, len: u32) -> Span {
        Span { line: self.line, col: self.col, len }
    }

    fn bump(&mut self, n: usize) {
        for k in 0..n {
            if self.bytes.get(self.i + k) == Some(&b'\n') {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        self.i += n;
    }

    pub fn run(mut self) -> (Vec<Token>, Vec<Diagnostic>) {
        let mut out: Vec<Token> = Vec::new();

        while self.i < self.bytes.len() {
            let c = self.bytes[self.i];

            if c == b'\n' {
                let span = self.span(1);
                self.bump(1);
                // Inside an unclosed bracket a newline is whitespace; outside
                // one it ends the statement. That is the whole continuation
                // rule, and it lives here so no production has to know it.
                if self.depth == 0 && !matches!(out.last().map(|t| &t.kind), None | Some(Kind::Newline)) {
                    out.push(Token { kind: Kind::Newline, text: "\n".into(), span });
                }
                continue;
            }
            if c == b' ' || c == b'\t' || c == b'\r' {
                self.bump(1);
                continue;
            }
            if c == b'/' && self.bytes.get(self.i + 1) == Some(&b'/') {
                while self.i < self.bytes.len() && self.bytes[self.i] != b'\n' {
                    self.bump(1);
                }
                continue;
            }

            if c == b'"' {
                let span = self.span(1);
                self.bump(1);
                let mut text = String::new();
                let mut closed = false;
                while self.i < self.bytes.len() {
                    match self.bytes[self.i] {
                        b'"' => {
                            self.bump(1);
                            closed = true;
                            break;
                        }
                        // A string is one line. A newline inside one is almost
                        // always a missing quote, and saying so beats carrying
                        // the mistake to the end of the file.
                        b'\n' => break,
                        b'\\' => {
                            let at = self.span(2);
                            self.bump(1);
                            let esc = *self.bytes.get(self.i).unwrap_or(&b'"');
                            self.bump(1);
                            match esc {
                                b'"' => text.push('"'),
                                b'\\' => text.push('\\'),
                                b'n' => text.push('\n'),
                                b't' => text.push('\t'),
                                other => {
                                    // A closed set, so that what a string means
                                    // is answerable without a table nobody has.
                                    self.diagnostics.push(Diagnostic::error(
                                        at,
                                        format!(
                                            "`\\{}` is not an escape. The set is `\\\"`, `\\\\`, `\\n` and `\\t`",
                                            other as char
                                        ),
                                    ));
                                }
                            }
                        }
                        _ => {
                            // A whole character, not a byte. Advancing one byte
                            // into `—` or a Thai vowel and then slicing there
                            // panics the compiler on source that is perfectly
                            // valid — and the language's strings are full UTF-8
                            // on purpose.
                            let start = self.i;
                            let len = char_len(self.bytes[self.i]);
                            self.bump(len);
                            text.push_str(&self.src[start..self.i]);
                        }
                    }
                }
                if !closed {
                    self.diagnostics.push(Diagnostic::error(span, "this string is never closed"));
                }
                out.push(Token { kind: Kind::Str, text, span });
                continue;
            }

            if c.is_ascii_digit() {
                let span = self.span(1);
                let start = self.i;
                while self.i < self.bytes.len()
                    && (self.bytes[self.i].is_ascii_digit() || self.bytes[self.i] == b'_' || self.bytes[self.i] == b'.')
                {
                    // `1..2` would be a range if this language had one; a dot
                    // followed by a non-digit is field access, not a decimal.
                    if self.bytes[self.i] == b'.' && !self.bytes.get(self.i + 1).is_some_and(|b| b.is_ascii_digit()) {
                        break;
                    }
                    self.bump(1);
                }
                let text = self.src[start..self.i].to_string();
                // Taken whole on purpose: split into `12`, `.`, `50` and the
                // error would have been "expected field name", which teaches
                // nothing. The message itself is the checker's, which has the
                // reason to hand.
                if text.starts_with('_') || text.ends_with('_') {
                    self.diagnostics.push(Diagnostic::error(span, "`_` separates digits; it does not start or end a number"));
                }
                out.push(Token { kind: Kind::Num, text, span });
                continue;
            }

            if c.is_ascii_alphabetic() || c == b'_' {
                let span = self.span(1);
                let start = self.i;
                while self.i < self.bytes.len() && (self.bytes[self.i].is_ascii_alphanumeric() || self.bytes[self.i] == b'_') {
                    self.bump(1);
                }
                out.push(Token { kind: Kind::Ident, text: self.src[start..self.i].to_string(), span });
                continue;
            }

            if let Some(p) = PUNCT.iter().find(|p| self.src[self.i..].starts_with(**p)) {
                let span = self.span(p.len() as u32);
                match *p {
                    // Only the brackets an expression wraps in. A brace holds
                    // statements, and statements are newline-separated — so a
                    // newline inside one is a separator and not whitespace.
                    // Counting braces too is why a `switch` arm ran on into the
                    // next line and read `"gold" >= 100` as a comparison.
                    "(" | "[" => self.depth += 1,
                    ")" | "]" => self.depth -= 1,
                    _ => {}
                }
                self.bump(p.len());
                out.push(Token { kind: Kind::Punct, text: (*p).to_string(), span });
                continue;
            }

            // Two different mistakes, two different sentences.
            let ch = self.src[self.i..].chars().next().unwrap_or('?');
            let span = self.span(ch.len_utf8() as u32);
            let msg = if !ch.is_ascii() {
                format!("identifiers are ASCII, and `{ch}` is not. Thai belongs in strings and in the manifest's text bundle")
            } else {
                format!("unexpected character `{ch}`")
            };
            self.diagnostics.push(Diagnostic::error(span, msg));
            self.bump(ch.len_utf8());
        }

        out.push(Token { kind: Kind::Eof, text: String::new(), span: self.span(0) });
        (out, self.diagnostics)
    }
}

/// How many bytes a UTF-8 character takes, from its leading byte.
fn char_len(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        // A continuation byte on its own is not a start; treat it as one byte
        // so the lexer moves rather than looping.
        _ => 1,
    }
}
