//! Types, and the provenance that travels with a value.
//!
//! `Verified<P>` names the *policy*, not the credential, because the policy
//! already determines the credential and two parameters that cannot disagree
//! are two parameters somebody will eventually write as if they could (§4).

use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Int,
    Str,
    Bool,
    Date,
    DateTime,
    Bytes,
    /// A declared enum.
    Enum(String),
    /// A declared credential's claim record.
    Claims(String),
    /// `Credential<T>` — held, not yet verified.
    Credential(String),
    /// `Verified<P>` — the only thing a `verify` block produces.
    Verified(String),
    Proof,
    List(Box<Ty>),
    Optional(Box<Ty>),
    /// A record literal, or a declared credential being constructed.
    Record(String),
    Lambda,
    /// Nothing is known. Never an error on its own — an error was already
    /// reported where the knowledge was lost.
    Unknown,
}

impl Ty {
    pub fn optional(self) -> Ty {
        Ty::Optional(Box::new(self))
    }
    pub fn inner(&self) -> &Ty {
        match self {
            Ty::Optional(t) => t,
            other => other,
        }
    }
    pub fn is_unknown(&self) -> bool {
        matches!(self, Ty::Unknown)
    }
    /// Assignability, which in this language is nearly equality — deliberately.
    pub fn accepts(&self, other: &Ty) -> bool {
        if self.is_unknown() || other.is_unknown() {
            return true;
        }
        if self == other {
            return true;
        }
        match (self, other) {
            // A non-optional value satisfies an optional one, never the reverse.
            (Ty::Optional(a), b) => a.accepts(b),
            (Ty::List(a), Ty::List(b)) => a.accepts(b),
            // Dates and datetimes compare as integers do; nothing else widens.
            (Ty::DateTime, Ty::Date) | (Ty::Date, Ty::DateTime) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int => write!(f, "int"),
            Ty::Str => write!(f, "string"),
            Ty::Bool => write!(f, "bool"),
            Ty::Date => write!(f, "date"),
            Ty::DateTime => write!(f, "datetime"),
            Ty::Bytes => write!(f, "bytes"),
            Ty::Enum(n) | Ty::Record(n) => write!(f, "{n}"),
            Ty::Claims(n) => write!(f, "{n}'s claims"),
            Ty::Credential(n) => write!(f, "Credential<{n}>"),
            Ty::Verified(p) => write!(f, "Verified<{p}>"),
            Ty::Proof => write!(f, "Proof<bool>"),
            Ty::List(t) => write!(f, "List<{t}>"),
            Ty::Optional(t) => write!(f, "{t}?"),
            Ty::Lambda => write!(f, "a function"),
            Ty::Unknown => write!(f, "?"),
        }
    }
}

/// The set of trust policies a value descends from. Empty means self-asserted:
/// computed from state, from a query, or from a literal — true, checkable, and
/// backed by nobody (§7).
pub type Provenance = BTreeSet<String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typed {
    pub ty: Ty,
    pub from: Provenance,
}

impl Typed {
    pub fn plain(ty: Ty) -> Typed {
        Typed { ty, from: Provenance::new() }
    }
    pub fn with(ty: Ty, from: Provenance) -> Typed {
        Typed { ty, from }
    }
    pub fn unknown() -> Typed {
        Typed::plain(Ty::Unknown)
    }
    /// Propagation is set union, in one pass, because the language is total and
    /// the lattice is a set of names rather than a hierarchy.
    pub fn join(a: &Typed, b: &Typed, ty: Ty) -> Typed {
        let mut from = a.from.clone();
        from.extend(b.from.iter().cloned());
        Typed { ty, from }
    }
}
