//! Walking an action with the module instead of the evaluator.
//!
//! **This is what a phone does.** It has no compiler, so it cannot walk a typed
//! AST; it has a module and the host behind it. Everything a record rests on —
//! the roots, the hashes, the batch offered to the host, the moment state
//! commits — belongs to `valang_runtime` and is the same either way. What is
//! here is evaluation, and the plumbing that answers a module's imports.
//!
//! **What the host is asked for is settled before anything runs.** A module's
//! imports are the whole of what it can reach, so they can be resolved up
//! front: the credentials, the claims, the query answers, the input. Nothing is
//! fetched mid-run, which means the host is asked once and a person is
//! interrupted once.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use valang_runtime::eval::Trap;
use valang_runtime::host::{Context, EffectRequest, Host};
use valang_runtime::value::Value;
use valang_runtime::{ActionAbout, Engine, State, Stopped};
use wasmi::{Caller, Config, Extern, Func, Linker, Store};

use crate::abi::{Cap, Op, CAPS, OPS};
use crate::compile::Module;
use crate::run::{konst_value, HasValues, Shared, Values};

/// The module, and what running it is allowed to cost.
pub struct Wasm<'a> {
    pub module: &'a Module,
    /// Totality says an action ends. Fuel says it ends in time, which is a
    /// different promise and the only one that can be made to somebody standing
    /// at a till.
    ///
    /// **Never `None` by default.** Totality is a property of programs this
    /// compiler emitted, and a wallet runs modules nobody here compiled — a
    /// loop in one of those would hang the phone, and the person would be
    /// looking at a wallet that had stopped rather than at an application that
    /// misbehaved.
    pub fuel: Option<u64>,
}

/// What one action may cost. Generous — the examples finish inside a few
/// thousand — and finite, which is the only thing about it that matters.
pub const FUEL: u64 = 50_000_000;

