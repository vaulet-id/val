//! One action, two engines, and everything about the run compared.
//!
//! The tree-walking evaluator is what a build with a compiler in it uses; the
//! module is what a phone uses, because a phone has no compiler. They are two
//! implementations of one language, so the only thing worth asserting is that
//! they agree — and about the whole of a run, not about the answer: the state
//! that came out, what the host was asked to do, the roots, and the outcome.

use std::collections::BTreeMap;

use valang::capability::{Host as Registry, Hosts};
use valang_runtime::fixture::Fixture;
use valang_runtime::{run_action, run_action_with, About, Run};

const WALLET: &str = include_str!("../../../fixtures/wallet.json");

fn registries() -> Hosts {
    Hosts::of(vec![Registry::parse(include_str!("../../../hosts/core.json")).expect("core parses")])
}

/// Both runs of one action: the walk, and the module.
fn both(src: &str, action: &str) -> (Run, Run) {
    let (program, diagnostics) = valang::analyse_fully(src, None, &registries());
    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    assert!(errors.is_empty(), "the example does not compile: {errors:?}");

    let module = valang_wasm::compile::compile_program(&program)
        .unwrap_or_else(|missing| panic!("not emitted: {missing:?}"));

    let host = Fixture::parse(WALLET).expect("the wallet parses");
    let state = valang_runtime::initial_state(&program, &host.state());
    let input = BTreeMap::new();

    let walked = run_action(&program, src, action, &state, &input, &host);

    // The module's run is given the same code hash so the two records are
    // comparable. A wallet passes the hash of the module, because that is what
    // ran there — which is the point of the hash being an argument.
    let code_hash: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(src.as_bytes()).into();
    let mut engine = valang_wasm::WasmEngine::new(&module);
    let ran = run_action_with(
        &About::of(&program),
        code_hash,
        action,
        &state,
        &input,
        &host,
        &mut engine,
    );
    (walked, ran)
}

fn agree(walked: &Run, ran: &Run, about: &str) {
    assert_eq!(format!("{:?}", ran.outcome), format!("{:?}", walked.outcome), "the outcome of {about}");
    assert_eq!(ran.next_state, walked.next_state, "the state {about} produced");
    assert_eq!(ran.record.next_root, walked.record.next_root, "the root after {about}");
    assert_eq!(ran.effects.len(), walked.effects.len(), "how much {about} asked for");
    for (a, b) in ran.effects.iter().zip(&walked.effects) {
        assert_eq!(a.capability, b.capability, "which capability {about} asked for");
        assert_eq!(a.payload, b.payload, "what {about} asked to have done");
        assert_eq!(a.reversible, b.reversible, "whether it can be taken back");
    }
}

/// The example the specification is written around: a receipt read, points
/// computed, three fields written, a membership issued.
#[test]
fn the_two_engines_agree_about_earning_points() {
    let (walked, ran) = both(include_str!("../../../examples/loyalty.val"), "ScanToEarn");
    agree(&walked, &ran, "ScanToEarn");
    assert!(
        matches!(ran.outcome, valang_runtime::Outcome::Committed),
        "the module's run did not commit: {:?}",
        ran.outcome
    );
}

/// The one where an effect goes out. `present { disclose …; prove … }` is one
/// request carrying both lines — which is why the module builds the parts and
/// hands them over together rather than sending each as it goes.
#[test]
fn the_two_engines_agree_about_a_disclosure() {
    let (walked, ran) = both(include_str!("../../../examples/door.val"), "EnterVenue");
    agree(&walked, &ran, "EnterVenue");
    assert_eq!(ran.effects.len(), 1, "one `present` is one request: {:?}", ran.effects);
}

/// Every action of every example, which is the only version of this test worth
/// having: two engines agreeing about one program says little, and the shapes
/// that break are the ones nobody thought to write a case for.
#[test]
fn the_two_engines_agree_about_every_example() {
    let examples: [(&str, &str); 9] = [
        ("loyalty", include_str!("../../../examples/loyalty.val")),
        ("door", include_str!("../../../examples/door.val")),
        ("condo", include_str!("../../../examples/condo.val")),
        ("transit", include_str!("../../../examples/transit.val")),
        ("portfolio", include_str!("../../../examples/portfolio.val")),
        ("referendum", include_str!("../../../examples/referendum.val")),
        ("note", include_str!("../../../examples/note.val")),
        ("catalogue", include_str!("../../../examples/catalogue.val")),
        ("syntax", include_str!("../../../examples/syntax.val")),
    ];

    let mut ran_any = false;
    for (name, src) in examples {
        let (program, diagnostics) = valang::analyse_fully(src, None, &registries());
        assert!(
            !diagnostics.iter().any(|d| d.severity == valang::Severity::Error),
            "{name} does not compile"
        );
        for action in &program.actions {
            let (walked, ran) = both(src, &action.name);
            agree(&walked, &ran, &format!("{name}.{}", action.name));
            ran_any = true;
        }
    }
    assert!(ran_any, "no example carried an action, so this compared nothing");
}

