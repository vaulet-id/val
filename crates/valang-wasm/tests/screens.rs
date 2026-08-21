//! The tree a host draws, from the module and from the evaluator.
//!
//! A wallet has no compiler, so what draws its screens is the module. The
//! tree-walking resolver is the other implementation, and the only thing worth
//! asserting about two of them is that they agree — about the whole tree, not
//! about its shape: every node, every slot, every row a list drew.

use std::collections::BTreeMap;

use valang::capability::{Host as Registry, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::render::{render, Component};
use valang_runtime::value::Value;

const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn registries() -> Hosts {
    Hosts::of(vec![
        Registry::parse(include_str!("../../../hosts/core.json")).expect("core parses"),
        Registry::parse(include_str!("../../../hosts/vaulet.json")).expect("vaulet parses"),
    ])
}

/// The resolver's tree, in the shape the module answers with, so the two can be
/// compared as one value rather than field by field.
fn as_value(nodes: &[Component]) -> Value {
    Value::List(
        nodes
            .iter()
            .map(|c| {
                let mut m = BTreeMap::new();
                m.insert("kind".to_string(), Value::Str(c.kind.clone()));
                m.insert("args".to_string(), Value::Map(c.args.clone()));
                m.insert("children".to_string(), as_value(&c.children));
                Value::Map(m)
            })
            .collect(),
    )
}

fn both(src: &str, screen: &str) -> (Value, Value) {
    let (program, diagnostics) = valang::analyse_fully(src, None, &registries());
    assert!(
        !diagnostics.iter().any(|d| d.severity == valang::Severity::Error),
        "the example does not compile: {diagnostics:?}"
    );
    let module = valang_wasm::compile::compile_program(&program)
        .unwrap_or_else(|missing| panic!("not emitted: {missing:?}"));

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &host.state());

    let walked = render(&program, screen, &state, &host).expect("the resolver draws it");
    let about = valang_runtime::About::of(&program);
    let mut engine = valang_wasm::WasmEngine::new(&module);
    let drawn = engine.screen(screen, &about, &state, &host).expect("the module draws it");

    (as_value(&walked.tree), drawn)
}

/// Every screen of every example, which is the only version of this worth
/// having: the shapes that break are the ones nobody wrote a case for — a list
/// inside a list, an `if` with no `else`, a slot holding a word rather than a
/// value.
#[test]
fn the_two_draw_every_screen_of_every_example() {
    let examples: [(&str, &str); 7] = [
        ("loyalty", include_str!("../../../examples/loyalty.val")),
        ("door", include_str!("../../../examples/door.val")),
        ("condo", include_str!("../../../examples/condo.val")),
        ("transit", include_str!("../../../examples/transit.val")),
        ("portfolio", include_str!("../../../examples/portfolio.val")),
        ("catalogue", include_str!("../../../examples/catalogue.val")),
        ("note", include_str!("../../../examples/note.val")),
    ];

    let mut drew = 0;
    for (name, src) in examples {
        let (program, _) = valang::analyse_fully(src, None, &registries());
        for screen in &program.screens {
            let (walked, drawn) = both(src, &screen.name);
            assert_eq!(drawn, walked, "{name}.{} is drawn differently", screen.name);
            drew += 1;
        }
    }
    assert!(drew >= 5, "only {drew} screens were compared, which is fewer than these examples carry");
}

/// **An audience this host cannot ask is not an audience that answered
/// nothing**, and the screen is refused rather than drawn empty.
///
/// It used to be drawn: `query` answered with a list, a host with no way to
/// reach anybody answered with an empty one, and a person looking at
/// "holdings: none" could not tell that from a broker who had not been asked.
#[test]
fn a_screen_whose_query_cannot_be_asked_is_not_drawn() {
    let src = r#"
app "x.y"
version 1

capabilities {
  api.query(audience: "broker.co.th")
}

state { n: int default 0 }

@main
screen Home {
  data {
    quotes: query broker.quotes
  }

  column {
    list(quotes) { q -> tile(text: "row") }
  }
}
"#;
    let (program, diagnostics) = valang::analyse_fully(src, None, &registries());
    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");

    // A wallet holding nothing and able to ask nobody. The default `query`
    // answers `None`, which is what every host that has not implemented one
    // does — this one included, until it does.
    struct Silent;
    impl valang_runtime::host::Host for Silent {
        fn context(&self) -> valang_runtime::host::Context {
            valang_runtime::host::Context { time_now: 0, random_uuid: String::new() }
        }
        fn credential(
            &self,
            _ty: &str,
            _policy: Option<&str>,
        ) -> Option<std::collections::BTreeMap<String, valang_runtime::value::Value>> {
            None
        }
        fn decide(
            &self,
            _effects: &[valang_runtime::host::EffectRequest],
        ) -> valang_runtime::host::Verdict {
            valang_runtime::host::Verdict::Approved
        }
        fn sign(&self, _bytes: &[u8]) -> Vec<u8> {
            Vec::new()
        }
        fn device_key(&self) -> Vec<u8> {
            Vec::new()
        }
    }

    let walked = valang_runtime::render::render_with(
        &program,
        "Home",
        &Default::default(),
        &Default::default(),
        &Silent,
    );
    assert!(walked.is_err(), "a screen nobody could answer was drawn anyway");

    let module = valang_wasm::compile::compile_program(&program).expect("it compiles");
    let mut engine = valang_wasm::WasmEngine::new(&module);
    let about = valang_wasm::compile::about_of(&module.bytes).expect("about");
    let ran = engine.screen("Home", &about, &Default::default(), &Silent);
    assert!(ran.is_err(), "and the other engine drew it: {ran:?}");
}
