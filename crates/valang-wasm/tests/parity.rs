//! Two back ends over one front end must agree.
//!
//! This is the only test of a second back end worth having, and it satisfies
//! the rule that a fixture must not be written by the same reasoning as the
//! code it tests: the expected values come from the tree-walking evaluator,
//! which was written first and knows nothing about Wasm.

use std::collections::BTreeMap;

use valang_runtime::eval::Eval;
use valang_runtime::host::Context;
use valang_runtime::value::Value;
use valang_wasm::{compile_function, run_function, run_with_fuel};

const LOYALTY: &str = include_str!("../../../examples/loyalty.val");
const PORTFOLIO: &str = include_str!("../../../examples/portfolio.val");

fn walked(src: &str, name: &str, args: &[Value]) -> Result<Value, String> {
    let (program, _) = valang::analyse(src);
    let f = program.functions.iter().find(|f| f.name == name).expect("declared");
    let mut ev = Eval::new(&program, Context { time_now: 0, random_uuid: String::new() });
    for (p, v) in f.params.iter().zip(args) {
        ev.bind(&p.name, v.clone());
    }
    let state = BTreeMap::new();
    let call = valang::ast::Expr::Call {
        callee: Box::new(valang::ast::Expr::Ident { name: name.into(), span: Default::default() }),
        args: args
            .iter()
            .enumerate()
            .map(|(i, _)| valang::ast::Arg {
                spread: false,
                name: None,
                span: Default::default(),
                value: valang::ast::Expr::Ident { name: f.params[i].name.clone(), span: Default::default() },
            })
            .collect(),
        span: Default::default(),
    };
    ev.expr(&call, &state).map_err(|t| format!("{t:?}"))
}

fn emitted(src: &str, name: &str, args: &[Value]) -> Result<Value, String> {
    let (program, _) = valang::analyse(src);
    let module = compile_function(&program).expect("every function in this example is one both back ends have");
    run_function(&module, name, args)
}

fn both_agree(src: &str, name: &str, args: &[Value]) {
    let a = walked(src, name, args);
    let b = emitted(src, name, args);
    assert_eq!(a, b, "{name}{args:?}: the evaluator said {a:?} and the module said {b:?}");
}

#[test]
fn a_tier_lookup_means_the_same_thing_compiled() {
    for points in [0, 1, 1_999, 2_000, 9_999, 10_000, 250_000] {
        both_agree(LOYALTY, "tierFor", &[Value::Int(points)]);
    }
}

#[test]
fn the_switch_returns_an_enum_member_not_a_string() {
    let v = emitted(LOYALTY, "tierFor", &[Value::Int(10_000)]).unwrap();
    assert_eq!(v, Value::Enum("Tier".into(), "gold".into()));
}

#[test]
fn a_ternary_and_integer_division_agree() {
    for (market, cost) in [(89_900i64, 64_000i64), (100, 100), (0, 1), (64_000, 89_900)] {
        both_agree(PORTFOLIO, "returnBasisPoints", &[Value::Int(market), Value::Int(cost)]);
    }
}

/// The zero denominator the portfolio example writes an answer for, so neither
/// back end should trap here — and both should say the same thing about it.
#[test]
fn a_gift_with_no_cost_basis_is_answered_not_trapped() {
    both_agree(PORTFOLIO, "returnBasisPoints", &[Value::Int(50_000), Value::Int(0)]);
    assert_eq!(emitted(PORTFOLIO, "returnBasisPoints", &[Value::Int(50_000), Value::Int(0)]).unwrap(), Value::Int(0));
}

/// What the back end is actually for. Totality says the program halts; fuel
/// says when. A tier lookup finishes on a small budget, and the same lookup on
/// a budget of one does not — which is the promise you can make to somebody
/// standing at a till and cannot make from totality alone.
#[test]
fn fuel_is_the_second_belt() {
    let (program, _) = valang::analyse(LOYALTY);
    let module = compile_function(&program).expect("every function in this example is one both back ends have");

    let ok = run_with_fuel(&module, "tierFor", &[Value::Int(5_000)], Some(10_000));
    assert_eq!(ok.unwrap(), Value::Enum("Tier".into(), "silver".into()));

    let starved = run_with_fuel(&module, "tierFor", &[Value::Int(5_000)], Some(1));
    assert!(starved.is_err(), "a budget of one instruction should not finish anything");
}

