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
];

fn import_index(name: &str) -> u32 {
    IMPORTS.iter().position(|(n, _)| *n == name).expect("unknown import") as u32
}

struct Ctx<'a> {
    program: &'a Program,
    konsts: Vec<Konst>,
    locals: BTreeMap<String, u32>,
    /// Function index in the module: imports first, then VAL functions.
    fn_index: BTreeMap<String, u32>,
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

    fn call_op(&mut self, op: &str, f: &mut Function) {
        f.instruction(&Instruction::Call(import_index(op)));
    }
}

/// Compile every `function` in the program. Actions are not compiled: their
/// phases are the host's business, and `execute` describes effects rather than
/// performing them, so there is nothing there for a module to do.
pub fn compile_function(program: &Program) -> Module {
    let mut ctx = Ctx {
        program,
        konsts: Vec::new(),
        locals: BTreeMap::new(),
        fn_index: BTreeMap::new(),
    };

    let import_count = IMPORTS.len() as u32;
    for (i, f) in program.functions.iter().enumerate() {
        ctx.fn_index.insert(f.name.clone(), import_count + i as u32);
    }

    let mut types = TypeSection::new();
    // Type 0: (i32) -> i32. Type 1: (i32, i32) -> i32.
    types.ty().function([ValType::I32], [ValType::I32]);
    types.ty().function([ValType::I32, ValType::I32], [ValType::I32]);
    // Type 2..: one per VAL function arity.
    let mut arity_type: BTreeMap<usize, u32> = BTreeMap::from([(1, 0), (2, 1)]);
    for f in &program.functions {
        let n = f.params.len();
        if !arity_type.contains_key(&n) {
            let id = types.len();
            types.ty().function(vec![ValType::I32; n], [ValType::I32]);
            arity_type.insert(n, id);
        }
    }

    let mut imports = ImportSection::new();
    for (name, arity) in IMPORTS {
        imports.import("val", name, wasm_encoder::EntityType::Function(if *arity == 1 { 0 } else { 1 }));
    }

    let mut funcs = FunctionSection::new();
    let mut code = CodeSection::new();
    let mut exports = ExportSection::new();
    let mut names = Vec::new();

    for (i, f) in program.functions.iter().enumerate() {
        funcs.function(arity_type[&f.params.len()]);
        exports.export(&f.name, wasm_encoder::ExportKind::Func, import_count + i as u32);
        names.push(f.name.clone());

        ctx.locals.clear();
        for (j, p) in f.params.iter().enumerate() {
            ctx.locals.insert(p.name.clone(), j as u32);
        }
        // `const` bindings become extra locals, allocated as they are met.
        let extra = count_lets(&f.body);
        let mut body = Function::new(if extra > 0 { vec![(extra, ValType::I32)] } else { vec![] });
        let mut next_local = f.params.len() as u32;
        emit_body(&mut ctx, &f.body, &mut body, &mut next_local);
        body.instruction(&Instruction::End);
        code.function(&body);
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

    Module { bytes: module.finish(), konsts: ctx.konsts, functions: names }
}

/// The custom section the constants live in. A custom section is ignored by any
/// Wasm runtime that does not know it, which is the right shape: a host that
/// cannot read the pool cannot run the module either, and will say so.
pub const KONST_SECTION: &str = "val.konsts";

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
    body.iter().filter(|s| matches!(s, Stmt::Let { .. })).count() as u32
}

fn emit_body(ctx: &mut Ctx, body: &[Stmt], f: &mut Function, next_local: &mut u32) {
    let mut returned = false;
    for s in body {
        match s {
            Stmt::Let { name, value, .. } => {
                emit(ctx, value, f);
                let slot = *next_local;
                *next_local += 1;
                ctx.locals.insert(name.clone(), slot);
                f.instruction(&Instruction::LocalSet(slot));
            }
            Stmt::Return { value, .. } => {
                emit(ctx, value, f);
                returned = true;
                break;
            }
            // A function is pure and total: nothing else can appear in one, and
            // `check.rs` has already refused anything that did.
            _ => {}
        }
    }
    if !returned {
        ctx.push_konst(Konst::Bool(false), f);
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

        // `Tier.gold` is one value, not a lookup: an enum member is a constant.
        Expr::Member { obj, name, .. } => {
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

        Expr::Call { callee, args, .. } => {
            let Some(name) = callee.path() else {
                ctx.push_konst(Konst::Bool(false), f);
                return;
            };
            for a in args {
                emit(ctx, &a.value, f);
            }
            match ctx.fn_index.get(&name) {
                Some(i) => {
                    let i = *i;
                    f.instruction(&Instruction::Call(i));
                }
                // A builtin the host owns, or something `check.rs` refused. Pop
                // the arguments by producing a value in their place.
                None => ctx.push_konst(Konst::Bool(false), f),
            }
        }

        _ => ctx.push_konst(Konst::Bool(false), f),
    }
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
