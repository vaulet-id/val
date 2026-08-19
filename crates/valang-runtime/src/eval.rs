//! The tree-walking evaluator.
//!
//! An action is `(previous state, input, runtime context, code)` to
//! `(new state, output, effects)`. Every phase below is one of those arrows,
//! and the only phase that can reach the world is the last one — which does not
//! reach it either, it describes what it would like the host to do.

use std::collections::BTreeMap;

use valang::ast::*;

use crate::host::{Context, EffectRequest};
use crate::value::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trap {
    /// The application declined, for its own reasons, naming a key in the text
    /// bundle. Not a defect and not a trust failure — a third thing, which is
    /// why the language needed it.
    Refused(String),
    Overflow(String),
    DivideByZero,
    /// `require` failed: a defect in the application. Nobody is shown this.
    Defect(String),
    /// `verify` failed: an ordinary outcome. The person is told.
    Failed(String),
    Unsupported(String),
}

pub struct Eval<'a> {
    pub program: &'a Program,
    pub context: Context,
    scope: Vec<BTreeMap<String, Value>>,
    pub effects: Vec<EffectRequest>,
}

type R<T> = Result<T, Trap>;

impl<'a> Eval<'a> {
    pub fn new(program: &'a Program, context: Context) -> Self {
        Eval { program, context, scope: vec![BTreeMap::new()], effects: Vec::new() }
    }

    pub fn bind(&mut self, name: &str, v: Value) {
        self.scope.last_mut().unwrap().insert(name.to_string(), v);
    }
    fn lookup(&self, name: &str) -> Option<&Value> {
        self.scope.iter().rev().find_map(|s| s.get(name))
    }
    pub fn returned(&self) -> bool {
        self.lookup("__return").is_some()
    }
    fn push(&mut self) {
        self.scope.push(BTreeMap::new());
    }
    fn pop(&mut self) {
        self.scope.pop();
    }

    // ------------------------------------------------------------ statements

    pub fn stmt(&mut self, s: &Stmt, phase: Phase, state: &mut BTreeMap<String, Value>) -> R<()> {
        match s {
            Stmt::Binding { .. } | Stmt::Data { .. } => Ok(()),

            Stmt::Refuse { key, .. } => Err(Trap::Refused(key.clone())),

            Stmt::Let { name, value, .. } => {
                let v = self.expr(value, state)?;
                self.bind(name, v);
                Ok(())
            }

            Stmt::Return { value, .. } => {
                let v = self.expr(value, state)?;
                self.bind("__return", v);
                Ok(())
            }

            Stmt::If { cond, then, other, .. } => {
                let branch = if self.expr(cond, state)?.truthy() { then } else { other };
                for s in branch {
                    self.stmt(s, phase, state)?;
                    // A `return` inside a branch ends the function, not the
                    // branch. Without this the rest of the body runs and the
                    // last `return` wins, which is a silent wrong answer.
                    if self.returned() {
                        break;
                    }
                }
                Ok(())
            }

            Stmt::Expr { value, span } => {
                let v = self.expr(value, state)?;
                if v.truthy() {
                    return Ok(());
                }
                let where_ = format!("line {}", span.line);
                Err(match phase {
                    // The distinction the language draws, kept at runtime: one
                    // of these is a bug report and the other is a sentence the
                    // person reads.
                    Phase::Require => Trap::Defect(format!("a precondition did not hold ({where_})")),
                    _ => Trap::Failed(format!("a trust rule did not hold ({where_})")),
                })
            }

            Stmt::Patch { path, value, .. } => {
                let v = self.expr(value, state)?;
                patch(state, path, v);
                Ok(())
            }

            Stmt::Effect { name, args, body, .. } => {
                for a in args {
                    let payload = self.expr(&a.value, state)?;
                    self.effects.push(EffectRequest {
                        capability: name.clone(),
                        operation: name.split('.').nth(1).unwrap_or(name).to_string(),
                        payload,
                        reversible: name != "present" && name != "disclose" && name != "prove",
                    });
                }
                if name == "present" && args.is_empty() {
                    // A `present` block: its `disclose` and `prove` lines are
                    // the payload, and it is one effect, not several.
                    let mut parts = Vec::new();
                    for s in body {
                        if let Stmt::Effect { name, args, .. } = s {
                            let mut m = BTreeMap::new();
                            m.insert("kind".into(), Value::Str(name.clone()));
                            match (name.as_str(), args.first()) {
                                // `disclose` hands over a value, so the value is
                                // what the host is being asked to hand over.
                                ("disclose", Some(a)) => {
                                    m.insert("of".into(), self.expr(&a.value, state)?);
                                }
                                // `prove` hands over an answer, and the host is
                                // the one that produces it. Evaluating the
                                // predicate here would mean the runtime knew the
                                // answer and shipped it — which is a disclosure
                                // wearing the word `prove`.
                                ("prove", Some(a)) => {
                                    m.insert("statement".into(), Value::Str(render(&a.value)));
                                }
                                _ => {}
                            }
                            parts.push(Value::Map(m));
                        }
                    }
                    self.effects.push(EffectRequest {
                        capability: "disclosure.present".into(),
                        operation: "present".into(),
                        payload: Value::List(parts),
                        reversible: false,
                    });
                    return Ok(());
                }
                for s in body {
                    self.stmt(s, phase, state)?;
                }
                Ok(())
            }
        }
    }

