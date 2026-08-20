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
            // **`prove` takes nothing either**, and that is the whole of what
            // makes it a proof: the host evaluates the statement and builds the
            // proof, because the host is the only one that can. Handing the
            // claim to the module would be the same answer with the privacy
            // removed, which is the thing `prove` exists instead of.
            Cap::Prove(_) => 0,
            // The rest are handed the value they act on.
            Cap::Disclose(_) | Cap::Issue(_) | Cap::Pay(_) | Cap::Write(_) => 1,
        }
    }
}

/// What a module reaches for that is not a capability.
///
/// The runtime's own values: the input it was handed, the state it is running
/// against, the clock. None of it is a line in the report, because none of it
/// is something a person is being asked to agree to — reading state this
/// application wrote is not reading anything of theirs.
///
/// They are still named imports, and still refused when unknown: a hole in this
/// namespace would be a hole in the other one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// One of the fixed arithmetic and value operations.
    Fixed(String),
    /// `state:member.points` — a field of state, read.
    State(String),
    /// `input:receipt` — something the host collected before the action ran.
    Input(String),
    /// `context:time.now`
    Context(String),
    /// `refuse:tooSmallToEarn` — the application declining, in words from the
    /// signed text bundle. An outcome, not an effect.
    Refuse(String),
    /// A `require` that did not hold: a defect, and the action stops.
    Defect,
    /// A `verify` that did not hold. A different outcome from a defect, because
    /// a credential failing its policy is not a mistake in this program.
    Unverified,
}

impl Op {
    pub fn name(&self) -> String {
        match self {
            Op::Fixed(x) => x.clone(),
            Op::State(x) => format!("state:{x}"),
            Op::Input(x) => format!("input:{x}"),
            Op::Context(x) => format!("context:{x}"),
            Op::Refuse(x) => format!("refuse:{x}"),
            Op::Defect => "defect".into(),
            Op::Unverified => "unverified".into(),
        }
    }

    pub fn parse(name: &str, is_fixed: impl Fn(&str) -> bool) -> Option<Op> {
        if is_fixed(name) {
            return Some(Op::Fixed(name.to_string()));
        }
        if name == "defect" {
            return Some(Op::Defect);
        }
        if name == "unverified" {
            return Some(Op::Unverified);
        }
        let (kind, what) = name.split_once(':')?;
        if what.is_empty() {
            return None;
        }
        let what = what.to_string();
        Some(match kind {
            "state" => Op::State(what),
            "input" => Op::Input(what),
            "context" => Op::Context(what),
            "refuse" => Op::Refuse(what),
            _ => return None,
        })
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
    /// What it reads, as a person is shown it: the claims of one credential on
    /// one line, under the policy they were checked against.
    ///
    /// The set is finer than the sentence — `PurchaseReceipt under Policy` and
    /// `PurchaseReceipt.amount` are two facts, and a person reads one line. The
    /// joining is presentation and lives here rather than in the derivation,
    /// because a report that dropped the difference could not answer "which
    /// claims" later.
    pub fn reads_as_lines(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for entry in &self.reads {
            // `Type under Policy` and `Type — unverified` carry a policy; a
            // bare `Type.claim` is one of the claims under one of them.
            let Some((ty, rest)) = entry.split_once(' ') else { continue };
            let touched: Vec<&str> = self
                .reads
                .iter()
                .filter(|c| c.starts_with(&format!("{ty}.")))
                .map(String::as_str)
                .collect();
            if touched.is_empty() {
                out.insert(entry.clone());
            } else {
                out.insert(format!("{} {rest}", touched.join(", ")));
            }
        }
        out
    }

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
    wants_with(bytes, |name| crate::compile::IMPORTS.iter().any(|(n, _)| *n == name))
}

/// The same, told which names are the fixed operations. Split out so the check
/// can be run without the compiler beside it — a wallet has the second half of
/// this crate and none of the first.
pub fn wants_with(bytes: &[u8], is_fixed: impl Fn(&str) -> bool) -> Result<Wants, String> {
    let engine = wasmi::Engine::default();
    let module = wasmi::Module::new(&engine, bytes).map_err(|e| e.to_string())?;

    let mut caps = Vec::new();
    for import in module.imports() {
        match import.module() {
            OPS => {
                Op::parse(import.name(), &is_fixed).ok_or_else(|| {
                    format!("`{}` is not an operation this host provides", import.name())
                })?;
                continue;
            }
            CAPS => {}
            other => return Err(format!("a module may import `{OPS}` and `{CAPS}`, and this one imports `{other}`")),
        }
        let cap = Cap::parse(import.name())
            .ok_or_else(|| format!("`{}` is not a capability this host knows", import.name()))?;
        caps.push(cap);
    }
    Ok(Wants::of(caps))
}

/// The capability report of a program, derived the way a wallet derives it.
///
/// **One route.** What an application does to the person comes from the module
/// and from nowhere else — a second walk, over the source, is a second answer,
/// and the two disagreed the first time they were compared: the walk said a
/// program that proves an age reads the birthdate, about a module with no way
/// to reach it.
///
/// The rest of the report is not about effects and does not come from here:
/// which application this is, which hosts it needs, what it exports. Those are
/// facts about the package, and the front end already knows them.
pub fn report_of(program: &valang::ast::Program) -> Result<valang::report::Report, Vec<String>> {
    let module = crate::compile::compile_program(program)?;
    let wants = wants_of(&module.bytes).map_err(|e| vec![e])?;
    let mut r = valang::report::report(program);
    r.reads = wants.reads_as_lines();
    r.discloses = wants.discloses;
    r.proves = wants.proves;
    r.issues = wants.issues;
    r.audiences = wants.audiences;
    r.payments = wants.payments;
    r.writes = wants.writes;
    // Nothing a person can take back: what was disclosed was seen, what was
    // proved was proved, and money that moved moved.
    r.irreversible = !r.discloses.is_empty() || !r.proves.is_empty() || !r.payments.is_empty();
    Ok(r)
}
