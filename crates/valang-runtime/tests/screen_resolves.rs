//! Every screen in the examples resolves.
//!
//! `val_render` drops a screen that fails, so an application whose screen threw
//! showed the next one instead of an error — which is how a broken screen looks
//! exactly like a screen somebody forgot to write.

use std::collections::BTreeMap;

use valang::capability::{Host, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;

const CATALOGUE: &str = include_str!("../../../examples/catalogue.val");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");
const CORE: &str = include_str!("../../../hosts/core.json");

/// Against the registry, which is how a host runs it. Analysed without one, a
/// positional argument keeps its index instead of the name the registry gives
/// it — so a test that skipped the registry was testing a program no host would
/// admit, and a list drew no rows without anybody noticing.
fn hosts() -> Hosts {
    Hosts::of(vec![Host::parse(CORE).expect("the core registry parses")])
}

#[test]
fn every_screen_of_the_catalogue_resolves() {
    let (program, d) = valang::analyse_fully(CATALOGUE, None, &hosts());
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());

    for screen in &program.screens {
        let out = render(&program, &screen.name, &state, &host);
        assert!(out.is_ok(), "`{}` did not resolve: {:?}", screen.name, out.err());
    }
}

/// A list draws its rows.
///
/// `list(receipts) { … }` is expanded by the compiler, and the renderer reads
/// the argument by the name the registry gives it. Both halves have to agree,
/// and for a while they did not: one wrote `of` and the other read `0`, so the
/// list came out empty and every screen still resolved.
#[test]
fn a_list_draws_a_row_for_each_item() {
    let (program, d) = valang::analyse_fully(CATALOGUE, None, &hosts());
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());

    let mut lists = 0;
    for screen in &program.screens {
        let Ok(out) = render(&program, &screen.name, &state, &host) else { continue };
        fn walk(c: &valang_runtime::render::Component, lists: &mut usize) {
            if c.kind == "list" {
                assert!(!c.children.is_empty(), "a list drew no rows");
                *lists += 1;
            }
            for child in &c.children {
                walk(child, lists);
            }
        }
        for node in &out.tree {
            walk(node, &mut lists);
        }
    }
    assert!(lists > 0, "the catalogue draws no list, so this test proves nothing");
}