    // ----------------------------------------------------------- expressions

    pub fn expr(&mut self, e: &Expr, state: &BTreeMap<String, Value>) -> R<Value> {
        Ok(match e {
            Expr::Num { value, .. } => Value::Int(*value),
            Expr::Str { value, .. } => Value::Str(value.clone()),
            Expr::Bool { value, .. } => Value::Bool(*value),
            Expr::Float { text, .. } => return Err(Trap::Unsupported(format!("`{text}` is not a value in this language"))),
            Expr::Error { .. } => Value::Null,

            Expr::Ident { name, .. } => match name.as_str() {
                "state" => Value::Map(state.clone()),
                // A binding wins over a type name, and an enum's name is a
                // value so that `Tier.bronze` has something to be a member of.
                _ => match self.lookup(name) {
                    Some(v) => v.clone(),
                    None if self.program.enums.iter().any(|e| e.name == *name) => {
                        Value::Enum(name.clone(), String::new())
                    }
                    None => Value::Null,
                },
            },

            Expr::Member { obj, name, .. } => self.member(obj, name, state)?,

            Expr::Unary { op, rhs, .. } => {
                let v = self.expr(rhs, state)?;
                match (op.as_str(), v) {
                    ("-", Value::Int(i)) => Value::Int(i.checked_neg().ok_or_else(|| Trap::Overflow("negation".into()))?),
                    ("!", Value::Bool(b)) => Value::Bool(!b),
                    _ => Value::Null,
                }
            }

            Expr::Binary { op, lhs, rhs, .. } => {
                let a = self.expr(lhs, state)?;
                // Short-circuit, because the right side of an `&&` may be the
                // thing the left side was checking was safe.
                match (op.as_str(), &a) {
                    ("&&", Value::Bool(false)) => return Ok(Value::Bool(false)),
                    ("||", Value::Bool(true)) => return Ok(Value::Bool(true)),
                    _ => {}
                }
                let b = self.expr(rhs, state)?;
                binary(op, a, b)?
            }

            Expr::Ternary { cond, then, other, .. } => {
                if self.expr(cond, state)?.truthy() {
                    self.expr(then, state)?
                } else {
                    self.expr(other, state)?
                }
            }

            Expr::Exists { subject, .. } => Value::Bool(!matches!(self.expr(subject, state)?, Value::Null)),

            // Verification happened before the program ran: the host handed over
            // a credential it had already checked against this policy, or it
            // handed over nothing.
            Expr::With { subject, policy, .. } => match self.expr(subject, state)? {
                Value::Credential { ty, claims, .. } => Value::Credential { ty, claims, verified: Some(policy.clone()) },
                Value::Null => return Err(Trap::Failed(format!("nothing satisfied `{policy}`"))),
                other => other,
            },

            Expr::From { value, .. } => self.expr(value, state)?,

            Expr::List { items, .. } => {
                let mut out = Vec::new();
                for i in items {
                    out.push(self.expr(i, state)?);
                }
                Value::List(out)
            }
            Expr::Record { spread, fields, .. } => {
                let mut m = match spread {
                    Some(s) => match self.expr(s, state)? {
                        Value::Map(m) => m,
                        Value::Credential { claims, .. } => claims,
                        _ => BTreeMap::new(),
                    },
                    None => BTreeMap::new(),
                };
                for (k, v) in fields {
                    let val = self.expr(v, state)?;
                    m.insert(k.clone(), val);
                }
                Value::Map(m)
            }

            Expr::Switch { subject, arms, .. } => {
                let s = self.expr(subject, state)?;
                let mut out = Value::Null;
                for a in arms {
                    let hit = match &a.pattern {
                        ArmPattern::Default => true,
                        ArmPattern::Value(v) => self.expr(v, state)? == s,
                        ArmPattern::Compare { op, rhs } => {
                            let r = self.expr(rhs, state)?;
                            binary(op, s.clone(), r)?.truthy()
                        }
                    };
                    if hit {
                        out = self.expr(&a.body, state)?;
                        break;
                    }
                }
                out
            }

            Expr::Lambda { .. } => Value::Null,

            Expr::Call { callee, args, .. } => self.call(callee, args, state)?,
        })
    }