/// The path that is not a commit. An application declining for its own reasons
/// is an ordinary outcome, and the record has to say the same thing whichever
/// engine produced it — including which key in the signed bundle the person is
/// told from.
#[test]
fn the_two_engines_agree_about_a_refusal() {
    const REFUSING: &str = r#"app "x.y"
version 1

capabilities {
  credential.read(PurchaseReceipt)
}

credential PurchaseReceipt as "https://org.vaulet.id/example/credential/purchase-receipt" {
  amount: int
}

trust P(r: PurchaseReceipt) {
  anchor: "shop.example.com"
  require {
    r.signature.valid
  }
}

state {
  points: int default 0
}

action Earn {
  input {
    r: Credential<PurchaseReceipt>
  }

  verify {
    const checked = r with P
  }

  compute {
    if (checked.claims.amount < 1000000) {
      refuse "tooSmallToEarn"
    }
  }

  update {
    points: 1
  }
}
"#;
    let (walked, ran) = both(REFUSING, "Earn");
    agree(&walked, &ran, "a refusal");
    assert!(
        matches!(&ran.outcome, valang_runtime::Outcome::Declined(k) if k == "tooSmallToEarn"),
        "the module did not decline in the application's own words: {:?}",
        ran.outcome
    );
}

/// Money, which is the one effect where what the sheet says and what the host
/// is handed are deliberately different things: the sheet names the recipient,
/// and the amount travels in the request for the host to show at the moment.
/// The two engines still have to agree about the request itself.
#[test]
fn the_two_engines_agree_about_a_payment() {
    const PAYING: &str = r#"app "th.co.codefin.pay"
version 1

capabilities {
  credential.read(TransitPass)
  payment.request(to: "shop.example.com")
}

credential TransitPass as "https://org.vaulet.id/example/credential/transit-pass" {
  zone: string
}

trust IssuedByOperator(t: TransitPass) {
  anchor: "shop.example.com"
  require {
    t.signature.valid
  }
}

state {
  paid: int default 0
}

action Settle {
  input {
    ticket: Credential<TransitPass>
  }

  verify {
    const checked = ticket with IssuedByOperator
  }

  compute {
    const owed = state.paid + 25
  }

  update {
    paid: owed
  }

  execute {
    payment.request(to: "shop.example.com", amount: owed)
  }
}
"#;
    let (walked, ran) = both(PAYING, "Settle");
    agree(&walked, &ran, "a payment");

    assert_eq!(ran.effects.len(), 1, "one payment is one request: {:?}", ran.effects);
    let paid = &ran.effects[0];
    assert_eq!(paid.capability, "payment.request");
    assert!(!paid.reversible, "money that moved has moved");
    match &paid.payload {
        valang_runtime::value::Value::Map(m) => {
            assert_eq!(m.get("to"), Some(&valang_runtime::value::Value::Str("shop.example.com".into())));
            assert!(matches!(m.get("amount"), Some(valang_runtime::value::Value::Int(_))), "{m:?}");
        }
        other => panic!("a payment is a request with a recipient and an amount: {other:?}"),
    }
}

/// **A disclosure is not something the person can undo, and both engines have
/// to say so.**
///
/// They said different things: the module-running side had it written out at
/// the call site, and the source-walking side asked the registry under the
/// statement's name — `present` — while the registry declares it as
/// `disclosure.present`, so it found nothing and answered "reversible".
///
/// The sheet renders that difference as a warning icon, which is the whole of
/// what it is for.
#[test]
fn both_engines_agree_that_a_disclosure_cannot_be_taken_back() {
    let src = r#"
app "x.y"
version 1

capabilities {
  credential.read(NationalId)
  credential.issue(Card)
  disclosure.present
}

credential NationalId as "https://dopa.go.th/credential/national-id" {
  given_name:  string
  family_name: string
  birthdate:   date
  country:     string
}

credential Card as "https://org.vaulet.id/example/credential/card" {
  who: string
}

trust Issued(id: NationalId) {
  anchor: "th.go.dopa"
  require {
    id.signature.valid
  }
}

state { n: int default 0 }

action Go {
  input {
    id: Credential<NationalId>
  }

  verify {
    const checked = id with Issued
  }

  update {
    n: 1
  }

  execute {
    present {
      disclose checked.claims.country
    }
    credential.issue(Card { who: "me" })
  }
}

@main
screen Home {
  column {
    button(text: "go", onTap: Go)
  }
}
"#;
    let (ran, walked) = both(src, "Go");

    // Written disclosure-first and offered issue-first: what cannot be taken
    // back goes last, and the compiler is what puts it there.
    let order: Vec<&str> = ran.effects.iter().map(|e| e.capability.as_str()).collect();
    assert_eq!(order, ["credential.issue", "disclosure.present"], "{:?}", ran.effects);

    for effects in [&ran.effects, &walked.effects] {
        let present = effects.iter().find(|e| e.capability == "disclosure.present").expect("one");
        assert!(!present.reversible, "a disclosure cannot be taken back");
        let issued = effects.iter().find(|e| e.capability == "credential.issue").expect("one");
        assert!(issued.reversible, "a credential can be revoked by whoever issued it");
    }
}
