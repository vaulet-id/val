//! The typed-AST-to-be. Types are not resolved here — that is the next pass —
//! but everything a check needs to ask about is on the tree with its span.

use crate::diag::Span;

#[derive(Debug, Default, Clone)]
pub struct Program {
    pub app: Option<String>,
    pub version: Option<String>,
    pub capabilities: Vec<Capability>,
    /// Who this application opens for at all. Empty for almost every
    /// application, and the whole of what some of them are.
    pub admits: Vec<Admit>,
    pub admits_span: Span,
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
    /// The props that hold an action, as the host's registry names them —
    /// `onTap`, and `onRemove` on a list. Filled by the pass that reads the
    /// registry and read by the one that asks whether a press names something
    /// that exists, which has no registry of its own.
    ///
    /// It was the string `"onTap"`, written in three places, so `onRemove` named
    /// an action nothing declared and nobody said so.
    pub handlers: Vec<String>,
    /// The capabilities that cannot be taken back, as the host's registry
    /// declares them. Read where an effect is built, so what "irreversible"
    /// means is answered in one place.
    pub irreversible: Vec<String>,
    /// Every `//` line in the file, where it was. Held on the program rather
    /// than on the nodes: a comment belongs to a position, and which node it is
    /// about is a guess the printer makes rather than a fact the parser has.
    pub comments: Vec<crate::lex::Comment>,
    /// The lines that held nothing but space. A blank line is the author's
    /// grouping, and it is kept as "the line above was empty" rather than as a
    /// distance: the printer changes how many lines a thing takes.
    pub blank_lines: std::collections::BTreeSet<u32>,
    /// Every block the parser opened, innermost last when they are filtered to
    /// a position. What an editor asks instead of counting braces.
    pub scopes: Vec<Scope>,
    /// Where `app`, `version` and `state` were written. The other declarations
    /// carry their own; these three are held on the program, and a printer that
    /// did not know where they were could not put a file back in the order
    /// somebody wrote it.
    pub app_span: Span,
    pub version_span: Span,
    pub state_span: Span,
    pub capabilities_span: Span,
}

impl Program {
    /// Who answers a query, as opposed to what was asked of them.
    ///
    /// `broker.quotes(…)` is an operation on `broker.co.th`, and the audience is
    /// the one fixed in the manifest — reporting the head of the call as a
    /// party would be a lie about how many people see this. Held here because
    /// three passes ask it and three answers is how they come to disagree.
    pub fn audience_for(&self, head: &str) -> String {
        let declared: Vec<&str> = self
            .capabilities
            .iter()
            .filter(|c| c.name == "api.query")
            .filter_map(|c| {
                c.args.iter().find(|a| a.name.as_deref() == Some("audience")).and_then(|a| {
                    match &a.value {
                        Expr::Str { value, .. } => Some(value.as_str()),
                        _ => None,
                    }
                })
            })
            .collect();
        match declared.as_slice() {
            [only] => (*only).to_string(),
            _ => head.to_string(),
        }
    }
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

/// A credential somebody must hold before this application opens.
///
/// **The host decides this, not the module.** "You do not hold one" is an
/// answer only the wallet can give — a module that asked would be asking for
/// the credential in order to learn it was absent, which is the read the gate
/// exists instead of. So the compiler records the line and emits the `check`
/// import that names it, and the host resolves it before the first screen.
///
/// It is a check and never a read: the application learns that the door opened
/// and nothing else about what opened it.
#[derive(Debug, Clone)]
pub struct Admit {
    /// `EmployeeBadge` — a credential this package declared.
    pub credential: String,
    /// The policy it is checked against. Not optional: a gate that accepted
    /// anything shaped like a badge is a gate that accepts a badge somebody
    /// made.
    pub policy: String,
    /// What the person is told when they do not hold one, as a key in the
    /// signed text bundle. Also not optional — a door that closes without
    /// saying why is a fault report, and the words are the publisher's.
    pub phrase: String,
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
    /// What this credential **is**, as the wallet holding one knows it: the
    /// `vct` its issuer stamped into every card of the kind.
    ///
    /// `EmployeeBadge` is a name this package chose, and no wallet has ever
    /// heard it. Without this line a package could declare a credential and
    /// nothing in the world could tell which of somebody's cards it meant — so
    /// it is required on a `credential` and absent on a `type`, which is a
    /// record nobody signed.
    pub vct: String,
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
    /// Where `anchor:` was written, so a comment above it stays above it.
    pub anchor_span: Span,
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
    /// `credentials of PurchaseReceipt verified with P order by purchased_at desc limit 50`
    ///
    /// `order` is the claim to sort on and whether it descends. It was parsed
    /// and thrown away, so a screen that asked for its receipts newest first
    /// got whatever order the host happened to answer in.
    Credentials {
        ty: String,
        policy: Option<String>,
        order: Option<(String, bool)>,
        limit: Option<i64>,
    },
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

/// A block the parser opened, and where it ran from and to.
///
/// **What a program that is still being typed can answer.** An editor asking
/// "what may I write here" was reading braces and indentation, which is a guess
/// about the grammar made by something that does not have one. The parser knows
/// what it opened; this is it saying so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub kind: ScopeKind,
    /// What it is called, where that means anything: the action's name, the
    /// node's kind, the phase.
    pub name: String,
    /// The `{` and the `}`. A block nobody closed runs to the end of the file,
    /// which is what a program being typed looks like.
    pub from: Span,
    pub to: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Capabilities,
    Enum,
    /// A credential, a type, or `state` — the three that hold fields.
    Fields,
    Trust,
    Requires,
    Function,
    Action,
    Phase,
    Screen,
    ScreenData,
    Compute,
    Component,
    /// A run of drawn things: an `if`/`else` branch, or a component's body.
    /// A screen's own tree is the screen's block; a node's is the node's.
    Tree,
    /// The block on a node, which holds its props and its children.
    Node,
    Statements,
    Switch,
    Record,
}

impl ScopeKind {
    /// The name this block goes by outside Rust — in a report, and in the
    /// editor that asks where the cursor is. Written out rather than derived
    /// from the variant, so renaming a variant cannot silently rename the
    /// thing an editor matches on.
    pub fn as_str(self) -> &'static str {
        match self {
            ScopeKind::Capabilities => "capabilities",
            ScopeKind::Enum => "enum",
            ScopeKind::Fields => "fields",
            ScopeKind::Trust => "trust",
            ScopeKind::Requires => "require",
            ScopeKind::Function => "function",
            ScopeKind::Action => "action",
            ScopeKind::Phase => "phase",
            ScopeKind::Screen => "screen",
            ScopeKind::ScreenData => "data",
            ScopeKind::Compute => "compute",
            ScopeKind::Component => "component",
            ScopeKind::Tree => "tree",
            ScopeKind::Node => "node",
            ScopeKind::Statements => "statements",
            ScopeKind::Switch => "switch",
            ScopeKind::Record => "record",
        }
    }
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
    Destructure { names: Vec<String>, value: Expr, mutable: bool, span: Span },
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
    /// `100_000`. `text` is what was written, because the separators are the
    /// author's and a formatter that dropped them would be reformatting the
    /// number rather than the file.
    Num { value: i64, text: String, span: Span },
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
            // The leaves, written out. A catch-all here is how a variant added
            // later comes to be skipped by every pass that walks expressions,
            // and nothing says so.
            Num { .. } | Float { .. } | Str { .. } | Bool { .. } | Ident { .. } | Error { .. } => {}
        }
    }
}