    fn member(&mut self, obj: &Expr, name: &str, state: &BTreeMap<String, Value>) -> R<Value> {
        if let Some(path) = obj.path() {
            if path == "context" {
                return Ok(Value::Null);
            }
            if path == "context.time" && name == "now" {
                return Ok(Value::Int(self.context.time_now));
            }
            if path == "context.random" && name == "uuid" {
                return Ok(Value::Str(self.context.random_uuid.clone()));
            }
        }
        let base = self.expr(obj, state)?;
        Ok(match base {
            Value::Credential { claims, verified, ty } => match name {
                // The type checker refuses this at compile time; the runtime
                // agrees rather than trusting that it ran.
                "claims" if verified.is_some() => Value::Map(claims),
                "claims" => return Err(Trap::Defect(format!("the claims of an unverified `{ty}` are out of reach"))),
                _ => Value::Null,
            },
            Value::Map(m) => m.get(name).cloned().unwrap_or(Value::Null),
            Value::Enum(e, _) => Value::Enum(e, name.to_string()),
            _ => Value::Null,
        })
    }

    fn call(&mut self, callee: &Expr, args: &[Arg], state: &BTreeMap<String, Value>) -> R<Value> {
        // `xs.fold(0) { acc, x -> … }` and the rest of the closed set.
        if let Expr::Member { obj, name, .. } = callee {
            let recv = self.expr(obj, state)?;
            if let Value::List(items) = recv {
                return self.combinator(name, items, args, state);
            }
        }

        let Some(name) = callee.path() else { return Ok(Value::Null) };

        if let Some(decl) = self.program.enums.iter().find(|e| e.name == name) {
            return Ok(Value::Enum(decl.name.clone(), String::new()));
        }

        // Constructing a credential's claims.
        if self.program.credentials.iter().any(|c| c.name == name) {
            if let Some(a) = args.first() {
                let v = self.expr(&a.value, state)?;
                let claims = match v {
                    Value::Map(m) => m,
                    _ => BTreeMap::new(),
                };
                return Ok(Value::Credential { ty: name, claims, verified: None });
            }
        }

        if name == "duration" {
            let n = args.first().map(|a| self.expr(&a.value, state)).transpose()?.and_then(|v| v.as_int()).unwrap_or(0);
            let unit = args.first().and_then(|a| a.name.clone()).unwrap_or_else(|| "days".into());
            let ms = match unit.as_str() {
                "hours" => 3_600_000,
                "days" => 86_400_000,
                "years" => 31_536_000_000,
                other => return Err(Trap::Unsupported(format!("`duration` has no unit `{other}`"))),
            };
            return Ok(Value::Int(n.checked_mul(ms).ok_or_else(|| Trap::Overflow("duration".into()))?));
        }

        // The rest of the closed set. The type checker knows their arity and
        // their types; without them here a program it accepted stopped at run
        // time with `no function named min`, which is the two halves of one
        // compiler disagreeing about what the language has.
        if matches!(name.as_str(), "min" | "max" | "abs") {
            let mut ns = Vec::new();
            for a in args {
                let v = self.expr(&a.value, state)?;
                ns.push(v.as_int().ok_or_else(|| {
                    Trap::Unsupported(format!("`{name}` takes whole numbers"))
                })?);
            }
            return match (name.as_str(), ns.as_slice()) {
                ("min", [a, b]) => Ok(Value::Int(*a.min(b))),
                ("max", [a, b]) => Ok(Value::Int(*a.max(b))),
                // Trapping rather than wrapping, as every other arithmetic here
                // does: the absolute value of the smallest i64 is not an i64.
                ("abs", [a]) => a
                    .checked_abs()
                    .map(Value::Int)
                    .ok_or_else(|| Trap::Overflow("abs".into())),
                _ => Err(Trap::Unsupported(format!("`{name}` was given {} arguments", ns.len()))),
            };
        }

        if let Some(f) = self.program.functions.iter().find(|f| f.name == name).cloned() {
            let mut values = Vec::new();
            for arg in args {
                values.push(self.expr(&arg.value, state)?);
            }
            return self.call_declared(&f, values, state);
        }

        Err(Trap::Unsupported(format!("no function named `{name}`")))
    }

