//! Typed AST to Wasm.
//!
//! Every VAL function becomes a Wasm function taking and returning `i32`
//! handles. Arithmetic, comparison and field access are imported from the host,
//! so this file emits `call`, `if`, `local.get` and constants — and that is the
//! whole of it.

use std::collections::BTreeMap;

use valang::ast::*;
use wasm_encoder::{
    CodeSection, CustomSection, ExportSection, Function, FunctionSection, ImportSection, Instruction,
    Module as Enc, TypeSection, ValType,
};

/// The constants a module needs, in the order the module refers to them. The
/// host builds real values from these before it runs anything, so no literal
/// crosses the boundary at run time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Konst {
    Int(i64),
    Str(String),
    Bool(bool),
    /// `Tier.gold`
    Enum(String, String),
    /// `[]` and `{}`. A collection is built from an empty one and added to,
    /// because the host owns values and a module has no allocator.
    EmptyList,
    EmptyRecord,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub bytes: Vec<u8>,
    pub konsts: Vec<Konst>,
    /// Exported function names, in the order they were compiled.
    pub functions: Vec<String>,
}

/// The imports every emitted module expects, in a fixed order. A host that
/// supplies them in another order is a host running a different language.
pub const IMPORTS: &[(&str, u32)] = &[
    ("konst", 1),  // (index) -> handle
    ("add", 2),
    ("sub", 2),
    ("mul", 2),
    ("div", 2),
    ("rem", 2),
    ("lt", 2),
    ("le", 2),
    ("gt", 2),
    ("ge", 2),
    ("eq", 2),
    ("ne", 2),
    ("and", 2),
    ("or", 2),
    ("not", 1),
    ("neg", 1),
    ("truthy", 1), // (handle) -> i32, the only place a value becomes control flow
    ("field", 2),  // (handle, name-konst) -> handle
    ("exists", 1), // (handle) -> handle, a bool: whether it is there at all
    // Lists and records are built by the host, because the host owns values.
    // An empty one comes from `konst`; these add to it.
    ("push", 2),   // (list, item) -> list
    ("set", 3),    // (record, name-konst, value) -> record
    // Walking a list. `len` and `at` take and give a raw index rather than a
    // handle, because a loop counter is the one number the module itself
    // holds: everything else is the host's.
    ("len", 1),    // (list) -> i32
    ("at", 2),     // (list, i32) -> handle
    ("count", 1),  // (list) -> handle, the length as a value
    ("first", 1),  // (list) -> handle, or nothing
];

/// The list operations, and how many values each hands the function it is given.
const COMBINATORS: &[(&str, usize)] =
    &[("map", 1), ("filter", 1), ("any", 1), ("all", 1), ("fold", 2), ("count", 0), ("first", 0)];

fn import_index(name: &str) -> u32 {
    IMPORTS.iter().position(|(n, _)| *n == name).expect("unknown import") as u32
}

struct Ctx<'a> {
    program: &'a Program,
    konsts: Vec<Konst>,
    /// Everything this module imports beyond the fixed operations —
    /// `(namespace, name)` to how many handles it takes. Filled on the first
    /// pass and indexed on the second: an index cannot be known until every
    /// import is, and a call has to name one. A `BTreeMap`, because two builds
    /// of one source must be one module.
    dynamic: BTreeMap<(&'static str, String), usize>,
    /// Where each of them landed, once they were all known.
    dyn_index: BTreeMap<(&'static str, String), u32>,
    /// Which pass this is. The same emit code runs twice — once into a buffer
    /// nobody keeps, to find out what is imported, and once for real. A second
    /// walk that decided for itself what to collect would be a second answer to
    /// what this module does, and the first thing it would get wrong is the
    /// report.
    collecting: bool,
    locals: BTreeMap<String, u32>,
    /// Function index in the module: imports first, then VAL functions.
    fn_index: BTreeMap<String, u32>,
    /// Which phase is being lowered. A predicate that does not hold means
    /// different things in `require` and in `verify` — a defect in this program
    /// against a credential that did not satisfy its policy — and the outcome
    /// is the difference.
    phase: Option<Phase>,
    /// Names bound by `x with Policy`, to the credential type behind the
    /// policy. Built while lowering rather than by a walk of its own: what an
    /// import is called has to come from the same pass that emits the call.
    verified: BTreeMap<String, String>,
    /// What this back end met and does not emit.
    unsupported: Vec<String>,
    /// The next scratch slot, for the expressions that need somewhere to put a
    /// value they read once and use twice.
    next_scratch: u32,
    scratch_used: u32,
}

impl Ctx<'_> {
    fn konst(&mut self, k: Konst) -> u32 {
        if let Some(i) = self.konsts.iter().position(|x| *x == k) {
            return i as u32;
        }
        self.konsts.push(k);
        (self.konsts.len() - 1) as u32
    }

    fn push_konst(&mut self, k: Konst, f: &mut Function) {
        let i = self.konst(k);
        f.instruction(&Instruction::I32Const(i as i32));
        f.instruction(&Instruction::Call(import_index("konst")));
    }

    /// A slot nothing else is using. Nested expressions each get their own:
    /// sharing one would make `a ?: (b ?: c)` read the inner value into the
    /// outer one's slot.
    fn scratch(&mut self) -> u32 {
        let slot = self.next_scratch;
        self.next_scratch += 1;
        self.scratch_used = self.scratch_used.max(self.next_scratch);
        slot
    }

    fn call_op(&mut self, op: &str, f: &mut Function) {
        f.instruction(&Instruction::Call(import_index(op)));
    }

    /// Call something outside the fixed table. On the collecting pass this only
    /// records that the module needs it; the index it emits then is discarded
    /// with the body it emitted into.
    fn call_dyn(&mut self, ns: &'static str, name: String, arity: usize, f: &mut Function) {
        let key = (ns, name);
        if self.collecting {
            self.dynamic.insert(key, arity);
            f.instruction(&Instruction::Call(0));
            return;
        }
        let index = self.dyn_index[&key];
        f.instruction(&Instruction::Call(index));
    }

    fn call_cap(&mut self, cap: &crate::abi::Cap, f: &mut Function) {
        self.call_dyn(crate::abi::CAPS, cap.name(), cap.arity(), f);
    }

    fn call_val(&mut self, op: &crate::abi::Op, arity: usize, f: &mut Function) {
        self.call_dyn(crate::abi::OPS, op.name(), arity, f);
    }

    /// The credential type behind a name bound by `x with Policy`, which is what
    /// a report calls a claim read through it: the author wrote
    /// `checked.claims.amount` and the person is being told `PurchaseReceipt`.
    fn verified_type(&self, binding: &str) -> Option<String> {
        self.verified.get(binding).cloned()
    }
}

