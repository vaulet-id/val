//! The closed set, evaluated.
//!
//! The expected values are the ones a person reading `min(a: 3, b: 9)` would
//! predict, not values read back from this evaluator. The gap this guards is
//! real: the type checker accepted `min` for months while the evaluator had
//! never heard of it, so a program that compiled stopped at run time.

use std::collections::BTreeMap;

use valang_runtime::host::{Context, EffectRequest, Host, Verdict};
use valang_runtime::value::Value;
use valang_runtime::{run_action, Outcome};

const SRC: &str = r#"
app "example.builtins"
version "1.0.0"

capabilities {
}

state {
  smaller: int default 0
  larger:  int default 0
  size:    int default 0
}

action Compute {
  compute {
    const a = min(a: 3, b: 9)
    const b = max(a: 3, b: 9)
    const c = abs(-7)
  }

  update {
    smaller: a
    larger:  b
    size:    c
  }
}

screen Home {
  column {
    button(text: "go", emphasis: primary, onTap: Compute)
  }
}
"#;

/// A host with nothing in it. This action reads no credential and asks for no
/// effect, so what is left to supply is a clock and a signature.
struct Bare;

impl Host for Bare {
    fn context(&self) -> Context {
        Context { time_now: 0, random_uuid: "0".into() }
    }
    fn credential(&self, _ty: &str, _policy: Option<&str>) -> Option<BTreeMap<String, Value>> {
        None
    }
    fn decide(&self, _effects: &[EffectRequest]) -> Verdict {
        Verdict::Approved
    }
    fn sign(&self, _bytes: &[u8]) -> Vec<u8> {
        vec![0; 64]
    }
    fn device_key(&self) -> Vec<u8> {
        vec![0; 32]
    }
}

#[test]
fn min_max_and_abs_are_evaluated() {
    let (program, diagnostics) = valang::analyse(SRC);
    assert!(
        diagnostics.iter().all(|d| d.severity != valang::diag::Severity::Error),
        "{diagnostics:?}"
    );

    let run = run_action(&program, SRC, "Compute", &BTreeMap::new(), &BTreeMap::new(), &Bare);
    assert_eq!(run.outcome, Outcome::Committed, "{:?}", run.outcome);

    assert_eq!(run.next_state.get("smaller"), Some(&Value::Int(3)));
    assert_eq!(run.next_state.get("larger"), Some(&Value::Int(9)));
    assert_eq!(run.next_state.get("size"), Some(&Value::Int(7)));
}
