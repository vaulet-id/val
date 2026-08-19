//! An imported component draws on the screen it was expanded into.

use std::collections::BTreeMap;

use valang::capability::{Host, Hosts};
use valang::expand::Packages;
use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;

const CORE: &str = include_str!("../../../hosts/core.json");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");
const KIT: &str = include_str!("../../../examples/kit.val");
const STOREFRONT: &str = include_str!("../../../examples/storefront.val");

fn words(c: &valang_runtime::render::Component, out: &mut Vec<String>) {
    for v in c.args.values() {
        if let valang_runtime::value::Value::Str(s) = v {
            out.push(s.clone());
        }
    }
    for child in &c.children {
        words(child, out);
    }
}

#[test]
fn the_storefront_draws_the_kit() {
    let hosts = Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")]);
    let packages = Packages::of(vec![valang::analyse(KIT).0]);
    let (program, d) = valang::analyse_with_packages(STOREFRONT, None, &hosts, &packages);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());
    let screen = render(&program, "Home", &state, &host).expect("Home resolves");

    let mut found = Vec::new();
    for node in &screen.tree {
        words(node, &mut found);
    }

    for want in ["Balance", "since March", "Visits this month", "Spend 100"] {
        assert!(found.iter().any(|f| f.contains(want)), "`{want}` is not drawn: {found:?}");
    }
}
