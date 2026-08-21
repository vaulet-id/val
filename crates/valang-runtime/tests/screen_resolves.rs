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

/// `order by purchased_at desc` was parsed and thrown away, so a screen asking
/// for its receipts newest first got whatever order the wallet answered in.
#[test]
fn a_screen_gets_its_rows_in_the_order_it_asked_for() {
    let src = r#"
app "x.y"
version 1

capabilities {
  credential.read(PurchaseReceipt)
}

credential PurchaseReceipt as "https://org.vaulet.id/example/credential/purchase-receipt" {
  merchant:     string
  amount:       int
  purchased_at: datetime
}

trust AnyReceipt(r: PurchaseReceipt) {
  anchor: "th.co.codefin.merchants"
  require {
    r.signature.valid
  }
}

state {
  n: int default 0
}

@main
screen Home {
  data {
    newest: credentials of PurchaseReceipt verified with AnyReceipt
      order by amount desc
      limit 2
  }

  column {
    list(newest) { r ->
      text(r.claims.amount)
    }
  }
}
"#;
    let (program, d) = valang::analyse_fully(src, None, &hosts());
    assert!(d.iter().all(|x| x.severity != valang::diag::Severity::Error), "{d:?}");

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());
    let screen = render(&program, "Home", &state, &host).expect("Home resolves");

    let mut amounts = Vec::new();
    fn walk(c: &valang_runtime::render::Component, out: &mut Vec<i64>) {
        for v in c.args.values() {
            if let valang_runtime::value::Value::Int(i) = v {
                out.push(*i);
            }
        }
        for k in &c.children {
            walk(k, out);
        }
    }
    for n in &screen.tree {
        walk(n, &mut amounts);
    }

    assert_eq!(amounts.len(), 2, "the limit was not applied: {amounts:?}");
    assert!(amounts[0] >= amounts[1], "the rows came back in another order: {amounts:?}");

    // And they are the two largest, which is what sorting before cutting means.
    use valang_runtime::host::Host as _;
    let all: Vec<i64> = host
        .credentials_of("PurchaseReceipt", None, None, None)
        .into_iter()
        .filter_map(|r| match r.get("amount") {
            Some(valang_runtime::value::Value::Int(i)) => Some(*i),
            _ => None,
        })
        .collect();
    let mut largest = all.clone();
    largest.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(amounts, largest[..2], "the limit cut before the sort: {all:?}");
}