impl<'a> Wasm<'a> {
    pub fn new(module: &'a Module) -> Wasm<'a> {
        Wasm { module, fuel: Some(FUEL) }
    }
}

/// Everything the imports read and write while the module runs.
struct Run {
    values: Shared,
    /// What the host answered before anything ran, by import name — or why it
    /// could not. A credential the host does not hold is a failure at the line
    /// that reads it and not before: a module may import a capability it never
    /// exercises, and refusing up front would stop an action that never asked.
    answers: BTreeMap<String, Result<Value, Trap>>,
    /// The state the action started from, and the one it is building.
    state: State,
    next: State,
    effects: Vec<EffectRequest>,
    /// The lines of the `present` block being built. The host accumulates them
    /// as they are called; a module that could hand over the list could hand
    /// over one that says anything.
    parts: Vec<Value>,
    /// Why it stopped, when it did. A Wasm trap carries a string; this carries
    /// the outcome the language means by it.
    stopped: Option<Trap>,
}

type Shell = Rc<RefCell<Run>>;

impl HasValues for Shell {
    fn values(&self) -> Shared {
        self.borrow().values.clone()
    }
}

/// Read a dotted path out of a map, which is what `state.member.points` is.
fn at(map: &State, path: &str) -> Value {
    let mut here = Value::Map(map.clone());
    for part in path.split('.') {
        here = here.field(part).cloned().unwrap_or(Value::Null);
    }
    here
}

/// Write one, growing the maps it passes through. A patch names a path, and a
/// path that does not exist yet is a field being written for the first time.
fn put(map: &mut State, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    let Some((last, rest)) = parts.split_last() else { return };
    let mut here = map;
    for part in rest {
        let entry = here.entry((*part).to_string()).or_insert_with(|| Value::Map(Default::default()));
        if !matches!(entry, Value::Map(_)) {
            *entry = Value::Map(Default::default());
        }
        let Value::Map(inner) = entry else { return };
        here = inner;
    }
    here.insert((*last).to_string(), value);
}

/// The credential a claim is read through, and the claims themselves.
///
/// `read:PurchaseReceipt under ReceiptFromMerchant` is the credential;
/// `read:PurchaseReceipt.amount` is one of its claims. Both come from the same
/// answer, so the host is asked once per credential however many claims are
/// read off it.
fn resolve(action: &ActionAbout, input: &State, context: &Context, host: &dyn Host, module: &[u8]) -> Result<BTreeMap<String, Result<Value, Trap>>, String> {
    let engine = wasmi::Engine::default();
    let parsed = wasmi::Module::new(&engine, module).map_err(|e| e.to_string())?;

    // Which claims of each credential this module asked for by name. A
    // credential handed over whole would carry every claim on it, and the sheet
    // renders the named claims — so a module asking for `NationalId under
    // GovernmentIssued` *and* `NationalId.country` would be shown as reading
    // the country while holding the birthdate. It is handed what it named.
    let named = claims_named(&parsed);

    let mut answers: BTreeMap<String, Result<Value, Trap>> = BTreeMap::new();
    let mut credentials: BTreeMap<String, Option<BTreeMap<String, Value>>> = BTreeMap::new();

    for import in parsed.imports() {
        let name = import.name().to_string();
        match import.module() {
            CAPS => match Cap::parse(&name) {
                Some(Cap::Read(line)) => {
                    // `Type under Policy`, `Type — unverified`, or `Type.claim`.
                    let (ty, policy) = match line.split_once(" under ") {
                        Some((ty, policy)) => (ty.to_string(), Some(policy.to_string())),
                        None => match line.split_once(' ') {
                            Some((ty, _)) => (ty.to_string(), None),
                            None => (line.split('.').next().unwrap_or(&line).to_string(), None),
                        },
                    };
                    let held = match credentials.get(&ty) {
                        Some(c) => c.clone(),
                        None => {
                            let asked = policy.clone().or_else(|| {
                                action
                                    .inputs
                                    .iter()
                                    .find(|d| d.credential == ty)
                                    .and_then(|d| d.policy.clone())
                            });
                            let c = host.credential(&ty, asked.as_deref());
                            credentials.insert(ty.clone(), c.clone());
                            c
                        }
                    };
                    let answer = match held {
                        // The same sentence the evaluator gives, because it is
                        // the same thing that happened: the host was asked for
                        // a credential under this policy and had none.
                        None => Err(Trap::Failed(format!(
                            "nothing satisfied `{}`",
                            policy.clone().unwrap_or_else(|| ty.clone())
                        ))),
                        Some(claims) => Ok(match line.split_once('.') {
                            // A claim, when the line names one and not a policy.
                            Some((_, claim)) if !line.contains(' ') => {
                                claims.get(claim).cloned().unwrap_or(Value::Null)
                            }
                            // The credential itself, carrying only the claims
                            // this module asked for by name — which are the
                            // claims the person was shown.
                            _ => Value::Credential {
                                ty: ty.clone(),
                                claims: kept(&claims, named.get(&ty)),
                                verified: policy.clone(),
                            },
                        }),
                    };
                    answers.insert(format!("{CAPS}/{name}"), answer);
                }
                // The claim a `disclose` names. The host fetches it and hands
                // it to whoever is being shown it — never to the module, which
                // is why it is not a read.
                Some(Cap::Disclose(claim)) => {
                    // Written out rather than fetched: `disclose "yes"` hands
                    // over the word, and the sheet says the word.
                    if let Some(literal) = written_out(&claim) {
                        answers.insert(format!("{CAPS}/{name}"), Ok(literal));
                        continue;
                    }
                    let Some((ty, which)) = claim.split_once('.') else { continue };
                    let asked = action
                        .inputs
                        .iter()
                        .find(|d| d.credential == ty)
                        .and_then(|d| d.policy.clone());
                    let answer = match host.credential(ty, asked.as_deref()) {
                        Some(claims) => Ok(claims.get(which).cloned().unwrap_or(Value::Null)),
                        None => Err(Trap::Failed(format!(
                            "nothing satisfied `{}`",
                            asked.unwrap_or_else(|| ty.to_string())
                        ))),
                    };
                    answers.insert(format!("{CAPS}/{name}"), answer);
                }
                Some(Cap::Query(audience)) => {
                    answers.insert(
                        format!("{CAPS}/{name}"),
                        Ok(Value::List(host.query(&audience, ""))),
                    );
                }
                _ => {}
            },
            OPS => match Op::parse(&name, |n| crate::compile::IMPORTS.iter().any(|(x, _)| *x == n)) {
                Some(Op::Input(binding)) => {
                    // Either something the host collected into `input`, or a
                    // credential the action declared and the host chose.
                    let value = match input.get(&binding) {
                        Some(v) => v.clone(),
                        None => credential_for(action, &binding, host, &named)
                            .unwrap_or(Value::Null),
                    };
                    answers.insert(format!("{OPS}/{name}"), Ok(value));
                }
                Some(Op::Context(what)) => {
                    let value = match what.as_str() {
                        "time.now" => Value::Int(context.time_now),
                        "uuid" | "random.uuid" => Value::Str(context.random_uuid.clone()),
                        _ => Value::Null,
                    };
                    answers.insert(format!("{OPS}/{name}"), Ok(value));
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(answers)
}

/// The claims each credential was asked for by name, across the whole import
/// section.
fn claims_named(module: &wasmi::Module) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for import in module.imports() {
        if import.module() != CAPS {
            continue;
        }
        let Some(Cap::Read(line)) = Cap::parse(import.name()) else { continue };
        if line.contains(' ') {
            continue;
        }
        let Some((ty, claim)) = line.split_once('.') else { continue };
        out.entry(ty.to_string()).or_default().push(claim.to_string());
    }
    out
}

/// A credential's claims, narrowed to the ones that were named. Nothing named
/// means nothing carried: a module that asked for a credential and no claim of
/// it can check the credential and read none of it, which is what `verify`
/// does and the whole of what it needs.
fn kept(claims: &BTreeMap<String, Value>, named: Option<&Vec<String>>) -> BTreeMap<String, Value> {
    let Some(named) = named else { return BTreeMap::new() };
    claims.iter().filter(|(k, _)| named.contains(k)).map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// What a module would actually be handed, of a credential it asked for.
///
/// Here so that a test can attack it: the check that matters is not what the
/// sheet renders but what the module ends up holding, and the two used to
/// differ.
pub fn claims_handed_over(module: &[u8], ty: &str, held: &[String]) -> Vec<String> {
    let engine = wasmi::Engine::default();
    let Ok(parsed) = wasmi::Module::new(&engine, module) else { return Vec::new() };
    let named = claims_named(&parsed);
    let all: BTreeMap<String, Value> =
        held.iter().map(|k| (k.clone(), Value::Str(String::new()))).collect();
    kept(&all, named.get(ty)).into_keys().collect()
}

/// The credential the action's `input` declared under this name, as the host
/// chose it — already checked against the policy `verify` will name.
fn credential_for(
    action: &ActionAbout,
    binding: &str,
    host: &dyn Host,
    named: &BTreeMap<String, Vec<String>>,
) -> Option<Value> {
    let declared = action.inputs.iter().find(|d| d.binding == binding)?;
    let claims = host.credential(&declared.credential, declared.policy.as_deref())?;
    // Narrowed like every other credential. `input:` is not a line in the
    // report — it is the host handing over what somebody chose — so a whole
    // credential arriving here would be every claim on it read with nothing
    // said about it anywhere.
    Some(Value::Credential {
        ty: declared.credential.clone(),
        claims: kept(&claims, named.get(&declared.credential)),
        verified: None,
    })
}

impl Engine for Wasm<'_> {
    fn walk(
        &mut self,
        action: &ActionAbout,
        state: &State,
        input: &State,
        host: &dyn Host,
        context: &Context,
    ) -> Result<(State, Vec<EffectRequest>), Stopped> {
        let stop = |why: String| Stopped { trap: Trap::Defect(why), effects: Vec::new() };

        let answers = resolve(action, input, context, host, &self.module.bytes)
            .map_err(|e| stop(format!("this module cannot be read: {e}")))?;

        let shell: Shell = Rc::new(RefCell::new(Run {
            values: Rc::new(RefCell::new(Values::default())),
            answers,
            state: state.clone(),
            next: state.clone(),
            effects: Vec::new(),
            parts: Vec::new(),
            stopped: None,
        }));

        let mut config = Config::default();
        config.consume_fuel(self.fuel.is_some());
        let engine = wasmi::Engine::new(&config);
        let parsed = wasmi::Module::new(&engine, &self.module.bytes[..])
            .map_err(|e| stop(format!("this module cannot be read: {e}")))?;

        let mut store = Store::new(&engine, shell.clone());
        if let Some(f) = self.fuel {
            store.set_fuel(f).map_err(|e| stop(e.to_string()))?;
        }
        let mut linker = <Linker<Shell>>::new(&engine);

        let konsts: Vec<Value> = self.module.konsts.iter().map(konst_value).collect();
        crate::run::define_ops(&mut store, &mut linker, konsts).map_err(&stop)?;
        define_host(&mut store, &mut linker, &parsed).map_err(&stop)?;

        let instance = linker
            .instantiate(&mut store, &parsed)
            .and_then(|i| i.start(&mut store))
            .map_err(|e| stop(e.to_string()))?;

        let export = format!("action:{}", action.name);
        let Some(Extern::Func(f)) = instance.get_export(&store, &export) else {
            return Err(stop(format!("this module does not carry `{}`", action.name)));
        };

        let mut out = [wasmi::Val::I32(0)];
        let called = f.call(&mut store, &[], &mut out);

        let run = shell.borrow();
        if let Some(trap) = run.stopped.clone() {
            // The action said why it stopped. The Wasm error underneath it is
            // how it stopped, which is nobody's business but this file's.
            return Err(Stopped { trap, effects: run.effects.clone() });
        }
        if let Err(e) = called {
            let why = e.to_string();
            return Err(Stopped {
                trap: if why.contains("fuel") {
                    Trap::Failed("this action ran longer than it was given".into())
                } else {
                    Trap::Defect(why)
                },
                effects: run.effects.clone(),
            });
        }
        Ok((run.next.clone(), run.effects.clone()))
    }
}

/// Everything outside the fixed operations: what the host answers, what the
/// action writes, and what it asks to have done.
fn define_host(
    store: &mut Store<Shell>,
    linker: &mut Linker<Shell>,
    module: &wasmi::Module,
) -> Result<(), String> {
    for import in module.imports() {
        let ns = import.module().to_string();
        let name = import.name().to_string();
        let key = format!("{ns}/{name}");

        if ns == OPS {
            let Some(op) = Op::parse(&name, |n| crate::compile::IMPORTS.iter().any(|(x, _)| *x == n))
            else {
                return Err(format!("`{name}` is not an operation this host provides"));
            };
            match op {
                // Already defined, and the same in every module.
                Op::Fixed(_) => continue,
                Op::State(path) => answer(store, linker, &ns, &name, move |run| at(&run.state, &path))?,
                // The state `update` produced, read live: `execute` reads what
                // was just written, not what the action started with.
                Op::Next(path) => answer(store, linker, &ns, &name, move |run| at(&run.next, &path))?,
                Op::Input(_) | Op::Context(_) => answers_with(store, linker, &ns, &name, key)?,
                // `builtin:duration:days` — the name, and the unit when the
                // language writes one as an argument name.
                Op::Builtin(what) => {
                    let (fname, unit) = match what.split_once(':') {
                        Some((f, u)) => (f.to_string(), Some(u.to_string())),
                        None => (what.clone(), None),
                    };
                    let arity = match import.ty().func() {
                        Some(ty) => ty.params().len(),
                        None => 0,
                    };
                    builtin_import(store, linker, &ns, &name, fname, unit, arity)?
                }
                Op::Refuse(what) => stops(store, linker, &ns, &name, move || Trap::Refused(what.clone()))?,
                Op::Defect => stops(store, linker, &ns, &name, || {
                    Trap::Defect("a `require` did not hold".into())
                })?,
                Op::Unverified => stops(store, linker, &ns, &name, || {
                    Trap::Failed("a credential did not satisfy the policy it was checked against".into())
                })?,
            }
            continue;
        }

        let Some(cap) = Cap::parse(&name) else {
            return Err(format!("`{name}` is not a capability this host knows"));
        };
        match cap {
            Cap::Read(_) | Cap::Query(_) => answers_with(store, linker, &ns, &name, key)?,
            Cap::Write(path) => takes(store, linker, &ns, &name, move |run, v| {
                put(&mut run.next, &path, v.clone());
                v
            })?,
            // A line of a `present`, which answers with the part it contributes
            // rather than sending anything: one block is one request.
            // The claim the import names, fetched by the host. The module never
            // sees it and never chose it.
            Cap::Disclose(claim) => {
                let key = format!("{CAPS}/{name}");
                discloses(store, linker, &ns, &name, key, claim)?
            }
            Cap::Prove(said) => gathers(store, linker, &ns, &name, move || {
                let mut m = BTreeMap::new();
                m.insert("kind".into(), Value::Str("prove".into()));
                m.insert("statement".into(), Value::Str(said.clone()));
                m
            })?,
            // What its lines produced, in the order they were called. Nothing
            // the module composed.
            Cap::Present => present(store, linker, &ns, &name)?,
            // The record the module built becomes the credential it issues:
            // the type is in the import's own name, which is what a person is
            // being told is about to be signed for them.
            // No design yet for what the report should say about an amount that
            // is computed, so a module may not ask. Refusing beats a sheet that
            // renders one number while the host is handed another.
            Cap::Pay(_) => {
                return Err(format!("`{name}`: this host does not carry payments yet"))
            }
            Cap::Issue(ty) => takes(store, linker, &ns, &name, move |run, v| {
                let claims = match &v {
                    Value::Map(m) => m.clone(),
                    _ => Default::default(),
                };
                let credential = Value::Credential { ty: ty.clone(), claims, verified: None };
                run.effects.push(EffectRequest {
                    capability: "credential.issue".into(),
                    operation: "issue".into(),
                    payload: credential.clone(),
                    reversible: true,
                });
                credential
            })?,
            Cap::Pay(_) => effect(store, linker, &ns, &name, "payment.request")?,
        }
    }
    Ok(())
}

/// An import that takes nothing and answers with a value.
fn answer(
    store: &mut Store<Shell>,
    linker: &mut Linker<Shell>,
    ns: &str,
    name: &str,
    f: impl Fn(&Run) -> Value + Send + Sync + 'static,
) -> Result<(), String> {
    let func = Func::wrap(&mut *store, move |caller: Caller<'_, Shell>| -> i32 {
        let shell = caller.data().clone();
        let value = f(&shell.borrow());
        let values = shell.borrow().values.clone();
        let h = values.borrow_mut().put(value);
        h
    });
    linker.define(ns, name, func).map_err(|e| e.to_string())?;
    Ok(())
}

/// An import whose value the host settled before anything ran — and which stops
/// the action when the host had nothing to settle it with.
fn answers_with(
    store: &mut Store<Shell>,
    linker: &mut Linker<Shell>,
    ns: &str,
    name: &str,
    key: String,
) -> Result<(), String> {
    let func =
        Func::wrap(&mut *store, move |caller: Caller<'_, Shell>| -> Result<i32, wasmi::Error> {
            let shell = caller.data().clone();
            let answer = shell.borrow().answers.get(&key).cloned();
            match answer {
                Some(Ok(v)) => {
                    let values = shell.borrow().values.clone();
                    let h = values.borrow_mut().put(v);
                    Ok(h)
                }
                Some(Err(trap)) => {
                    shell.borrow_mut().stopped = Some(trap);
                    Err(wasmi::Error::new("the action stopped"))
                }
                None => {
                    let values = shell.borrow().values.clone();
                    let h = values.borrow_mut().put(Value::Null);
                    Ok(h)
                }
            }
        });
    linker.define(ns, name, func).map_err(|e| e.to_string())?;
    Ok(())
}

/// An import that takes one value and answers with one.
fn takes(
    store: &mut Store<Shell>,
    linker: &mut Linker<Shell>,
    ns: &str,
    name: &str,
    f: impl Fn(&mut Run, Value) -> Value + Send + Sync + 'static,
) -> Result<(), String> {
    let func = Func::wrap(&mut *store, move |caller: Caller<'_, Shell>, h: i32| -> i32 {
        let shell = caller.data().clone();
        let values = shell.borrow().values.clone();
        let given = values.borrow().get(h);
        let out = f(&mut shell.borrow_mut(), given);
        let h = values.borrow_mut().put(out);
        h
    });
    linker.define(ns, name, func).map_err(|e| e.to_string())?;
    Ok(())
}

/// An effect that is its own request: one call, one thing asked of the host.
fn effect(
    store: &mut Store<Shell>,
    linker: &mut Linker<Shell>,
    ns: &str,
    name: &str,
    capability: &'static str,
) -> Result<(), String> {
    takes(store, linker, ns, name, move |run, v| {
        run.effects.push(EffectRequest {
            capability: capability.to_string(),
            operation: capability.split('.').nth(1).unwrap_or(capability).to_string(),
            payload: v.clone(),
            reversible: true,
        });
        v
    })
}

/// One of the functions the language has and nobody declares. What it means is
/// `valang_runtime::eval::builtin` and nothing here — a second answer to what
/// `duration(days: 30)` is would be the two engines disagreeing about a date.
fn builtin_import(
    store: &mut Store<Shell>,
    linker: &mut Linker<Shell>,
    ns: &str,
    name: &str,
    fname: String,
    unit: Option<String>,
    arity: usize,
) -> Result<(), String> {
    let call = move |caller: &Caller<'_, Shell>, given: Vec<Value>| -> Result<i32, wasmi::Error> {
        let shell = caller.data().clone();
        match valang_runtime::eval::builtin(&fname, unit.as_deref(), &given) {
            Ok(v) => {
                let values = shell.borrow().values.clone();
                let h = values.borrow_mut().put(v);
                Ok(h)
            }
            Err(trap) => {
                shell.borrow_mut().stopped = Some(trap);
                Err(wasmi::Error::new("the action stopped"))
            }
        }
    };
    let read = |caller: &Caller<'_, Shell>, hs: &[i32]| -> Vec<Value> {
        let values = caller.data().borrow().values.clone();
        let v = values.borrow();
        hs.iter().map(|h| v.get(*h)).collect()
    };
    match arity {
        1 => {
            let f = Func::wrap(&mut *store, move |caller: Caller<'_, Shell>, a: i32| {
                let given = read(&caller, &[a]);
                call(&caller, given)
            });
            linker.define(ns, name, f).map_err(|e| e.to_string())?;
        }
        2 => {
            let f = Func::wrap(&mut *store, move |caller: Caller<'_, Shell>, a: i32, b: i32| {
                let given = read(&caller, &[a, b]);
                call(&caller, given)
            });
            linker.define(ns, name, f).map_err(|e| e.to_string())?;
        }
        n => return Err(format!("`{name}` takes {n} arguments, and this host wires one or two")),
    }
    Ok(())
}

