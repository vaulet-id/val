//! Running an emitted module.
//!
//! The host holds the values; the module holds `i32` handles into them. Every
//! operation the language has is an import, so the module cannot reach anything
//! that was not handed to it — which is the same guarantee the tree-walking
//! evaluator gets from having no I/O, arrived at differently.

use std::cell::RefCell;
use std::rc::Rc;

use valang_runtime::value::Value;
use wasmi::{Caller, Config, Engine, Extern, Func, Linker, Module as WasmModule, Store};

use crate::compile::{Konst, Module};

/// Handles are indices into this. It only grows during a call, and the whole of
/// it is dropped when the call returns — there is nothing to collect.
#[derive(Default)]
pub struct Values {
    pub slots: Vec<Value>,
}

impl Values {
    fn put(&mut self, v: Value) -> i32 {
        self.slots.push(v);
        (self.slots.len() - 1) as i32
    }
    fn get(&self, h: i32) -> Value {
        self.slots.get(h as usize).cloned().unwrap_or(Value::Null)
    }
}

pub struct Wasm {
    pub konsts: Vec<Konst>,
}

fn konst_value(k: &Konst) -> Value {
    match k {
        Konst::Int(i) => Value::Int(*i),
        Konst::Str(s) => Value::Str(s.clone()),
        Konst::Bool(b) => Value::Bool(*b),
        Konst::Enum(e, m) => Value::Enum(e.clone(), m.clone()),
        Konst::EmptyList => Value::List(Vec::new()),
        Konst::EmptyRecord => Value::Map(Default::default()),
    }
}

type Shared = Rc<RefCell<Values>>;

/// Compile-and-run one function. The arithmetic here traps the same way the
/// evaluator does — a host function returning an error is a Wasm trap, which is
/// what makes "overflow traps" true on this back end too rather than only on
/// the other one.
pub fn run_function(module: &Module, name: &str, args: &[Value]) -> Result<Value, String> {
    run_with_fuel(module, name, args, None)
}

