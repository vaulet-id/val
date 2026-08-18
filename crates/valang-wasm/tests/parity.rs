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
    let module = compile_function(&program);
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
    let module = compile_function(&program);

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
    let module = compile_function(&program);
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
