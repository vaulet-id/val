//! Runtime values.
//!
//! Deliberately small: there is no float, no reference, no mutable cell. A map
//! is ordered because the canonical encoding needs it to be, and getting that
//! from the type rather than from a sort at encoding time means it cannot be
//! forgotten in one path.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    Null,
    Bool(bool),
    /// 64-bit and signed. Overflow traps — see `Trap::Overflow`.
    Int(i64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
    /// `Tier.gold` — the enum's name and the member's, kept apart so that two
    /// enums with a `gold` are not the same value.
    Enum(String, String),
    /// A credential the host handed over, with the policy it was verified
    /// under, if any. `None` is held-but-unverified and its claims are out of
    /// reach — the type checker says so, and this is the runtime agreeing.
    Credential { ty: String, claims: BTreeMap<String, Value>, verified: Option<String> },
}

impl Value {
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }
    pub fn truthy(&self) -> bool {
        matches!(self, Value::Bool(true))
    }
    /// Field access on a map. A credential's `claims` are reached by the
    /// evaluator, which has the map in hand and the policy to check first.
    pub fn field(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Map(m) => m.get(name),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Str(s) => write!(f, "\"{s}\""),
            Value::Bytes(b) => write!(f, "<{} bytes>", b.len()),
            Value::List(v) => {
                write!(f, "[")?;
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{x}")?;
                }
                write!(f, "]")
            }
            Value::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
            Value::Enum(e, m) => write!(f, "{e}.{m}"),
            Value::Credential { ty, verified, .. } => match verified {
                Some(p) => write!(f, "Verified<{p}> of {ty}"),
                None => write!(f, "Credential<{ty}>"),
            },
        }
    }
}