impl Stmt {
    /// This statement and everything inside it.
    ///
    /// **The one answer to what a statement contains.** Four passes each had
    /// their own, and three of them stopped at the first `if`: an effect written
    /// in a branch never reached the capability report, so a person's consent
    /// sheet did not mention something the application does.
    pub fn walk(&self, f: &mut dyn FnMut(&Stmt)) {
        f(self);
        // Destructured rather than matched loosely, so that a statement which
        // gains a body stops this compiling instead of being walked halfway.
        match self {
            Stmt::If { cond: _, then, other, span: _ } => {
                for s in then.iter().chain(other) {
                    s.walk(f);
                }
            }
            Stmt::Effect { name: _, args: _, body, span: _ } => {
                for s in body {
                    s.walk(f);
                }
            }
            Stmt::Let { .. }
            | Stmt::Assign { .. }
            | Stmt::Destructure { .. }
            | Stmt::Expr { .. }
            | Stmt::Patch { .. }
            | Stmt::Binding { .. }
            | Stmt::Return { .. }
            | Stmt::Data { .. }
            | Stmt::Refuse { .. } => {}
        }
    }

    /// The same walk, collecting — for a pass that holds a diagnostic list
    /// while it reads, which a closure over both cannot.
    ///
    /// Written out rather than layered over `walk`: borrowing through a `dyn
    /// FnMut` loses the lifetime, and laundering it back is the kind of thing
    /// that is correct until somebody moves a line.
    pub fn flatten<'a>(stmts: &'a [Stmt], out: &mut Vec<&'a Stmt>) {
        for s in stmts {
            out.push(s);
            match s {
                Stmt::If { cond: _, then, other, span: _ } => {
                    Stmt::flatten(then, out);
                    Stmt::flatten(other, out);
                }
                Stmt::Effect { name: _, args: _, body, span: _ } => Stmt::flatten(body, out),
                Stmt::Let { .. }
                | Stmt::Assign { .. }
                | Stmt::Destructure { .. }
                | Stmt::Expr { .. }
                | Stmt::Patch { .. }
                | Stmt::Binding { .. }
                | Stmt::Return { .. }
                | Stmt::Data { .. }
                | Stmt::Refuse { .. } => {}
            }
        }
    }
}

impl UiNode {
    /// This node and everything under it, both halves of an `if` included.
    ///
    /// **The one answer to what a node contains.** There were twenty-three
    /// places that reached for `children` directly, and adding a second list to
    /// the type — the other branch of an `if` — broke six of them silently,
    /// because a field nobody reads is not a compile error.
    pub fn walk(&self, f: &mut dyn FnMut(&UiNode)) {
        let UiNode { kind: _, args: _, lambda: _, children, otherwise, slots: _, span: _ } = self;
        f(self);
        for c in children.iter().chain(otherwise) {
            c.walk(f);
        }
    }

    pub fn walk_mut(&mut self, f: &mut dyn FnMut(&mut UiNode)) {
        f(self);
        let UiNode { kind: _, args: _, lambda: _, children, otherwise, slots: _, span: _ } = self;
        for c in children.iter_mut().chain(otherwise) {
            c.walk_mut(f);
        }
    }

    /// Every node in a run of siblings.
    pub fn walk_all(nodes: &[UiNode], f: &mut dyn FnMut(&UiNode)) {
        for n in nodes {
            n.walk(f);
        }
    }

    pub fn walk_all_mut(nodes: &mut [UiNode], f: &mut dyn FnMut(&mut UiNode)) {
        for n in nodes {
            n.walk_mut(f);
        }
    }
}