    /// A declared function, with its arguments already evaluated.
    ///
    /// Held apart from the call syntax because a function is also passed by
    /// name — `receipts.map(double)` — and there the values come from the list
    /// rather than from a call site.
    fn call_declared(
        &mut self,
        f: &FunctionDecl,
        values: Vec<Value>,
        state: &BTreeMap<String, Value>,
    ) -> R<Value> {
        {
            self.push();
            for (param, v) in f.params.iter().zip(values) {
                self.bind(&param.name, v);
            }
            let mut out = Value::Null;
            for s in &f.body {
                let mut ignored = state.clone();
                self.stmt(s, Phase::Compute, &mut ignored)?;
                if let Some(v) = self.lookup("__return") {
                    out = v.clone();
                    break;
                }
            }
            self.pop();
            return Ok(out);
        }
    }

    /// One function, with values for its parameters.
    fn apply(
        &mut self,
        f: &Callable,
        values: Vec<Value>,
        state: &BTreeMap<String, Value>,
    ) -> R<Value> {
        match f {
            Callable::Written(params, body) => {
                self.push();
                for (p, v) in params.iter().zip(values) {
                    self.bind(p, v);
                }
                let out = self.expr(body, state);
                self.pop();
                out
            }
            Callable::Named(name) => {
                let Some(decl) =
                    self.program.functions.iter().find(|f| f.name == *name).cloned()
                else {
                    return Err(Trap::Unsupported(format!("no function named `{name}`")));
                };
                self.call_declared(&decl, values, state)
            }
        }
    }

    fn combinator(&mut self, name: &str, items: Vec<Value>, args: &[Arg], state: &BTreeMap<String, Value>) -> R<Value> {
        // Either shape of function: written here, or named. A function passed
        // by name used to match nothing and the combinator returned the list
        // unchanged — which compiled, ran, and did nothing.
        let f = args.iter().find_map(|a| match &a.value {
            Expr::Lambda { params, body, .. } => {
                Some(Callable::Written(params.clone(), (**body).clone()))
            }
            Expr::Ident { name, .. }
                if self.program.functions.iter().any(|f| f.name == *name) =>
            {
                Some(Callable::Named(name.clone()))
            }
            _ => None,
        });

        Ok(match name {
            "count" => Value::Int(items.len() as i64),
            "first" => items.into_iter().next().unwrap_or(Value::Null),
            "fold" => {
                let mut acc = match args.first() {
                    Some(a) => self.expr(&a.value, state)?,
                    None => Value::Null,
                };
                let Some(f) = f else { return Ok(acc) };
                for item in items {
                    acc = self.apply(&f, vec![acc, item], state)?;
                }
                acc
            }
            "map" | "filter" | "any" | "all" => {
                let Some(f) = f else { return Ok(Value::List(items)) };
                let mut mapped = Vec::new();
                for item in items {
                    let v = self.apply(&f, vec![item.clone()], state)?;
                    match name {
                        "map" => mapped.push(v),
                        "filter" => {
                            if v.truthy() {
                                mapped.push(item)
                            }
                        }
                        "any" => {
                            if v.truthy() {
                                return Ok(Value::Bool(true));
                            }
                        }
                        _ => {
                            if !v.truthy() {
                                return Ok(Value::Bool(false));
                            }
                        }
                    }
                }
                match name {
                    "any" => Value::Bool(false),
                    "all" => Value::Bool(true),
                    _ => Value::List(mapped),
                }
            }
            other => return Err(Trap::Unsupported(format!("a list has no `{other}`"))),
        })
    }
}

