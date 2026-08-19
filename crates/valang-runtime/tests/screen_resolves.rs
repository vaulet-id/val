//! Every screen in the examples resolves.
//!
//! `val_render` drops a screen that fails, so an application whose screen threw
//! showed the next one instead of an error — which is how a broken screen looks
//! exactly like a screen somebody forgot to write.

use std::collections::BTreeMap;

use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;

const CATALOGUE: &str = include_str!("../../../examples/catalogue.val");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

#[test]
fn every_screen_of_the_catalogue_resolves() {
    let (program, d) = valang::analyse(CATALOGUE);
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());

    for screen in &program.screens {
        let out = render(&program, &screen.name, &state, &host);
        assert!(out.is_ok(), "`{}` did not resolve: {:?}", screen.name, out.err());
    }
}
