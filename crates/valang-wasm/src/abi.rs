//! What a module may reach outside itself, and what that is called.
//!
//! **The capability report is the import section.** A wallet that downloads a
//! compiled Micro App has no compiler — it cannot recompile the code to find
//! out what the code does, and a report the publisher wrote is a claim rather
//! than evidence. So every operation that touches anything outside the module
//! is an *import*, named as the line it becomes in the report, and a wallet
//! reads that list off the bytes it was handed.
//!
//! It is not a promise that can be broken. A module can call what it imports
//! and nothing else: there is no memory it shares, no syscall, no dynamic
//! linking. Reading a claim of a credential is an import; disclosing one is an
//! import; writing a field of state is an import. What the list says it can do
//! is the whole of what it can do.
//!
//! The list is a **ceiling and not a trace** — a module may import something
//! and never call it. That is the safe direction for consent: a person agrees
//! to what could happen.
//!
//! Two namespaces, and the difference is the point:
//!
//! | | |
//! |---|---|
//! | `val` | arithmetic and values. Every module imports all of it; it says nothing about anybody |
//! | `cap` | what the person is consenting to. Every entry is a line of the report |

use std::collections::BTreeSet;

/// Arithmetic. Fixed, and the same in every module.
pub const OPS: &str = "val";

/// What the person consents to.
pub const CAPS: &str = "cap";

/// One thing a module may do to the world.
///
/// The parameter is part of the name because the report names it: "reads the
/// amount on your receipts" and "reads your receipts" are different sentences,
/// and a wallet with only the bytes has to be able to say the first one. It
/// also has to be settled at compile time — a claim chosen while running could
/// not appear here, and a report that could not name it would be a report that
/// understates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cap {
    /// A claim of a credential: `PurchaseReceipt.amount`.
    Read(String),
    /// A claim handed to somebody else.
    Disclose(String),
    /// A statement proved without the value behind it.
    Prove(String),
    /// A credential this application issues.
    Issue(String),
    /// An audience it talks to.
    Query(String),
    /// Money it moves.
    Pay(String),
    /// A field of state it writes: `member.points`.
    Write(String),
}

impl Cap {
    fn parts(&self) -> (&'static str, &str) {
        match self {
            Cap::Read(x) => ("read", x),
            Cap::Disclose(x) => ("disclose", x),
            Cap::Prove(x) => ("prove", x),
            Cap::Issue(x) => ("issue", x),
            Cap::Query(x) => ("query", x),
            Cap::Pay(x) => ("pay", x),
            Cap::Write(x) => ("write", x),
        }
    }

    /// The name this is imported under. Two capabilities with the same name are
    /// one import, which is what makes the list a set rather than a log.
    pub fn name(&self) -> String {
        let (kind, what) = self.parts();
        format!("{kind}:{what}")
    }

    /// Read one back. A name this build does not know is `None` — and a host
    /// refuses a module that imports one, rather than running something it
    /// could not describe to the person.
    pub fn parse(name: &str) -> Option<Cap> {
        let (kind, what) = name.split_once(':')?;
        if what.is_empty() {
            return None;
        }
        let what = what.to_string();
        Some(match kind {
            "read" => Cap::Read(what),
            "disclose" => Cap::Disclose(what),
            "prove" => Cap::Prove(what),
            "issue" => Cap::Issue(what),
            "query" => Cap::Query(what),
            "pay" => Cap::Pay(what),
            "write" => Cap::Write(what),
            _ => return None,
        })
    }

    /// How many handles it takes. The value it answers with is one handle, the
    /// same as everything else in this ABI.
    pub fn arity(&self) -> usize {
        match self {
            // `read` and `query` name what they want in the import itself, so
            // there is nothing left to pass.
            Cap::Read(_) | Cap::Query(_) => 0,
            // The rest are handed the value they act on.
            Cap::Disclose(_) | Cap::Prove(_) | Cap::Issue(_) | Cap::Pay(_) | Cap::Write(_) => 1,
        }
    }
}

/// Everything a set of capabilities amounts to, in the shape a report is read
/// in. Deliberately not `valang::report::Report`: that one is derived from the
/// source by a different route, and the test that the two agree is only worth
/// running while they are computed apart.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Wants {
    pub reads: BTreeSet<String>,
    pub discloses: BTreeSet<String>,
    pub proves: BTreeSet<String>,
    pub issues: BTreeSet<String>,
    pub audiences: BTreeSet<String>,
    pub payments: BTreeSet<String>,
    pub writes: BTreeSet<String>,
}

impl Wants {
    pub fn of(caps: impl IntoIterator<Item = Cap>) -> Wants {
        let mut w = Wants::default();
        for c in caps {
            match c {
                Cap::Read(x) => w.reads.insert(x),
                Cap::Disclose(x) => w.discloses.insert(x),
                Cap::Prove(x) => w.proves.insert(x),
                Cap::Issue(x) => w.issues.insert(x),
                Cap::Query(x) => w.audiences.insert(x),
                Cap::Pay(x) => w.payments.insert(x),
                Cap::Write(x) => w.writes.insert(x),
            };
        }
        w
    }
}

/// What a module can do, read off the module and nothing else.
///
/// **This is what a wallet runs before it shows anybody a consent sheet.** No
/// source, no manifest, no publisher's word: the bytes that are about to run
/// say what they can reach, because reaching anything means importing it.
///
/// A module importing a name this build does not know is refused. A host that
/// linked it anyway would be running something it could not describe, and a
/// host that dropped it would be describing something less than what runs.
pub fn wants_of(bytes: &[u8]) -> Result<Wants, String> {
    let engine = wasmi::Engine::default();
    let module = wasmi::Module::new(&engine, bytes).map_err(|e| e.to_string())?;

    let mut caps = Vec::new();
    for import in module.imports() {
        match import.module() {
            OPS => continue,
            CAPS => {}
            other => return Err(format!("a module may import `{OPS}` and `{CAPS}`, and this one imports `{other}`")),
        }
        let cap = Cap::parse(import.name())
            .ok_or_else(|| format!("`{}` is not a capability this host knows", import.name()))?;
        caps.push(cap);
    }
    Ok(Wants::of(caps))
}