/// A disclosure written out in the import's own name, rather than named as a
/// claim to fetch. The same string a person is shown.
fn written_out(said: &str) -> Option<Value> {
    if let Some(text) = said.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
        return Some(Value::Str(text.to_string()));
    }
    if let Ok(n) = said.parse::<i64>() {
        return Some(Value::Int(n));
    }
    match said {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => None,
    }
}

/// A line of a `present`: the host looks up what the import names, records the
/// part, and hands the module nothing.
fn discloses(
    store: &mut Store<Shell>,
    linker: &mut Linker<Shell>,
    ns: &str,
    name: &str,
    key: String,
    claim: String,
) -> Result<(), String> {
    let func =
        Func::wrap(&mut *store, move |caller: Caller<'_, Shell>| -> Result<i32, wasmi::Error> {
            let shell = caller.data().clone();
            let answer = shell.borrow().answers.get(&key).cloned();
            let of = match answer {
                Some(Ok(v)) => v,
                Some(Err(trap)) => {
                    shell.borrow_mut().stopped = Some(trap);
                    return Err(wasmi::Error::new("the action stopped"));
                }
                // Disclosing something it never asked to read. The host has
                // nothing to hand over, and the sheet said the claim — so this
                // is a module disagreeing with itself, not with the person.
                None => {
                    shell.borrow_mut().stopped =
                        Some(Trap::Defect(format!("`{claim}` is disclosed and never read")));
                    return Err(wasmi::Error::new("the action stopped"));
                }
            };
            let mut m = BTreeMap::new();
            m.insert("kind".to_string(), Value::Str("disclose".into()));
            m.insert("of".to_string(), of);
            shell.borrow_mut().parts.push(Value::Map(m));
            let values = shell.borrow().values.clone();
            let h = values.borrow_mut().put(Value::Null);
            Ok(h)
        });
    linker.define(ns, name, func).map_err(|e| e.to_string())?;
    Ok(())
}