/// Overflow traps on this back end too. A host function returning an error is a
/// Wasm trap, which is what makes the rule true on both tails rather than only
/// on the one that was easy.
#[test]
fn overflow_traps_in_the_module_as_it_does_in_the_evaluator() {
    let src = r#"
app "x"
version 1
function double(n: int): int { return n * n }
"#;
    let err = emitted(src, "double", &[Value::Int(i64::MAX)]).unwrap_err();
    assert!(err.contains("overflow"), "{err}");
    assert!(walked(src, "double", &[Value::Int(i64::MAX)]).is_err());
}

/// A module has to be a thing somebody can ship. The constants used to travel
/// beside it, which made it something that only ran next to the compiler that
/// produced it — not something to sign, hash, or hand to a host.
#[test]
fn the_constants_travel_inside_the_module() {
    use valang_wasm::konsts_of;

    let (program, _) = valang::analyse(LOYALTY);
    let module = compile_function(&program).expect("every function in this example is one both back ends have");
    let recovered = konsts_of(&module.bytes).expect("the pool is in the module");
    assert_eq!(recovered, module.konsts);

    // And running from only the bytes gives the same answer as running from the
    // compiler's own copy.
    let from_bytes = valang_wasm::Module {
        bytes: module.bytes.clone(),
        konsts: recovered,
        functions: module.functions.clone(),
    };
    assert_eq!(
        run_function(&from_bytes, "tierFor", &[Value::Int(5_000)]).unwrap(),
        Value::Enum("Tier".into(), "silver".into())
    );
}

/// `if` is a statement in this language and `return` inside one leaves the
/// function. Both back ends have to agree about that, and the failure it
/// prevents is quiet: the rest of the body running and a later `return`
/// winning.
#[test]
fn an_early_return_from_a_branch_leaves_the_function() {
    let src = r#"
app "x"
version 1
function sign(n: int): int {
  if (n < 0) { return 0 - 1 }
  if (n == 0) { return 0 }
  return 1
}
"#;
    for n in [-5, -1, 0, 1, 7] {
        both_agree(src, "sign", &[Value::Int(n)]);
    }
    assert_eq!(emitted(src, "sign", &[Value::Int(-5)]).unwrap(), Value::Int(-1));
    assert_eq!(emitted(src, "sign", &[Value::Int(0)]).unwrap(), Value::Int(0));
    assert_eq!(emitted(src, "sign", &[Value::Int(9)]).unwrap(), Value::Int(1));
}

/// A shape this back end does not emit is said so, rather than pushed as
/// `false`.
///
/// It used to compile to a module that computed a wrong answer and reported
/// nothing — which is worse than a missing back end, because the parity test
/// only compares what both of them have.
#[test]
fn what_this_back_end_cannot_emit_it_refuses() {
    let src = r#"
app "x.y"
version 1

capabilities {
}

state {
  n: int default 0
}

function held(n: int): int {
  const f = { x -> x * 2 }
  return n
}
"#;
    let (program, _) = valang::analyse(src);
    let out = compile_function(&program);
    let Err(said) = out else {
        panic!("a function this back end cannot emit compiled anyway");
    };
    assert!(said.iter().any(|m| m.contains("a function written in place")), "{said:?}");
}

/// What was added to the language after this back end was written, and what it
/// now emits: `exists`, `?:`, a list written out, a record built or derived.
/// Each compared against the evaluator, which knows nothing about Wasm.
#[test]
fn the_shapes_added_since_agree_on_both_back_ends() {
    let src = r#"
app "x"
version 1

type Row {
  a: int
}

function present(n: int): bool {
  return n exists
}

function orElse(n: int): int {
  return n ?: 7
}

function listed(n: int): List<int> {
  return [n, n + 1]
}

function built(n: int): Row {
  return { a: n }
}

function derived(n: int): Row {
  return { ...built(n), a: n + 1 }
}

function nested(n: int): int {
  return n ?: (n ?: 3)
}
"#;
    for name in ["present", "orElse", "listed", "built", "derived", "nested"] {
        for n in [0i64, 1, 42] {
            both_agree(src, name, &[Value::Int(n)]);
        }
    }
}

