//! The typed-AST-to-be. Types are not resolved here — that is the next pass —
//! but everything a check needs to ask about is on the tree with its span.

use crate::diag::Span;

#[derive(Debug, Default, Clone)]
pub struct Program {
    pub app: Option<String>,
    pub version: Option<String>,
    pub capabilities: Vec<Capability>,
    pub enums: Vec<EnumDecl>,
    pub credentials: Vec<CredentialDecl>,
    /// Plain records. The same shape as a credential's claims and none of the
    /// four faces, because nobody signed one — which is the whole difference.
    pub types: Vec<CredentialDecl>,
    pub state: Vec<Field>,
    pub trusts: Vec<TrustDecl>,
    pub functions: Vec<FunctionDecl>,
    pub actions: Vec<ActionDecl>,
    pub screens: Vec<ScreenDecl>,
    /// Compositions the package wrote itself. A component is a name for a piece
    /// of the catalogue arranged a particular way — it adds no primitive and no
    /// rendering path, because it expands into the catalogue before a host sees
    /// it.
    pub components: Vec<ComponentDecl>,
    /// Components taken from other packages. Resolved and expanded before any
    /// other pass, so what an imported component draws lands in this package's
    /// capability report — a person consents to one list, not to one per
    /// package that happened to be involved.
    pub imports: Vec<ImportDecl>,
    /// The hosts whose registries this package needs, as `name/version`. A
    /// host that does not provide one refuses the package rather than
    /// approximating it.
    pub hosts: Vec<String>,
    /// Capabilities used by drawing something that needs them — `video` needs
    /// `media.video`. Filled in by the pass that reads the host's registry, and
    /// read by the one that asks whether a declared capability goes unused.
    pub uses: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComponentDecl {
    pub name: String,
    /// `export component MoneyCard(…)`. What leaves the package.
    ///
    /// A component without it is visible to every file in the package, because
    /// a package's files share one scope — the boundary this crosses is the
    /// package, never the file.
    pub exported: bool,
    /// Named at the call site, like every other call with more than one
    /// argument. A component takes values and an action to call; it cannot
    /// declare state or data of its own.
    pub params: Vec<Field>,
    pub tree: Vec<UiNode>,
    pub span: Span,
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
    /// `...style`, where `style` names a record. It expands to one named
    /// argument per field of that record's declared type, so what arrives is
    /// knowable from the type rather than from whatever a caller happened to
    /// pass. A value that is not a declared record cannot be spread here.
    pub spread: bool,
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

impl TypeRef {
    /// The type as somebody would write it — `List(string)`, `int?`.
    ///
    /// Used where a person reads it rather than where the compiler does: the
    /// exported surface in the capability report is a thing a publisher diffs
    /// against what they published last time.
    pub fn written(&self) -> String {
        let mut out = self.name.clone();
        if !self.args.is_empty() {
            let args: Vec<String> = self.args.iter().map(|a| a.written()).collect();
            out = format!("{out}({})", args.join(", "));
        }
        if self.optional {
            out.push('?');
        }
        out
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    /// What the person reads at the top. A sentence rather than the screen's
    /// identifier: identifiers are ASCII, so a title taken from one could never
    /// be Thai, and every other word on the screen can be.
    ///
    /// Held as a node so it goes through the same flattening and the same
    /// bundle check as every other sentence, rather than being a second path
    /// that has to be kept faithful to the first.
    pub title: Option<UiNode>,
    /// `@main` — what is written above the declaration.
    pub directives: Vec<Directive>,
    /// `present: sheet`, `address: "receipt/{id}"` — props a capability gives a
    /// screen. The parser learns the shape; which props exist and what words
    /// they take is read from the host's interfaces.
    pub settings: Vec<Arg>,
    /// What this screen is handed when something moves to it, declared the way
    /// a component declares its parameters.
    pub params: Vec<Field>,
    pub data: Vec<DataDecl>,
    pub compute: Vec<Stmt>,
    pub tree: Vec<UiNode>,
    pub span: Span,
}

impl ScreenDecl {
    /// Whether this is the screen the application opens on.
    pub fn is_main(&self) -> bool {
        self.directives.iter().any(|d| d.name == "main")
    }
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
    /// `list(receipts) { r -> … }` — what a row is bound to. The host needs it
    /// to resolve a slot inside the row, and it is the only place a name is
    /// introduced by the interface rather than by the program.
    pub lambda: Option<String>,
    pub children: Vec<UiNode>,
    /// The names that came from this node's `slots { … }` block, kept after the
    /// block itself is flattened into ordinary arguments. The compiler compares
    /// them with the placeholders in the sentence this node names: a slot the
    /// sentence does not have, or a placeholder nothing fills, is a failed
    /// build.
    pub slots: Vec<String>,
    /// The second branch of an `if` in a screen's tree. Held on the node rather
    /// than as a node of its own, so that everything which walks a tree walks
    /// both halves without having to be taught a new shape.
    pub otherwise: Vec<UiNode>,
    pub span: Span,
}

/// `import "org.vaulet.ui/1" { MoneyCard, Chip }`.
///
/// Named the way every other external thing is named in this language — a
/// quoted identifier with a version, as `host "id.vaulet.wallet/1"` is. A
/// package is a signed artifact rather than a namespace, so what is imported
/// from is a version and not a scope.
///
/// The names are listed rather than opened wholesale: what crossed into a
/// package is then one line to read, which matters because everything that
/// crossed is signed as part of it.
#[derive(Debug, Clone)]
pub struct ImportDecl {
    /// `org.vaulet.ui/1`, as written.
    pub package: String,
    pub names: Vec<String>,
    pub span: Span,
}

/// `@main`, `@name(argument)` — a mark on a declaration.
///
/// Distinct from a setting, which configures the thing it is written on and
/// takes a value from the host's vocabulary. A directive says something about
/// the declaration's place in the package: which screen opens it. That is why
/// it sits above the declaration rather than among its props, and why the set
/// of them is the language's rather than a host's.
#[derive(Debug, Clone)]
pub struct Directive {
    pub name: String,
    /// Empty for a directive that only marks. The syntax carries arguments so
    /// that the first directive which needs one is a row in a table rather than
    /// a second shape.
    pub args: Vec<Arg>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    /// `const x = …`, and `let x = …` where `mutable` is set.
    ///
    /// A `const` is a definition: what the name means does not depend on how
    /// far down the block a reader has got. A `let` is a variable, and exists
    /// because most people arrive already knowing what one is.
    Let { name: String, value: Expr, mutable: bool, span: Span },
    /// `x = …`, to a name that was declared `let`.
    Assign { name: String, value: Expr, span: Span },
    /// `const { merchant, amount } = row`
    ///
    /// One statement rather than one per name, so the right-hand side is read
    /// once. Written out as several would read a credential once per field.
    Destructure { names: Vec<String>, value: Expr, span: Span },
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
    /// `refuse "notEnoughPoints"` — the application declining for its own
    /// reasons. An ordinary outcome, not a defect: the person is told, and
    /// what they are told comes from the signed text bundle rather than from a
    /// sentence assembled here.
    Refuse { key: String, span: Span },
    /// `if (cond) { … } else { … }`. A statement, never an expression — the
    /// expression form is `?:`, and two ways to write one thing is what this
    /// language spends its budget avoiding (§3).
    If { cond: Expr, then: Vec<Stmt>, other: Vec<Stmt>, span: Span },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::Destructure { span, .. }
            | Stmt::Expr { span, .. }
            | Stmt::Patch { span, .. }
            | Stmt::Binding { span, .. }
            | Stmt::Effect { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Data { span, .. }
            | Stmt::If { span, .. }
            | Stmt::Refuse { span, .. } => *span,
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
    /// `a.b`, and `a?.b` where `optional` is set: the whole path is nothing when
    /// the left side is, rather than a failure.
    Member { obj: Box<Expr>, name: String, optional: bool, span: Span },
    Call { callee: Box<Expr>, args: Vec<Arg>, span: Span },
    Unary { op: String, rhs: Box<Expr>, span: Span },
    Binary { op: String, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    Ternary { cond: Box<Expr>, then: Box<Expr>, other: Box<Expr>, span: Span },
    /// `receipt with ReceiptFromMerchant` — the only introduction of Verified<P>
    With { subject: Box<Expr>, policy: String, span: Span },
    Exists { subject: Box<Expr>, span: Span },
    /// `a ?: b` — the left side unless it is nothing.
    ///
    /// Not a ternary over `exists`, which would evaluate the left side twice —
    /// once to ask and once to answer — and in a language where a path may
    /// reach into a credential that is a second lookup nobody wrote.
    Elvis { subject: Box<Expr>, other: Box<Expr>, span: Span },
    Record { spread: Option<Box<Expr>>, fields: Vec<(String, Expr)>, span: Span },
    /// `["merchant", "amount"]` — a list written out. The columns of a table
    /// and the options of a picker are lists somebody types, and the language
    /// had no way to say one: every list came from the wallet or from a
    /// combinator over one.
    List { items: Vec<Expr>, span: Span },
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
            | Ternary { span, .. } | With { span, .. } | Exists { span, .. } | Elvis { span, .. } | Record { span, .. }
            | List { span, .. }
            | Switch { span, .. } | Lambda { span, .. } | From { span, .. } | Error { span } => *span,
        }
    }

    /// `state.member.points` as a readable path, where it is one.
    pub fn path(&self) -> Option<String> {
        match self {
            Expr::Ident { name, .. } => Some(name.clone()),
            Expr::Member { obj, name, .. } => Some(format!("{}.{}", obj.path()?, name)),
            // `Receipt(merchant: …)` — what a press names is the callee, and
            // the arguments are what it is opened with. Without this a press
            // that hands something over was a press that named nothing.
            Expr::Call { callee, .. } => callee.path(),
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
            Elvis { subject, other, .. } => {
                subject.walk(f);
                other.walk(f)
            }
            List { items, .. } => {
                for i in items {
                    i.walk(f);
                }
            }
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