/// Compile every `function` in the program. Actions are not compiled: their
/// phases are the host's business, and `execute` describes effects rather than
/// performing them, so there is nothing there for a module to do.
/// The typed AST, as a module — or the shapes this back end does not emit.
///
/// It used to push `false` for anything it did not recognise, so a function
/// using something added to the language since compiled to a module that
/// computed a wrong answer and said nothing. A back end that silently disagrees
/// with the other one is worse than a back end that is missing: the parity test
/// is what says the two agree, and it can only run on what both of them have.
pub fn compile_function(program: &Program) -> Result<Module, Vec<String>> {
    compile(program, false)
}

/// The whole program: its functions, and its actions.
///
/// **This is the artifact.** A wallet downloads it, reads its imports to find
/// out what it can do, shows the person that, and runs it. There is no compiler
/// on the other end and no source — so what a module imports has to be the
/// whole truth about it, which is why an effect this back end cannot emit is an
/// error here rather than a body that quietly does less.
pub fn compile_program(program: &Program) -> Result<Module, Vec<String>> {
    compile(program, true)
}

fn compile(program: &Program, actions: bool) -> Result<Module, Vec<String>> {
    let mut ctx = Ctx {
        program,
        konsts: Vec::new(),
        dynamic: BTreeMap::new(),
        dyn_index: BTreeMap::new(),
        collecting: false,
        phase: None,
        verified: BTreeMap::new(),
        locals: BTreeMap::new(),
        fn_index: BTreeMap::new(),
        unsupported: Vec::new(),
        next_scratch: 0,
        scratch_used: 0,
    };

    // The first pass emits every body into buffers nobody keeps, to find out
    // what this module imports. Indices cannot be handed out before that: an
    // import section is one list, and a call names a position in it.
    ctx.collecting = true;
    for f in &program.functions {
        ctx.fn_index.insert(f.name.clone(), 0);
    }
    for f in &program.functions {
        body_of(&mut ctx, &f.params, &f.body);
    }
    if actions {
        for a in &program.actions {
            action_body(&mut ctx, a);
        }
        for s in &program.screens {
            screen_data(&mut ctx, s);
        }
    }

    let fixed = IMPORTS.len() as u32;
    let dynamic: Vec<((&'static str, String), usize)> =
        ctx.dynamic.iter().map(|(k, v)| (k.clone(), *v)).collect();
    for (i, (key, _)) in dynamic.iter().enumerate() {
        ctx.dyn_index.insert(key.clone(), fixed + i as u32);
    }
    ctx.collecting = false;

    let import_count = fixed + dynamic.len() as u32;
    for (i, f) in program.functions.iter().enumerate() {
        ctx.fn_index.insert(f.name.clone(), import_count + i as u32);
    }

    let mut types = TypeSection::new();
    // Type 0: (i32) -> i32. Type 1: (i32, i32) -> i32.
    types.ty().function([ValType::I32], [ValType::I32]);
    types.ty().function([ValType::I32, ValType::I32], [ValType::I32]);
    // Type 2..: one per VAL function arity.
    let mut arity_type: BTreeMap<usize, u32> = BTreeMap::from([(1, 0), (2, 1)]);
    // An import of an arity nothing else uses still needs a type.
    for (_, arity) in IMPORTS {
        let n = *arity as usize;
        if !arity_type.contains_key(&n) {
            let id = types.len();
            types.ty().function(vec![ValType::I32; n], [ValType::I32]);
            arity_type.insert(n, id);
        }
    }
    for f in &program.functions {
        let n = f.params.len();
        if !arity_type.contains_key(&n) {
            let id = types.len();
            types.ty().function(vec![ValType::I32; n], [ValType::I32]);
            arity_type.insert(n, id);
        }
    }
    // An action takes nothing: what it needs, it asks the host for.
    for (_, arity) in &dynamic {
        if !arity_type.contains_key(arity) {
            let id = types.len();
            types.ty().function(vec![ValType::I32; *arity], [ValType::I32]);
            arity_type.insert(*arity, id);
        }
    }
    if !arity_type.contains_key(&0) {
        let id = types.len();
        types.ty().function([], [ValType::I32]);
        arity_type.insert(0, id);
    }

    let mut imports = ImportSection::new();
    for (name, arity) in IMPORTS {
        let ty = arity_type[&(*arity as usize)];
        imports.import(crate::abi::OPS, name, wasm_encoder::EntityType::Function(ty));
    }
    // Then everything the program itself reaches for, in the order the map
    // holds them — sorted, so one source is one module.
    for ((ns, name), arity) in &dynamic {
        imports.import(ns, name.as_str(), wasm_encoder::EntityType::Function(arity_type[arity]));
    }

    let mut funcs = FunctionSection::new();
    let mut code = CodeSection::new();
    let mut exports = ExportSection::new();
    let mut names = Vec::new();

    for (i, f) in program.functions.iter().enumerate() {
        funcs.function(arity_type[&f.params.len()]);
        exports.export(&f.name, wasm_encoder::ExportKind::Func, import_count + i as u32);
        names.push(f.name.clone());
        let body = body_of(&mut ctx, &f.params, &f.body);
        code.function(&body);
    }

    if actions {
        for (i, a) in program.actions.iter().enumerate() {
            funcs.function(arity_type[&0]);
            let index = import_count + (program.functions.len() + i) as u32;
            exports.export(&format!("action:{}", a.name), wasm_encoder::ExportKind::Func, index);
            names.push(format!("action:{}", a.name));
            let body = action_body(&mut ctx, a);
            code.function(&body);
        }
        for (i, s) in program.screens.iter().enumerate() {
            funcs.function(arity_type[&0]);
            let index =
                import_count + (program.functions.len() + program.actions.len() + i) as u32;
            exports.export(&format!("data:{}", s.name), wasm_encoder::ExportKind::Func, index);
            names.push(format!("data:{}", s.name));
            let body = screen_data(&mut ctx, s);
            code.function(&body);
        }
    }

    let mut module = Enc::new();
    module.section(&types);
    module.section(&imports);
    module.section(&funcs);
    module.section(&exports);
    module.section(&code);

    // The constants travel inside the module. Handing them alongside made the
    // module something that only ran next to the compiler that produced it,
    // which is not a thing anybody can ship, sign or hash on its own.
    let pool = encode_konsts(&ctx.konsts);
    module.section(&CustomSection { name: KONST_SECTION.into(), data: pool.into() });

    let about = encode_about(&valang_runtime::About::of(program));
    module.section(&CustomSection { name: ABOUT_SECTION.into(), data: about.into() });

    if !ctx.unsupported.is_empty() {
        let mut said: Vec<String> = ctx.unsupported.clone();
        said.sort();
        said.dedup();
        return Err(said
            .into_iter()
            .map(|what| format!("this back end does not emit {what}"))
            .collect());
    }

    Ok(Module { bytes: module.finish(), konsts: ctx.konsts, functions: names })
}

/// The custom section the constants live in. A custom section is ignored by any
/// Wasm runtime that does not know it, which is the right shape: a host that
/// cannot read the pool cannot run the module either, and will say so.
pub const KONST_SECTION: &str = "val.konsts";

/// What a record says about the application, carried by the module itself.
///
/// **A wallet has the bytes and no compiler.** It has to fill in an execution
/// record — which application, which version, which capabilities were declared,
/// which policies exist, and what the host is asked for before an action starts
/// — and none of that can come from a typed AST it cannot produce. So it
/// travels here, in a section beside the code, and what a wallet needs is one
/// file.
pub const ABOUT_SECTION: &str = "val.about";

/// The metadata, in the same canonical encoding as everything else this project
/// hashes — so two builds of one source are one module here as well.
fn encode_about(a: &valang_runtime::About) -> Vec<u8> {
    use std::collections::BTreeMap;
    use valang_runtime::canonical::{Canonical, DeterministicCbor};
    use valang_runtime::value::Value;

    let strings = |xs: &[String]| Value::List(xs.iter().cloned().map(Value::Str).collect());
    let mut m = BTreeMap::new();
    m.insert("app".to_string(), Value::Str(a.app.clone()));
    m.insert("version".to_string(), Value::Str(a.version.clone()));
    m.insert("capabilities".to_string(), strings(&a.capabilities));
    m.insert("policies".to_string(), strings(&a.policies));
    m.insert("state".to_string(), Value::Map(a.state.clone()));
    m.insert("fields".to_string(), strings(&a.fields));
    m.insert(
        "actions".to_string(),
        Value::List(
            a.actions
                .iter()
                .map(|act| {
                    let mut one = BTreeMap::new();
                    one.insert("name".to_string(), Value::Str(act.name.clone()));
                    one.insert(
                        "inputs".to_string(),
                        Value::List(
                            act.inputs
                                .iter()
                                .map(|d| {
                                    let mut i = BTreeMap::new();
                                    i.insert("binding".to_string(), Value::Str(d.binding.clone()));
                                    i.insert(
                                        "credential".to_string(),
                                        Value::Str(d.credential.clone()),
                                    );
                                    i.insert(
                                        "policy".to_string(),
                                        d.policy.clone().map_or(Value::Null, Value::Str),
                                    );
                                    Value::Map(i)
                                })
                                .collect(),
                        ),
                    );
                    Value::Map(one)
                })
                .collect(),
        ),
    );
    DeterministicCbor.encode(&Value::Map(m))
}

/// Read it back, from bytes and nothing else. This is the first thing a wallet
/// does with a Micro App it has been handed.
pub fn about_of(bytes: &[u8]) -> Option<valang_runtime::About> {
    use valang_runtime::decode::decode;
    use valang_runtime::value::Value;
    use valang_runtime::{About, ActionAbout, Declared};

    let data = custom_section(bytes, ABOUT_SECTION)?;
    let Value::Map(m) = decode(data).ok()? else { return None };
    let text = |v: Option<&Value>| match v {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    };
    let strings = |v: Option<&Value>| match v {
        Some(Value::List(xs)) => xs
            .iter()
            .map(|x| match x {
                Value::Str(s) => s.clone(),
                _ => String::new(),
            })
            .collect(),
        _ => Vec::new(),
    };
    let actions = match m.get("actions") {
        Some(Value::List(xs)) => xs
            .iter()
            .filter_map(|x| {
                let Value::Map(one) = x else { return None };
                let inputs = match one.get("inputs") {
                    Some(Value::List(ds)) => ds
                        .iter()
                        .filter_map(|d| {
                            let Value::Map(d) = d else { return None };
                            Some(Declared {
                                binding: text(d.get("binding")),
                                credential: text(d.get("credential")),
                                policy: match d.get("policy") {
                                    Some(Value::Str(s)) => Some(s.clone()),
                                    _ => None,
                                },
                            })
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                Some(ActionAbout { name: text(one.get("name")), inputs })
            })
            .collect(),
        _ => Vec::new(),
    };
    let state = match m.get("state") {
        Some(Value::Map(fields)) => fields.clone(),
        _ => Default::default(),
    };
    Some(About {
        state,
        fields: strings(m.get("fields")),
        app: text(m.get("app")),
        version: text(m.get("version")),
        capabilities: strings(m.get("capabilities")),
        policies: strings(m.get("policies")),
        actions,
    })
}

fn encode_konsts(ks: &[Konst]) -> Vec<u8> {
    use valang_runtime::canonical::{Canonical, DeterministicCbor};
    use valang_runtime::value::Value;

    let items: Vec<Value> = ks
        .iter()
        .map(|k| match k {
            Konst::Int(i) => Value::Int(*i),
            Konst::Str(s) => Value::Str(s.clone()),
            Konst::Bool(b) => Value::Bool(*b),
            Konst::Enum(e, m) => Value::Enum(e.clone(), m.clone()),
            Konst::EmptyList => Value::List(Vec::new()),
            Konst::EmptyRecord => Value::Map(Default::default()),
        })
        .collect();
    DeterministicCbor.encode(&Value::List(items))
}

/// Read the pool back out of a module somebody handed you.
pub fn konsts_of(bytes: &[u8]) -> Option<Vec<Konst>> {
    use valang_runtime::decode::decode;
    use valang_runtime::value::Value;

    let data = custom_section(bytes, KONST_SECTION)?;
    let Value::List(items) = decode(data).ok()? else { return None };
    items
        .into_iter()
        .map(|v| match v {
            Value::Int(i) => Some(Konst::Int(i)),
            Value::Str(s) => Some(Konst::Str(s)),
            Value::Bool(b) => Some(Konst::Bool(b)),
            Value::Enum(e, m) => Some(Konst::Enum(e, m)),
            Value::List(items) if items.is_empty() => Some(Konst::EmptyList),
            Value::Map(m) if m.is_empty() => Some(Konst::EmptyRecord),
            _ => None,
        })
        .collect()
}

fn custom_section<'a>(bytes: &'a [u8], want: &str) -> Option<&'a [u8]> {
    let mut i = 8; // magic + version
    while i < bytes.len() {
        let id = *bytes.get(i)?;
        i += 1;
        let (len, used) = leb(bytes, i)?;
        i += used;
        let body = bytes.get(i..i + len)?;
        i += len;
        if id == 0 {
            let (name_len, used) = leb(body, 0)?;
            let name = std::str::from_utf8(body.get(used..used + name_len)?).ok()?;
            if name == want {
                return body.get(used + name_len..);
            }
        }
    }
    None
}

fn leb(bytes: &[u8], mut i: usize) -> Option<(usize, usize)> {
    let (mut out, mut shift, start) = (0usize, 0u32, i);
    loop {
        let b = *bytes.get(i)?;
        i += 1;
        out |= ((b & 0x7f) as usize) << shift;
        if b & 0x80 == 0 {
            return Some((out, i - start));
        }
        shift += 7;
    }
}

fn count_lets(body: &[Stmt]) -> u32 {
    let mut n = 0;
    for s in body {
        s.walk(&mut |x| {
            n += match x {
                Stmt::Let { .. } => 1,
                // A declared input and a `data` line each name a value the
                // host answers with, and a name needs a slot to live in.
                Stmt::Binding { .. } | Stmt::Data { .. } => 1,
                // One for the record itself, and one per name taken out of it.
                Stmt::Destructure { names, .. } => 1 + names.len() as u32,
                _ => 0,
            };
        });
    }
    n
}

/// True when this body ends by returning on every path, so the caller knows
/// whether it still has to leave a value on the stack.
/// How many slots the expressions in this body want. One per `?:`, because
/// each reads its left side once and uses it twice, and a nested one may not
/// share the slot of the one it sits in.
fn count_scratch(body: &[Stmt]) -> u32 {
    let mut n = 0;
    for s in body {
        s.walk(&mut |x| {
            let mut count = |e: &Expr| {
                e.walk(&mut |inner| {
                    n += match inner {
                        Expr::Elvis { .. } => 1,
                        // A list operation holds the list, the index, the
                        // length, the row and what it is building.
                        Expr::Call { callee, .. } => match callee.as_ref() {
                            Expr::Member { name, .. }
                                if COMBINATORS
                                    .iter()
                                    .any(|(c, arity)| c == name && *arity > 0) =>
                            {
                                5
                            }
                            _ => 0,
                        },
                        _ => 0,
                    };
                })
            };
            match x {
                Stmt::Let { value, .. }
                | Stmt::Assign { value, .. }
                | Stmt::Destructure { value, .. }
                | Stmt::Return { value, .. }
                | Stmt::Expr { value, .. }
                | Stmt::Patch { value, .. } => count(value),
                Stmt::If { cond, .. } => count(cond),
                _ => {}
            }
        });
    }
    n
}

fn emit_body(ctx: &mut Ctx, body: &[Stmt], f: &mut Function, next_local: &mut u32) -> bool {
    for s in body {
        match s {
            Stmt::Let { name, value, .. } => {
                // `const checked = receipt with Policy` — what this name means
                // for the rest of the action, recorded as it is lowered so the
                // claims read through it can be named after the credential
                // rather than after the binding.
                if let Expr::With { policy, .. } = value {
                    if let Some(t) = ctx.program.trusts.iter().find(|t| t.name == *policy) {
                        ctx.verified.insert(name.clone(), t.subject_type.clone());
                    }
                }
                emit(ctx, value, f);
                let slot = *next_local;
                *next_local += 1;
                ctx.locals.insert(name.clone(), slot);
                f.instruction(&Instruction::LocalSet(slot));
            }

            // `x = …` writes the slot the name already has. A name that has
            // none is a name the checks refused, so there is nothing to do
            // about it here.
            Stmt::Assign { name, value, .. } => {
                emit(ctx, value, f);
                match ctx.locals.get(name).copied() {
                    Some(slot) => {
                        f.instruction(&Instruction::LocalSet(slot));
                    }
                    None => {
                        f.instruction(&Instruction::Drop);
                    }
                }
            }

            // The record is read once into a slot, then each name is a field
            // read out of it — which is what the statement means.
            Stmt::Destructure { names, value, .. } => {
                let holder = *next_local;
                *next_local += 1;
                emit(ctx, value, f);
                f.instruction(&Instruction::LocalSet(holder));
                for name in names {
                    f.instruction(&Instruction::LocalGet(holder));
                    ctx.push_konst(Konst::Str(name.clone()), f);
                    ctx.call_op("field", f);
                    let slot = *next_local;
                    *next_local += 1;
                    ctx.locals.insert(name.clone(), slot);
                    f.instruction(&Instruction::LocalSet(slot));
                }
            }
            Stmt::Return { value, .. } => {
                emit(ctx, value, f);
                // `return` from inside a block leaves the function, not the
                // block — the tree-walker learned this the same day.
                f.instruction(&Instruction::Return);
                return true;
            }
            Stmt::If { cond, then, other, .. } => {
                emit(ctx, cond, f);
                ctx.call_op("truthy", f);
                f.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
                emit_body(ctx, then, f, next_local);
                if !other.is_empty() {
                    f.instruction(&Instruction::Else);
                    emit_body(ctx, other, f, next_local);
                }
                f.instruction(&Instruction::End);
            }
            // `receipt: Credential<PurchaseReceipt>` — what the host collected
            // before any of this ran. It is not state and not a capability:
            // somebody handed it over, and the sheet they agreed to said so.
            Stmt::Binding { name, .. } => {
                let slot = *next_local;
                *next_local += 1;
                ctx.call_val(&crate::abi::Op::Input(name.clone()), 0, f);
                f.instruction(&Instruction::LocalSet(slot));
                ctx.locals.insert(name.clone(), slot);
            }

            // `const rows = credentials of Receipt verified with Policy`
            Stmt::Data { name, source, .. } => {
                match source {
                    DataSource::Credentials { ty, policy, .. } => {
                        ctx.call_cap(&crate::abi::Cap::Read(read_line(ty, policy.as_deref())), f);
                    }
                    DataSource::Query { audience } => {
                        let who = ctx.program.audience_for(audience);
                        ctx.call_cap(&crate::abi::Cap::Query(who), f);
                    }
                    DataSource::Unknown => {
                        ctx.unsupported.push("a data source the front end could not read".into());
                        ctx.push_konst(Konst::Bool(false), f);
                    }
                }
                let slot = *next_local;
                *next_local += 1;
                f.instruction(&Instruction::LocalSet(slot));
                ctx.locals.insert(name.clone(), slot);
            }

            // `member.points: total` — a patch, and the only way state moves.
            // The host writes it, which is why it is an import and why the line
            // it becomes is in the report.
            Stmt::Patch { path, value, .. } => {
                emit(ctx, value, f);
                ctx.call_cap(&crate::abi::Cap::Write(path.join(".")), f);
                f.instruction(&Instruction::Drop);
            }

            // `refuse "tooSmallToEarn"` — an outcome, and the action stops.
            Stmt::Refuse { key, .. } => {
                ctx.call_val(&crate::abi::Op::Refuse(key.clone()), 0, f);
                f.instruction(&Instruction::Return);
                return true;
            }

            // A bare predicate: `state.member exists` in `require`, a trust
            // check in `verify`. Holding is the ordinary case and emits nothing.
            Stmt::Expr { value, .. } => {
                emit(ctx, value, f);
                ctx.call_op("truthy", f);
                f.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
                f.instruction(&Instruction::Else);
                let out = match ctx.phase {
                    Some(Phase::Verify) => crate::abi::Op::Unverified,
                    _ => crate::abi::Op::Defect,
                };
                ctx.call_val(&out, 0, f);
                f.instruction(&Instruction::Return);
                f.instruction(&Instruction::End);
            }

            Stmt::Effect { name, args, body, .. } => {
                // `present { disclose …; prove … }` is one effect and not
                // several: what the person is shown is one request, and the
                // host takes all of it or none. Its lines answer with the part
                // they contribute, and the list is what goes out.
                if name == "present" && args.is_empty() {
                    ctx.push_konst(Konst::EmptyList, f);
                    for line in body {
                        if let Stmt::Effect { name, args, .. } = line {
                            emit_effect(ctx, name, args, f);
                            ctx.call_op("push", f);
                        }
                    }
                    ctx.call_cap(&crate::abi::Cap::Present, f);
                    f.instruction(&Instruction::Drop);
                    continue;
                }
                emit_effect(ctx, name, args, f);
                f.instruction(&Instruction::Drop);
                emit_body(ctx, body, f, next_local);
            }
        }
    }
    false
}

/// One function body, set up and emitted. Both passes call this — a first pass
/// that decided for itself how a body was laid out would be a second compiler.
fn body_of(ctx: &mut Ctx, params: &[Field], body: &[Stmt]) -> Function {
    ctx.locals.clear();
    for (j, p) in params.iter().enumerate() {
        ctx.locals.insert(p.name.clone(), j as u32);
    }
    let extra = count_lets(body);
    let scratch = count_scratch(body);
    let total = extra + scratch;
    let mut out = Function::new(if total > 0 { vec![(total, ValType::I32)] } else { vec![] });
    let mut next_local = params.len() as u32;
    ctx.next_scratch = next_local + extra;
    ctx.scratch_used = ctx.next_scratch;
    emit_body(ctx, body, &mut out, &mut next_local);
    // A body whose branches all return still needs a value on the stack for the
    // paths Wasm can see and the language cannot reach.
    ctx.push_konst(Konst::Bool(false), &mut out);
    out.instruction(&Instruction::End);
    out
}

/// An action, as one function: the phases in the order the language runs them.
///
/// Nothing here decides an outcome. `require` that does not hold calls out and
/// returns, `refuse` calls out and returns, and everything else is the host's
/// to judge from the effects it was handed — which is the same division the
/// tree-walking evaluator makes.
fn action_body(ctx: &mut Ctx, a: &ActionDecl) -> Function {
    ctx.verified.clear();
    let stmts: Vec<Stmt> = a.phases.iter().flat_map(|b| b.stmts.iter().cloned()).collect();
    ctx.locals.clear();
    let extra = count_lets(&stmts);
    let scratch = count_scratch(&stmts);
    let total = extra + scratch;
    let mut out = Function::new(if total > 0 { vec![(total, ValType::I32)] } else { vec![] });
    let mut next_local = 0;
    ctx.next_scratch = extra;
    ctx.scratch_used = ctx.next_scratch;
    for block in &a.phases {
        ctx.phase = Some(block.phase);
        if emit_body(ctx, &block.stmts, &mut out, &mut next_local) {
            break;
        }
    }
    ctx.phase = None;
    ctx.push_konst(Konst::Bool(false), &mut out);
    out.instruction(&Instruction::End);
    out
}

/// What a screen reads, as a function that reads it.
///
/// **Not a screen resolver.** Drawing is not emitted yet; this exists because a
/// screen's `data` lines are capabilities — `credentials of Receipt verified
/// with Policy` is a read, and a query is an audience — and a report that
/// missed them would understate what the person is agreeing to. It is exported
/// under `data:` rather than `screen:` so that nothing mistakes it for the
/// other thing later.
///
/// The tree adds nothing of its own: a screen has no `verify`, so every value
/// in it came from a `data` line or from state, and both are already named.
fn screen_data(ctx: &mut Ctx, s: &ScreenDecl) -> Function {
    ctx.locals.clear();
    ctx.verified.clear();
    let mut out = Function::new(vec![]);
    for d in &s.data {
        match &d.source {
            DataSource::Credentials { ty, policy, .. } => {
                ctx.call_cap(&crate::abi::Cap::Read(read_line(ty, policy.as_deref())), &mut out);
            }
            DataSource::Query { audience } => {
                let who = ctx.program.audience_for(audience);
                ctx.call_cap(&crate::abi::Cap::Query(who), &mut out);
            }
            DataSource::Unknown => {
                ctx.unsupported.push("a data source the front end could not read".into());
                ctx.push_konst(Konst::Bool(false), &mut out);
            }
        }
        out.instruction(&Instruction::Drop);
    }
    ctx.push_konst(Konst::Bool(false), &mut out);
    out.instruction(&Instruction::End);
    out
}

/// How a credential read is written where a person reads it. One string, and
/// the same one whether it came from a screen, a declared input or a `data`
/// line — a report with two spellings of one thing reads as two things.
fn read_line(ty: &str, policy: Option<&str>) -> String {
    match policy {
        Some(p) => format!("{ty} under {p}"),
        None => format!("{ty} — unverified"),
    }
}

/// One effect, as the part it contributes.
///
/// **Every arm leaves exactly one value on the stack.** A `present` block
/// gathers those parts into a list and hands it over as one request; anything
/// else is its own request and the caller drops what comes back. An arm that
/// left nothing would be a module that does not validate.
///
/// **`prove` takes nothing.** The host evaluates the statement and builds the
/// proof, because the host is the only one that can; handing the claim to the
/// module would be the same answer with the privacy removed, which is the thing
/// `prove` exists instead of. Everything else is handed the value it acts on.
fn emit_effect(ctx: &mut Ctx, name: &str, args: &[Arg], f: &mut Function) {
    let first = args.first().map(|a| &a.value);
    match name {
        "disclose" => {
            let path = first.and_then(|e| e.path()).unwrap_or_else(|| "—".into());
            emit_first(ctx, first, f);
            ctx.call_cap(&crate::abi::Cap::Disclose(ctx_claim(ctx, &path)), f);
        }
        "prove" => {
            let said = valang::report::render(first);
            ctx.call_cap(&crate::abi::Cap::Prove(said), f);
        }
        "credential.issue" => {
            let ty = match first {
                Some(Expr::Call { callee, .. }) => callee.path().unwrap_or_else(|| "?".into()),
                Some(Expr::Record { .. }) => "record".into(),
                other => other.and_then(|e| e.path()).unwrap_or_else(|| "?".into()),
            };
            emit_first(ctx, first, f);
            ctx.call_cap(&crate::abi::Cap::Issue(ty), f);
        }
        other => {
            // Loud rather than wrong. An effect this back end does not emit is
            // an effect that would be missing from the import section, and a
            // report short of a line is worse than no module at all. A value
            // still goes on the stack, because the caller is going to take one
            // off and a module that will not validate says less than a message.
            ctx.unsupported.push(format!("the effect `{other}`"));
            ctx.push_konst(Konst::Bool(false), f);
        }
    }
}

/// `checked.claims.country` is what the author wrote; `NationalId.country` is
/// what the person is asked about.
fn ctx_claim(ctx: &Ctx, path: &str) -> String {
    match path.split_once(".claims.") {
        Some((base, rest)) => match ctx.verified_type(base) {
            Some(ty) => format!("{ty}.{rest}"),
            None => path.to_string(),
        },
        None => path.to_string(),
    }
}

fn emit_first(ctx: &mut Ctx, first: Option<&Expr>, f: &mut Function) {
    match first {
        Some(e) => emit(ctx, e, f),
        None => ctx.push_konst(Konst::Bool(false), f),
    }
}

fn emit(ctx: &mut Ctx, e: &Expr, f: &mut Function) {
    match e {
        Expr::Num { value, .. } => ctx.push_konst(Konst::Int(*value), f),
        Expr::Str { value, .. } => ctx.push_konst(Konst::Str(value.clone()), f),
        Expr::Bool { value, .. } => ctx.push_konst(Konst::Bool(*value), f),

        Expr::Ident { name, .. } => match ctx.locals.get(name) {
            Some(slot) => {
                f.instruction(&Instruction::LocalGet(*slot));
            }
            None => ctx.push_konst(Konst::Str(name.clone()), f),
        },

        // The only way to a credential's words: the host checks it against the
        // policy and hands back what it checked. It is a read, and the line it
        // becomes says which policy — "your receipts, checked against the
        // merchant's key" is a different sentence from "your receipts".
        Expr::With { subject, policy, .. } => {
            let _ = subject;
            let ty = ctx
                .program
                .trusts
                .iter()
                .find(|t| t.name == *policy)
                .map(|t| t.subject_type.clone())
                .unwrap_or_else(|| "?".into());
            ctx.call_cap(&crate::abi::Cap::Read(read_line(&ty, Some(policy))), f);
        }

        // `Tier.gold` is one value, not a lookup: an enum member is a constant.
        Expr::Member { obj, name, .. } => {
            // Everything outside the module is an import, and which import it
            // is depends on what the path is rooted at. Read before the general
            // case, because the general case is a field of a value the module
            // is already holding.
            if let Some(path) = e.path() {
                if let Some(rest) = path.strip_prefix("state.") {
                    ctx.call_val(&crate::abi::Op::State(rest.to_string()), 0, f);
                    return;
                }
                if let Some(rest) = path.strip_prefix("next.") {
                    ctx.call_val(&crate::abi::Op::Next(rest.to_string()), 0, f);
                    return;
                }
                if let Some(rest) = path.strip_prefix("context.") {
                    ctx.call_val(&crate::abi::Op::Context(rest.to_string()), 0, f);
                    return;
                }
                if let Some((base, claim)) = path.split_once(".claims.") {
                    if let Some(ty) = ctx.verified_type(base) {
                        ctx.call_cap(&crate::abi::Cap::Read(format!("{ty}.{claim}")), f);
                        return;
                    }
                }
            }
            if let Expr::Ident { name: ty, .. } = obj.as_ref() {
                if ctx.program.enums.iter().any(|en| en.name == *ty) {
                    ctx.push_konst(Konst::Enum(ty.clone(), name.clone()), f);
                    return;
                }
            }
            emit(ctx, obj, f);
            let n = ctx.konst(Konst::Str(name.clone()));
            f.instruction(&Instruction::I32Const(n as i32));
            ctx.call_op("field", f);
        }

        Expr::Unary { op, rhs, .. } => {
            emit(ctx, rhs, f);
            ctx.call_op(if op == "-" { "neg" } else { "not" }, f);
        }

        Expr::Binary { op, lhs, rhs, .. } => {
            emit(ctx, lhs, f);
            emit(ctx, rhs, f);
            let name = match op.as_str() {
                "+" => "add",
                "-" => "sub",
                "*" => "mul",
                "/" => "div",
                "%" => "rem",
                "<" => "lt",
                "<=" => "le",
                ">" => "gt",
                ">=" => "ge",
                "==" => "eq",
                "!=" => "ne",
                "&&" => "and",
                _ => "or",
            };
            ctx.call_op(name, f);
        }

        Expr::Exists { subject, .. } => {
            emit(ctx, subject, f);
            ctx.call_op("exists", f);
        }

        // `a ?: b` is `a` unless it is nothing, and `a` is read once. Written
        // as a ternary over `exists` it would be read twice, and a path here
        // can reach into a credential — a second read of one is a second thing
        // the host is asked for.
        Expr::Elvis { subject, other, .. } => {
            let slot = ctx.scratch();
            emit(ctx, subject, f);
            f.instruction(&Instruction::LocalTee(slot));
            ctx.call_op("exists", f);
            ctx.call_op("truthy", f);
            f.instruction(&Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
            f.instruction(&Instruction::LocalGet(slot));
            f.instruction(&Instruction::Else);
            emit(ctx, other, f);
            f.instruction(&Instruction::End);
        }

        // Built from an empty one. The host owns values, and a module with an
        // allocator of its own would be a second place a value can be wrong.
        Expr::List { items, .. } => {
            ctx.push_konst(Konst::EmptyList, f);
            for item in items {
                emit(ctx, item, f);
                ctx.call_op("push", f);
            }
        }

        Expr::Record { spread, fields, .. } => {
            match spread {
                Some(base) => emit(ctx, base, f),
                None => ctx.push_konst(Konst::EmptyRecord, f),
            }
            for (name, value) in fields {
                ctx.push_konst(Konst::Str(name.clone()), f);
                emit(ctx, value, f);
                ctx.call_op("set", f);
            }
        }

        Expr::Ternary { cond, then, other, .. } => {
            emit(ctx, cond, f);
            ctx.call_op("truthy", f);
            f.instruction(&Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
            emit(ctx, then, f);
            f.instruction(&Instruction::Else);
            emit(ctx, other, f);
            f.instruction(&Instruction::End);
        }

        // Arms in order, as nested `if`s. The compiler has already refused an
        // arm that cannot be reached, so the nesting cannot hide one.
        Expr::Switch { subject, arms, .. } => {
            emit_switch(ctx, subject, arms, 0, f);
        }

        // `receipts.map { r -> … }` — a loop in the module, with the row in a
        // local and the body emitted where it was written. No closure and no
        // function value: a function here is written at the call site or named,
        // and both are known while this is being compiled.
        Expr::Call { callee, args, span } => {
            if let Expr::Member { obj, name, .. } = callee.as_ref() {
                if let Some((_, arity)) = COMBINATORS.iter().find(|(n, _)| n == name) {
                    emit_combinator(ctx, obj, name, *arity, args, *span, f);
                    return;
                }
            }
            let Some(name) = callee.path() else {
                ctx.push_konst(Konst::Bool(false), f);
                return;
            };
            // `LoyaltyMember { points: … }` is a record being built, not a
            // call: the name is the credential's, and every argument is named.
            // It reached the arm below and pushed its arguments onto a stack
            // nothing took them off — a module that would not even validate,
            // which is what says this was never exercised.
            // `LoyaltyMember { … }` is a credential being constructed, not a
            // call: the name is a type this package declares and what follows
            // is a record. It used to reach the arm below and push its argument
            // onto a stack nothing took it off — a module that would not even
            // validate, which is what says this was never exercised.
            //
            // The name has to be a declared type and not merely a call with
            // named arguments: `duration(days: 30)` is a builtin, and building
            // a record out of that made a date comparison compare a record,
            // which committed on one engine and refused on the other.
            let declares = ctx
                .program
                .credentials
                .iter()
                .chain(&ctx.program.types)
                .any(|c| c.name == name);
            if declares {
                match args.as_slice() {
                    [only] if only.name.is_none() => emit(ctx, &only.value, f),
                    _ => {
                        ctx.push_konst(Konst::EmptyRecord, f);
                        for a in args {
                            let field = a.name.clone().unwrap_or_default();
                            ctx.push_konst(Konst::Str(field), f);
                            emit(ctx, &a.value, f);
                            ctx.call_op("set", f);
                        }
                    }
                }
                return;
            }
            // The closed set of functions the language has and nobody
            // declares. A module has none of its own, so it asks for them —
            // and the unit of a `duration` travels in the name, because it is
            // written as an argument name and a module passes values.
            if valang_runtime::eval::is_builtin(&name) {
                for a in args {
                    emit(ctx, &a.value, f);
                }
                let named = args.first().and_then(|a| a.name.clone());
                let what = match named {
                    Some(unit) if name == "duration" => format!("{name}:{unit}"),
                    _ => name.clone(),
                };
                ctx.call_val(&crate::abi::Op::Builtin(what), args.len(), f);
                return;
            }
            for a in args {
                emit(ctx, &a.value, f);
            }
            match ctx.fn_index.get(&name) {
                Some(i) => {
                    let i = *i;
                    f.instruction(&Instruction::Call(i));
                }
                // A builtin the host owns, or something the checks refused.
                // Every argument still has to come off the stack.
                None => {
                    for _ in args {
                        f.instruction(&Instruction::Drop);
                    }
                    ctx.push_konst(Konst::Bool(false), f);
                }
            }
        }

        // Not emitted. Pushing a value in its place is how a module comes to
        // compute something the evaluator does not.
        other => {
            ctx.unsupported.push(describe(other));
            ctx.push_konst(Konst::Bool(false), f);
        }
    }
}

/// What an expression is, for a message about not emitting it.
fn describe(e: &Expr) -> String {
    match e {
        Expr::Exists { .. } => "`exists`".into(),
        Expr::Elvis { .. } => "`?:`".into(),
        Expr::List { .. } => "a list written out".into(),
        Expr::Lambda { .. } => "a function written in place".into(),
        Expr::With { .. } => "`with`".into(),
        Expr::From { .. } => "`from`".into(),
        Expr::Record { .. } => "a record".into(),
        Expr::Float { .. } => "a float".into(),
        Expr::Error { .. } => "something that did not parse".into(),
        _ => "this expression".into(),
    }
}

/// A list operation, as a loop.
///
/// Fuel is what makes this worth having: the language is total, so the loop
/// ends — but ending *eventually* and ending *in time* are different promises,
/// and a loop the module runs is a loop the fuel meter can see.
fn emit_combinator(
    ctx: &mut Ctx,
    subject: &Expr,
    name: &str,
    arity: usize,
    args: &[Arg],
    span: valang::diag::Span,
    f: &mut Function,
) {
    use wasm_encoder::BlockType;

    // `count` and `first` are the host's, whole.
    if arity == 0 {
        emit(ctx, subject, f);
        ctx.call_op(name, f);
        return;
    }

    // The function it was given: written here, or the name of one this package
    // declares. Neither is a value — both are known now.
    let given = args.iter().find_map(|a| match &a.value {
        Expr::Lambda { params, body, .. } => Some(Given::Written(params.clone(), (**body).clone())),
        Expr::Ident { name, .. } if ctx.program.functions.iter().any(|f| f.name == *name) => {
            Some(Given::Named(name.clone()))
        }
        _ => None,
    });
    let Some(given) = given else {
        ctx.unsupported.push(format!("`{name}` without a function to give it"));
        ctx.push_konst(Konst::Bool(false), f);
        let _ = span;
        return;
    };

    let list = ctx.scratch();
    let index = ctx.scratch();
    let length = ctx.scratch();
    let row = ctx.scratch();
    let acc = ctx.scratch();

    emit(ctx, subject, f);
    f.instruction(&Instruction::LocalTee(list));
    ctx.call_op("len", f);
    f.instruction(&Instruction::LocalSet(length));
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(index));

    // What the loop is building, before it starts.
    match name {
        "fold" => {
            match args.first() {
                Some(seed) if !matches!(seed.value, Expr::Lambda { .. }) => {
                    emit(ctx, &seed.value, f)
                }
                _ => ctx.push_konst(Konst::Bool(false), f),
            }
            f.instruction(&Instruction::LocalSet(acc));
        }
        "map" | "filter" => {
            ctx.push_konst(Konst::EmptyList, f);
            f.instruction(&Instruction::LocalSet(acc));
        }
        // `any` starts false and `all` starts true, which is also what each
        // means over no rows at all.
        "any" => {
            ctx.push_konst(Konst::Bool(false), f);
            f.instruction(&Instruction::LocalSet(acc));
        }
        _ => {
            ctx.push_konst(Konst::Bool(true), f);
            f.instruction(&Instruction::LocalSet(acc));
        }
    }

    f.instruction(&Instruction::Block(BlockType::Empty));
    f.instruction(&Instruction::Loop(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(index));
    f.instruction(&Instruction::LocalGet(length));
    f.instruction(&Instruction::I32GeS);
    f.instruction(&Instruction::BrIf(1));

    f.instruction(&Instruction::LocalGet(list));
    f.instruction(&Instruction::LocalGet(index));
    ctx.call_op("at", f);
    f.instruction(&Instruction::LocalSet(row));

    // The body, with what it was handed in scope. Bindings are restored after,
    // so a name the loop introduced does not outlive it.
    let saved = ctx.locals.clone();
    let value_of = |ctx: &mut Ctx, f: &mut Function| match &given {
        Given::Written(params, body) => {
            if name == "fold" {
                if let Some(p) = params.first() {
                    ctx.locals.insert(p.clone(), acc);
                }
                if let Some(p) = params.get(1) {
                    ctx.locals.insert(p.clone(), row);
                }
            } else if let Some(p) = params.first() {
                ctx.locals.insert(p.clone(), row);
            }
            emit(ctx, body, f);
        }
        Given::Named(fname) => {
            if name == "fold" {
                f.instruction(&Instruction::LocalGet(acc));
            }
            f.instruction(&Instruction::LocalGet(row));
            match ctx.fn_index.get(fname).copied() {
                Some(i) => {
                    f.instruction(&Instruction::Call(i));
                }
                None => ctx.push_konst(Konst::Bool(false), f),
            }
        }
    };

    match name {
        "fold" => {
            value_of(ctx, f);
            f.instruction(&Instruction::LocalSet(acc));
        }
        "map" => {
            f.instruction(&Instruction::LocalGet(acc));
            value_of(ctx, f);
            ctx.call_op("push", f);
            f.instruction(&Instruction::LocalSet(acc));
        }
        "filter" => {
            value_of(ctx, f);
            ctx.call_op("truthy", f);
            f.instruction(&Instruction::If(BlockType::Empty));
            f.instruction(&Instruction::LocalGet(acc));
            f.instruction(&Instruction::LocalGet(row));
            ctx.call_op("push", f);
            f.instruction(&Instruction::LocalSet(acc));
            f.instruction(&Instruction::End);
        }
        // Short-circuit: the row that decides ends the loop, so `any` over a
        // long list is not a long loop.
        "any" => {
            value_of(ctx, f);
            ctx.call_op("truthy", f);
            f.instruction(&Instruction::If(BlockType::Empty));
            ctx.push_konst(Konst::Bool(true), f);
            f.instruction(&Instruction::LocalSet(acc));
            f.instruction(&Instruction::Br(2));
            f.instruction(&Instruction::End);
        }
        _ => {
            value_of(ctx, f);
            ctx.call_op("truthy", f);
            f.instruction(&Instruction::I32Eqz);
            f.instruction(&Instruction::If(BlockType::Empty));
            ctx.push_konst(Konst::Bool(false), f);
            f.instruction(&Instruction::LocalSet(acc));
            f.instruction(&Instruction::Br(2));
            f.instruction(&Instruction::End);
        }
    }
    ctx.locals = saved;

    f.instruction(&Instruction::LocalGet(index));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(index));
    f.instruction(&Instruction::Br(0));
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);

    f.instruction(&Instruction::LocalGet(acc));
}

/// The function a list operation was given.
enum Given {
    Written(Vec<String>, Expr),
    Named(String),
}

fn emit_switch(ctx: &mut Ctx, subject: &Expr, arms: &[SwitchArm], i: usize, f: &mut Function) {
    let Some(arm) = arms.get(i) else {
        ctx.push_konst(Konst::Bool(false), f);
        return;
    };
    match &arm.pattern {
        ArmPattern::Default => emit(ctx, &arm.body, f),
        ArmPattern::Value(v) => {
            emit(ctx, subject, f);
            emit(ctx, v, f);
            ctx.call_op("eq", f);
            ctx.call_op("truthy", f);
            f.instruction(&Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
            emit(ctx, &arm.body, f);
            f.instruction(&Instruction::Else);
            emit_switch(ctx, subject, arms, i + 1, f);
            f.instruction(&Instruction::End);
        }
        ArmPattern::Compare { op, rhs } => {
            emit(ctx, subject, f);
            emit(ctx, rhs, f);
            let name = match op.as_str() {
                "<" => "lt",
                "<=" => "le",
                ">" => "gt",
                ">=" => "ge",
                "==" => "eq",
                _ => "ne",
            };
            ctx.call_op(name, f);
            ctx.call_op("truthy", f);
            f.instruction(&Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
            emit(ctx, &arm.body, f);
            f.instruction(&Instruction::Else);
            emit_switch(ctx, subject, arms, i + 1, f);
            f.instruction(&Instruction::End);
        }
    }
}
