//! The typed-AST-to-be. Types are not resolved here — that is the next pass —
//! but everything a check needs to ask about is on the tree with its span.

use crate::diag::Span;

#[derive(Debug, Default)]
pub struct Program {
    pub app: Option<String>,
    pub version: Option<String>,
    pub capabilities: Vec<Capability>,
    pub enums: Vec<EnumDecl>,
    pub credentials: Vec<CredentialDecl>,
    pub state: Vec<Field>,
    pub trusts: Vec<TrustDecl>,
    pub functions: Vec<FunctionDecl>,
    pub actions: Vec<ActionDecl>,
    pub screens: Vec<ScreenDecl>,
}

#[derive(Debug, Clone)]
pub struct Capability {
    /// `credential.read`, `disclosure.present`, `api.query`
    pub name: String,
    pub args: Vec<Arg>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Arg {
    pub name: Option<String>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub members: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CredentialDecl {
    pub name: String,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeRef,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRef {
    pub name: String,
    pub args: Vec<TypeRef>,
    pub optional: bool,
}

#[derive(Debug, Clone)]
pub struct TrustDecl {
    pub name: String,
    pub subject: String,
    pub subject_type: String,
    pub refines: Option<String>,
    pub anchor: Option<String>,
    pub requires: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Field>,
    pub ret: Option<TypeRef>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Input,
    Require,
    Verify,
    Compute,
    Update,
    Execute,
}

impl Phase {
    pub fn from_str(s: &str) -> Option<Phase> {
        Some(match s {
            "input" => Phase::Input,
            "require" => Phase::Require,
            "verify" => Phase::Verify,
            "compute" => Phase::Compute,
            "update" => Phase::Update,
            "execute" => Phase::Execute,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Phase::Input => "input",
            Phase::Require => "require",
            Phase::Verify => "verify",
            Phase::Compute => "compute",
            Phase::Update => "update",
            Phase::Execute => "execute",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PhaseBlock {
    pub phase: Phase,
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ActionDecl {
    pub name: String,
    pub phases: Vec<PhaseBlock>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ScreenDecl {
    pub name: String,
    pub data: Vec<DataDecl>,
    pub compute: Vec<Stmt>,
    pub tree: Vec<UiNode>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DataDecl {
    pub name: String,
    pub source: DataSource,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DataSource {
    Credentials { ty: String, policy: Option<String>, limit: Option<i64> },
    Query { audience: String },
    Unknown,
}

#[derive(Debug, Clone)]
pub struct UiNode {
    pub kind: String,
    pub args: Vec<Arg>,
    pub children: Vec<UiNode>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    /// `const x = …`
    Let { name: String, value: Expr, span: Span },
    /// A bare predicate in `require` or `verify`
    Expr { value: Expr, span: Span },
    /// `member.tier: tier` in `update` — a patch, not an assignment
    Patch { path: Vec<String>, value: Expr, span: Span },
    /// `receipt: Credential<PurchaseReceipt>` in `input`
    Binding { name: String, ty: TypeRef, span: Span },
    /// `credential.issue(…)`, `present { … }`
    Effect { name: String, args: Vec<Arg>, body: Vec<Stmt>, span: Span },
    /// `return …`
    Return { value: Expr, span: Span },
    /// `const holdings = credentials of Holding verified with Policy`
    Data { name: String, source: DataSource, span: Span },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. }
            | Stmt::Expr { span, .. }
            | Stmt::Patch { span, .. }
            | Stmt::Binding { span, .. }
            | Stmt::Effect { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Data { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Num { value: i64, span: Span },
    /// Kept so the checker can say "use satang" rather than the parser guessing.
    Float { text: String, span: Span },
    Str { value: String, span: Span },
    Bool { value: bool, span: Span },
    Ident { name: String, span: Span },
    Member { obj: Box<Expr>, name: String, span: Span },
    Call { callee: Box<Expr>, args: Vec<Arg>, span: Span },
    Unary { op: String, rhs: Box<Expr>, span: Span },
    Binary { op: String, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    Ternary { cond: Box<Expr>, then: Box<Expr>, other: Box<Expr>, span: Span },
    /// `receipt with ReceiptFromMerchant` — the only introduction of Verified<P>
    With { subject: Box<Expr>, policy: String, span: Span },
    Exists { subject: Box<Expr>, span: Span },
    Record { spread: Option<Box<Expr>>, fields: Vec<(String, Expr)>, span: Span },
    Switch { subject: Box<Expr>, arms: Vec<SwitchArm>, span: Span },
    Lambda { params: Vec<String>, body: Box<Expr>, span: Span },
    /// `points: … from { Policy }`
    From { value: Box<Expr>, policies: Vec<String>, span: Span },
    Error { span: Span },
}

#[derive(Debug, Clone)]
pub struct SwitchArm {
    pub pattern: ArmPattern,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArmPattern {
    /// `Tier.gold =>`
    Value(Expr),
    /// `>= 10_000 =>`
    Compare { op: String, rhs: Expr },
    /// `default =>`
    Default,
}

impl Expr {
    pub fn span(&self) -> Span {
        use Expr::*;
        match self {
            Num { span, .. } | Float { span, .. } | Str { span, .. } | Bool { span, .. } | Ident { span, .. }
            | Member { span, .. } | Call { span, .. } | Unary { span, .. } | Binary { span, .. }
            | Ternary { span, .. } | With { span, .. } | Exists { span, .. } | Record { span, .. }
            | Switch { span, .. } | Lambda { span, .. } | From { span, .. } | Error { span } => *span,
        }
    }

    /// `state.member.points` as a readable path, where it is one.
    pub fn path(&self) -> Option<String> {
        match self {
            Expr::Ident { name, .. } => Some(name.clone()),
            Expr::Member { obj, name, .. } => Some(format!("{}.{}", obj.path()?, name)),
            _ => None,
        }
    }

    pub fn walk(&self, f: &mut dyn FnMut(&Expr)) {
        f(self);
        use Expr::*;
        match self {
            Member { obj, .. } => obj.walk(f),
            Call { callee, args, .. } => {
                callee.walk(f);
                for a in args {
                    a.value.walk(f);
                }
            }
            Unary { rhs, .. } => rhs.walk(f),
            Binary { lhs, rhs, .. } => {
                lhs.walk(f);
                rhs.walk(f)
            }
            Ternary { cond, then, other, .. } => {
                cond.walk(f);
                then.walk(f);
                other.walk(f)
            }
            With { subject, .. } | Exists { subject, .. } => subject.walk(f),
            Record { spread, fields, .. } => {
                if let Some(s) = spread {
                    s.walk(f)
                }
                for (_, v) in fields {
                    v.walk(f)
                }
            }
            Switch { subject, arms, .. } => {
                subject.walk(f);
                for a in arms {
                    if let ArmPattern::Value(v) = &a.pattern {
                        v.walk(f)
                    }
                    if let ArmPattern::Compare { rhs, .. } = &a.pattern {
                        rhs.walk(f)
                    }
                    a.body.walk(f);
                }
            }
            Lambda { body, .. } => body.walk(f),
            From { value, .. } => value.walk(f),
            _ => {}
        }
    }
}