/// A predicate as text, for a host that has to build a circuit out of it. Not
/// pretty-printing for its own sake: the statement is what gets proved, so it
/// is what the execution record has to carry.
fn render(e: &Expr) -> String {
    match e {
        Expr::Num { value, .. } => value.to_string(),
        Expr::Str { value, .. } => format!("\"{value}\""),
        Expr::Bool { value, .. } => value.to_string(),
        Expr::Binary { op, lhs, rhs, .. } => format!("{} {op} {}", render(lhs), render(rhs)),
        Expr::Call { callee, args, .. } => format!(
            "{}({})",
            render(callee),
            args.iter().map(|a| render(&a.value)).collect::<Vec<_>>().join(", ")
        ),
        Expr::Member { obj, name, .. } => format!("{}.{name}", render(obj)),
        Expr::Ident { name, .. } => name.clone(),
        Expr::Lambda { .. } => "…".into(),
        other => other.path().unwrap_or_else(|| "…".into()),
    }
}

/// What a combinator was handed: a function written where it is used, or the
/// name of one the package declared.
///
/// Both are called the same way. Until this existed only the first was, and a
/// function passed by name matched nothing — so `receipts.map(double)` compiled,
/// ran, and returned the list unchanged.
enum Callable {
    Written(Vec<String>, Expr),
    Named(String),
}

/// How long a range may be.
///
/// Every other list in this language came from the host and carries a `limit`.
/// A range is written, so its bound is written here — and totality is the one
/// promise the language does not bend.
const RANGE_LIMIT: i64 = 10_000;

fn binary(op: &str, a: Value, b: Value) -> R<Value> {
    use Value::*;
    Ok(match (op, &a, &b) {
        // Trapping, not wrapping: a wrong number the record would then
        // faithfully prove is worse than a failure (§3).
        ("+", Int(x), Int(y)) => Int(x.checked_add(*y).ok_or_else(|| Trap::Overflow("addition".into()))?),
        ("-", Int(x), Int(y)) => Int(x.checked_sub(*y).ok_or_else(|| Trap::Overflow("subtraction".into()))?),
        ("*", Int(x), Int(y)) => Int(x.checked_mul(*y).ok_or_else(|| Trap::Overflow("multiplication".into()))?),
        ("/", Int(_), Int(0)) => return Err(Trap::DivideByZero),
        ("/", Int(x), Int(y)) => Int(x / y),
        ("%", Int(_), Int(0)) => return Err(Trap::DivideByZero),
        ("%", Int(x), Int(y)) => Int(x % y),
        // `0...10` — both ends included, as the punctuation says in every
        // language that spells it this way. Bounded here rather than trusted,
        // because a range is the one expression whose size is not a property of
        // the data the host handed over: it is whatever the arithmetic said.
        ("...", Int(x), Int(y)) => {
            if y < x {
                List(Vec::new())
            } else {
                let count = y.checked_sub(*x).and_then(|n| n.checked_add(1));
                match count {
                    Some(n) if n <= RANGE_LIMIT => List((*x..=*y).map(Int).collect()),
                    _ => {
                        return Err(Trap::Failed(format!(
                            "a range of {x} to {y} is more steps than a screen can be made of. The limit is {RANGE_LIMIT}"
                        )))
                    }
                }
            }
        }
        ("<", Int(x), Int(y)) => Bool(x < y),
        ("<=", Int(x), Int(y)) => Bool(x <= y),
        (">", Int(x), Int(y)) => Bool(x > y),
        (">=", Int(x), Int(y)) => Bool(x >= y),
        ("==", _, _) => Bool(a == b),
        ("!=", _, _) => Bool(a != b),
        ("&&", Bool(x), Bool(y)) => Bool(*x && *y),
        ("||", Bool(x), Bool(y)) => Bool(*x || *y),
        _ => Null,
    })
}

fn patch(state: &mut BTreeMap<String, Value>, path: &[String], v: Value) {
    if path.len() == 1 {
        state.insert(path[0].clone(), v);
        return;
    }
    let head = path[0].clone();
    let mut inner = match state.get(&head) {
        Some(Value::Map(m)) => m.clone(),
        Some(Value::Credential { claims, .. }) => claims.clone(),
        _ => BTreeMap::new(),
    };
    patch(&mut inner, &path[1..], v);
    match state.get(&head) {
        Some(Value::Credential { ty, verified, .. }) => {
            let (ty, verified) = (ty.clone(), verified.clone());
            state.insert(head, Value::Credential { ty, claims: inner, verified });
        }
        _ => {
            state.insert(head, Value::Map(inner));
        }
    }
}