/// The reason this back end exists. The language is total, so a program halts
/// on its own — but halting *eventually* and halting *in time* are different
/// promises, and only one of them can be made to somebody standing at a till.
/// Fuel is the second belt; totality is the first.
pub fn run_with_fuel(module: &Module, name: &str, args: &[Value], fuel: Option<u64>) -> Result<Value, String> {
    let mut config = Config::default();
    config.consume_fuel(fuel.is_some());
    let engine = Engine::new(&config);
    let wasm = WasmModule::new(&engine, &module.bytes[..]).map_err(|e| e.to_string())?;
    let values: Shared = Rc::new(RefCell::new(Values::default()));

    // Constants are built before anything runs, so no literal crosses the
    // boundary during a call.
    let konsts: Vec<Value> = module.konsts.iter().map(konst_value).collect();

    let mut store = Store::new(&engine, values.clone());
    if let Some(f) = fuel {
        store.set_fuel(f).map_err(|e| e.to_string())?;
    }
    let mut linker = <Linker<Shared>>::new(&engine);

    macro_rules! binop {
        ($name:literal, $f:expr) => {{
            let ks = ();
            let _ = ks;
            let func = Func::wrap(&mut store, move |caller: Caller<'_, Shared>, a: i32, b: i32| -> Result<i32, wasmi::Error> {
                let vals = caller.data().clone();
                let (x, y) = {
                    let v = vals.borrow();
                    (v.get(a), v.get(b))
                };
                let out: Result<Value, String> = $f(x, y);
                match out {
                    Ok(v) => Ok(vals.borrow_mut().put(v)),
                    Err(e) => Err(wasmi::Error::new(e)),
                }
            });
            linker.define("val", $name, func).map_err(|e| e.to_string())?;
        }};
    }

    let ks = konsts.clone();
    let konst_fn = Func::wrap(&mut store, move |caller: Caller<'_, Shared>, i: i32| -> i32 {
        let v = ks.get(i as usize).cloned().unwrap_or(Value::Null);
        caller.data().borrow_mut().put(v)
    });
    linker.define("val", "konst", konst_fn).map_err(|e| e.to_string())?;

    // Every one of these is `valang_runtime::eval::binary`, which is what the
    // other back end runs. Written out here once, they were a second answer to
    // what `+` does when it overflows — and the parity test compares results,
    // so two answers that agree on the values in a test agree on nothing else.
    macro_rules! shared {
        ($name:literal, $op:literal) => {
            binop!($name, |a: Value, b: Value| valang_runtime::eval::binary($op, a, b)
                .map_err(|t| t.to_string()));
        };
    }
    shared!("add", "+");
    shared!("sub", "-");
    shared!("mul", "*");
    shared!("div", "/");
    shared!("rem", "%");
    shared!("lt", "<");
    shared!("le", "<=");
    shared!("gt", ">");
    shared!("ge", ">=");
    shared!("eq", "==");
    shared!("ne", "!=");
    shared!("and", "&&");
    shared!("or", "||");
    binop!("field", |a: Value, b: Value| {
        let name = match b {
            Value::Str(s) => s,
            _ => String::new(),
        };
        Ok(a.field(&name).cloned().unwrap_or(Value::Null))
    });

    binop!("push", |a: Value, b: Value| match a {
        Value::List(mut items) => {
            items.push(b);
            Ok(Value::List(items))
        }
        other => Ok(other),
    });

    let exists = Func::wrap(&mut store, move |caller: Caller<'_, Shared>, a: i32| -> i32 {
        let vals = caller.data().clone();
        let there = !matches!(vals.borrow().get(a), Value::Null);
        let h = vals.borrow_mut().put(Value::Bool(there));
        h
    });
    linker.define("val", "exists", exists).map_err(|e| e.to_string())?;

    // Three arguments, which nothing else here takes: a record, a name and a
    // value. Written out rather than through `binop!`, which is for two.
    let set = Func::wrap(
        &mut store,
        move |caller: Caller<'_, Shared>, a: i32, b: i32, c: i32| -> i32 {
            let vals = caller.data().clone();
            let (record, name, value) = {
                let v = vals.borrow();
                (v.get(a), v.get(b), v.get(c))
            };
            let name = match name {
                Value::Str(s) => s,
                _ => String::new(),
            };
            let out = match record {
                Value::Map(mut m) => {
                    m.insert(name, value);
                    Value::Map(m)
                }
                other => other,
            };
            let h = vals.borrow_mut().put(out);
            h
        },
    );
    linker.define("val", "set", set).map_err(|e| e.to_string())?;

    let not = Func::wrap(&mut store, move |caller: Caller<'_, Shared>, a: i32| -> i32 {
        let vals = caller.data().clone();
        let v = vals.borrow().get(a);
        let h = vals.borrow_mut().put(Value::Bool(!v.truthy()));
        h
    });
    linker.define("val", "not", not).map_err(|e| e.to_string())?;

    let neg = Func::wrap(&mut store, move |caller: Caller<'_, Shared>, a: i32| -> Result<i32, wasmi::Error> {
        let vals = caller.data().clone();
        let v = vals.borrow().get(a);
        match v {
            Value::Int(i) => match i.checked_neg() {
                Some(n) => Ok(vals.borrow_mut().put(Value::Int(n))),
                None => Err(wasmi::Error::new("integer overflow in negation traps")),
            },
            other => Ok(vals.borrow_mut().put(other)),
        }
    });
    linker.define("val", "neg", neg).map_err(|e| e.to_string())?;

    // The only place a value becomes control flow. Keeping it in one import
    // means "what counts as true" is answered once.
    let truthy = Func::wrap(&mut store, move |caller: Caller<'_, Shared>, a: i32| -> i32 {
        i32::from(caller.data().borrow().get(a).truthy())
    });
    linker.define("val", "truthy", truthy).map_err(|e| e.to_string())?;

    let instance = linker.instantiate(&mut store, &wasm).map_err(|e| e.to_string())?.start(&mut store).map_err(|e| e.to_string())?;

    let Some(Extern::Func(f)) = instance.get_export(&store, name) else {
        return Err(format!("`{name}` is not exported by this module"));
    };

    let handles: Vec<wasmi::Val> = args
        .iter()
        .map(|v| wasmi::Val::I32(values.borrow_mut().put(v.clone())))
        .collect();
    let mut out = [wasmi::Val::I32(0)];
    f.call(&mut store, &handles, &mut out).map_err(|e| e.to_string())?;

    let wasmi::Val::I32(h) = out[0] else { return Err("a function returned something that is not a handle".into()) };
    let result = values.borrow().get(h);
    Ok(result)
}


