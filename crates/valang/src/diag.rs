//! Diagnostics. The message is part of the language: a rule that produces only
//! "type error" has not been taught to anybody.

use std::fmt;

use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: u32,
    pub col: u32,
    pub len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub span: Span,
    pub severity: Severity,
    pub message: String,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Diagnostic { span, severity: Severity::Error, message: message.into() }
    }
    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Diagnostic { span, severity: Severity::Warning, message: message.into() }
    }
}

impl Diagnostic {
    /// The diagnostic with the line it is about, and the part of it underlined.
    ///
    /// A position on its own is a thing to go and look up. The line is what a
    /// person reads, and the underline is the difference between "somewhere on
    /// line 12" and "this word".
    pub fn render(&self, source: &str) -> String {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let Some(line) = source.lines().nth(self.span.line.saturating_sub(1) as usize) else {
            // A span with no line is one of the few that are about the package
            // rather than a place in it — a capability declared and never used
            // has nowhere to point.
            return format!("{severity}: {}", self.message);
        };

        let number = self.span.line.to_string();
        let gutter = " ".repeat(number.len());
        // Placed by how wide the text is on a terminal, not by how many
        // characters it has: a Thai tone mark takes no columns and a CJK
        // character takes two, so counting characters marks the wrong place on
        // half of what these files say.
        let skip = self.span.col.saturating_sub(1) as usize;
        let before: usize = line
            .chars()
            .take(skip)
            .map(|c| if c == '\t' { 4 } else { UnicodeWidthChar::width(c).unwrap_or(0) })
            .sum();
        let width: usize = line
            .chars()
            .skip(skip)
            .take((self.span.len as usize).max(1))
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
            .sum::<usize>()
            .max(1);

        format!(
            "{severity}: {}\n{gutter}--> {}:{}\n{gutter} |\n{number} | {}\n{gutter} | {}{}",
            self.message,
            self.span.line,
            self.span.col,
            line.replace('\t', "    "),
            " ".repeat(before),
            "^".repeat(width),
        )
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{}:{}: {}: {}", self.span.line, self.span.col, s, self.message)
    }
}