/// A variable, written again, and a record taken apart. Both back ends have to
/// agree about what a name holds after a branch has written it.
#[test]
fn a_variable_and_a_destructuring_agree_on_both_back_ends() {
    let src = r#"
app "x"
version 1

function label(points: int): int {
  let out = 1
  if (points >= 100) {
    out = 2
  }
  if (points >= 1000) {
    out = 3
  }
  return out
}

function fromRecord(n: int): int {
  const { a, b } = { a: n, b: n + 1 }
  return a + b
}
"#;
    for n in [0i64, 50, 100, 5_000] {
        both_agree(src, "label", &[Value::Int(n)]);
    }
    for n in [0i64, 7] {
        both_agree(src, "fromRecord", &[Value::Int(n)]);
    }
}

/// The list operations, as a loop in the module. Every one compared against the
/// tree-walking evaluator, over an empty list as well as a full one — `any` and
/// `all` over nothing are the answers people get wrong.
#[test]
fn the_list_operations_agree_on_both_back_ends() {
    let src = r#"
app "x"
version 1

function double(x: int): int {
  return x * 2
}

function add(a: int, b: int): int {
  return a + b
}

function rows(n: int): List<int> {
  return n <= 0 ? [] : [1, 2, 3]
}

function mapped(n: int): List<int> {
  return rows(n).map { r -> r * 10 }
}

function mappedByName(n: int): List<int> {
  return rows(n).map(double)
}

function kept(n: int): List<int> {
  return rows(n).filter { r -> r > 1 }
}

function summed(n: int): int {
  return rows(n).fold(0) { sum, r -> sum + r }
}

function summedByName(n: int): int {
  return rows(n).fold(0, add)
}

function anyBig(n: int): bool {
  return rows(n).any { r -> r > 2 }
}

function allBig(n: int): bool {
  return rows(n).all { r -> r > 2 }
}

function howMany(n: int): int {
  return rows(n).count
}

function theFirst(n: int): int {
  return rows(n).first ?: 0
}

function nested(n: int): int {
  return rows(n).map { r -> r * 2 }.fold(0) { sum, r -> sum + r }
}
"#;
    for name in [
        "mapped",
        "mappedByName",
        "kept",
        "summed",
        "summedByName",
        "anyBig",
        "allBig",
        "howMany",
        "theFirst",
        "nested",
    ] {
        for n in [0i64, 1] {
            both_agree(src, name, &[Value::Int(n)]);
        }
    }
}

/// The reason a loop in the module is worth having: totality says the program
/// halts, fuel says when. A list operation over a long list is a long loop, and
/// a long loop on a small budget stops rather than keeping somebody waiting.
#[test]
fn a_list_operation_is_a_loop_the_fuel_meter_can_see() {
    let src = r#"
app "x"
version 1

function summed(n: int): int {
  return [1, 2, 3, 4, 5, 6, 7, 8].fold(0) { sum, r -> sum + r }
}
"#;
    let (program, _) = valang::analyse(src);
    let module = compile_function(&program).expect("every shape here is one both back ends have");

    assert_eq!(
        run_with_fuel(&module, "summed", &[Value::Int(0)], Some(100_000)).unwrap(),
        Value::Int(36)
    );
    assert!(
        run_with_fuel(&module, "summed", &[Value::Int(0)], Some(20)).is_err(),
        "a budget of twenty instructions finished a loop over eight rows"
    );
}

/// 10. Slots, under pressure. Each list operation holds five, each `?:` holds
/// one, and a function that has several of them in branches and inside each
/// other is where the arithmetic goes wrong.
#[test]
fn many_operations_in_one_function_agree_on_both_back_ends() {
    let src = r#"
app "x"
version 1

function rows(n: int): List<int> {
  return n <= 0 ? [] : [1, 2, 3]
}

function busy(n: int): int {
  let out = 0
  if (n > 0) {
    out = rows(n).fold(0) { sum, r -> sum + r }
  } else {
    out = rows(n).map { r -> r * 2 }.fold(0) { sum, r -> sum + r }
  }
  const also = rows(n).filter { r -> r > 1 }.count
  const maybe = rows(n).first ?: (rows(n).first ?: 9)
  return out + also + maybe
}

function seeded(n: int): int {
  return rows(n).fold(rows(n).count) { sum, r -> sum + r }
}
"#;
    for name in ["busy", "seeded"] {
        for n in [0i64, 1, 5] {
            both_agree(src, name, &[Value::Int(n)]);
        }
    }
}
