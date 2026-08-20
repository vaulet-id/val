//! Loops over what the wallet answered with, and the two loop shapes nested.

use std::collections::BTreeMap;

use valang::capability::{Host, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::render::render;
use valang_runtime::value::Value;

const CORE: &str = include_str!("../../../hosts/core.json");
const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn drawn(src: &str) -> Vec<String> {
    let hosts = Hosts::of(vec![Host::parse(CORE).unwrap()]);
    let (program, d) = valang::analyse_fully(src, None, &hosts);
    assert!(
        d.iter().all(|x| x.severity != valang::diag::Severity::Error),
        "{:?}",
        d.iter().map(|x| &x.message).collect::<Vec<_>>()
    );
    let host = Fixture::parse(WALLET).unwrap();
    let state = valang_runtime::initial_state(&program, &BTreeMap::new());
    let screen = render(&program, "Home", &state, &host).expect("Home resolves");

    let mut out = Vec::new();
    fn walk(c: &valang_runtime::render::Component, out: &mut Vec<String>) {
        let text = c.args.values().find_map(|v| match v {
            Value::Str(s) => Some(s.clone()),
            Value::Int(i) => Some(i.to_string()),
            _ => None,
        });
        out.push(match text {
            Some(t) => format!("{}({t})", c.kind),
            None => c.kind.clone(),
        });
        for k in &c.children {
            walk(k, out);
        }
    }
    for n in &screen.tree {
        walk(n, &mut out);
    }
    out
}

/// A loop over what the wallet answered with, rather than over a range or a
/// list written out — which is what a real screen does and what nothing had
/// tested.
#[test]
fn a_loop_over_the_wallets_own_rows() {
    let src = r#"
app "x.y"
version 1

capabilities {
  credential.read(PurchaseReceipt)
}

credential PurchaseReceipt {
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
    rows: credentials of PurchaseReceipt verified with AnyReceipt limit 2
  }

  column {
    for (r in rows) {
      text(r.claims.merchant)
    }
  }
}
"#;
    let out = drawn(src);
    assert_eq!(out.len(), 3, "a loop over the wallet's rows drew {out:?}");
    assert!(out[1].starts_with("text("), "{out:?}");
}

/// A loop inside a list's row, and a branch inside a loop: the two shapes
/// that are the language's own, one inside the other.
#[test]
fn a_loop_inside_a_row_and_a_branch_inside_a_loop() {
    let src = r#"
app "x.y"
version 1

capabilities {
}

state {
  n: int default 2
}

@main
screen Home {
  column {
    list([1, 2]) { a ->
      for (b in 1...2) {
        if (b > 1) {
          text(b)
        } else {
          section(a)
        }
      }
    }
  }
}
"#;
    assert_eq!(
        drawn(src),
        vec![
            "column",
            "list",
            "section(1)",
            "text(2)",
            "section(2)",
            "text(2)",
        ]
    );
}