/// A line that contributes something the host already knows — the statement a
/// `prove` proves, which is written in the import's own name.
fn gathers(
    store: &mut Store<Shell>,
    linker: &mut Linker<Shell>,
    ns: &str,
    name: &str,
    part: impl Fn() -> BTreeMap<String, Value> + Send + Sync + 'static,
) -> Result<(), String> {
    let func = Func::wrap(&mut *store, move |caller: Caller<'_, Shell>| -> i32 {
        let shell = caller.data().clone();
        shell.borrow_mut().parts.push(Value::Map(part()));
        let values = shell.borrow().values.clone();
        let h = values.borrow_mut().put(Value::Null);
        h
    });
    linker.define(ns, name, func).map_err(|e| e.to_string())?;
    Ok(())
}

/// One `present` is one request, carrying the lines the host gathered.
fn present(
    store: &mut Store<Shell>,
    linker: &mut Linker<Shell>,
    ns: &str,
    name: &str,
) -> Result<(), String> {
    let func = Func::wrap(&mut *store, move |caller: Caller<'_, Shell>| -> i32 {
        let shell = caller.data().clone();
        let parts = std::mem::take(&mut shell.borrow_mut().parts);
        shell.borrow_mut().effects.push(EffectRequest {
            capability: "disclosure.present".into(),
            operation: "present".into(),
            payload: Value::List(parts),
            reversible: false,
        });
        let values = shell.borrow().values.clone();
        let h = values.borrow_mut().put(Value::Null);
        h
    });
    linker.define(ns, name, func).map_err(|e| e.to_string())?;
    Ok(())
}

/// An import that ends the action, and says in the language's own terms why.
fn stops(
    store: &mut Store<Shell>,
    linker: &mut Linker<Shell>,
    ns: &str,
    name: &str,
    why: impl Fn() -> Trap + Send + Sync + 'static,
) -> Result<(), String> {
    let func =
        Func::wrap(&mut *store, move |caller: Caller<'_, Shell>| -> Result<i32, wasmi::Error> {
            caller.data().borrow_mut().stopped = Some(why());
            Err(wasmi::Error::new("the action stopped"))
        });
    linker.define(ns, name, func).map_err(|e| e.to_string())?;
    Ok(())
}
