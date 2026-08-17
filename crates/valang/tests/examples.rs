//! The examples are the fixtures, and they were written before this crate — by
//! reading the specification, not by reading the code that now has to satisfy
//! them. `rejected.val` in particular is a checklist somebody else wrote: each
//! numbered program carries the error it is owed, in a comment, and this test
//! asserts the compiler says something with the same shape.

use valang::{analyse, report::report, Severity};

fn errors(src: &str) -> Vec<String> {
    analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}: {}", d.span.line, d.message))
        .collect()
}

const LOYALTY: &str = include_str!("../../../examples/loyalty.val");
const DOOR: &str = include_str!("../../../examples/door.val");
const WALLET: &str = include_str!("../../../examples/wallet.val");
const PORTFOLIO: &str = include_str!("../../../examples/portfolio.val");
const REJECTED: &str = include_str!("../../../examples/rejected.val");

#[test]
fn the_valid_examples_have_nothing_to_say_about_them() {
    for (name, src) in [("loyalty", LOYALTY), ("door", DOOR), ("wallet", WALLET), ("portfolio", PORTFOLIO)] {
        let e = errors(src);
        assert!(e.is_empty(), "{name}.val should compile clean, got:\n  {}", e.join("\n  "));
    }
}

#[test]
fn loyalty_parses_into_the_shape_the_document_describes() {
    let (p, _) = analyse(LOYALTY);
    assert_eq!(p.app.as_deref(), Some("th.co.codefin.loyalty"));
    assert_eq!(p.capabilities.len(), 2);
    assert_eq!(p.credentials.len(), 2);
    assert_eq!(p.trusts.len(), 1);
    assert_eq!(p.trusts[0].anchor.as_deref(), Some("th.co.codefin.merchants"));
    assert_eq!(p.actions.len(), 1);

    let phases: Vec<_> = p.actions[0].phases.iter().map(|b| b.phase.name()).collect();
    assert_eq!(phases, ["input", "require", "verify", "compute", "update", "execute"]);
}

#[test]
fn the_report_is_derived_not_declared() {
    let (p, _) = analyse(LOYALTY);
    let r = report(&p);
    assert!(r.reads.iter().any(|s| s == "PurchaseReceipt under ReceiptFromMerchant"), "{r}");
    assert!(r.issues.contains("LoyaltyMember"), "{r}");
    assert!(r.discloses.is_empty(), "{r}");
    assert!(!r.irreversible, "{r}");
    // The three lines a reviewer came to read, and nothing that was not written.
    assert_eq!(
        r.writes.iter().cloned().collect::<Vec<_>>(),
        ["lifetimePoints", "member.points", "member.tier"]
    );
}

#[test]
fn a_portfolio_proves_and_discloses_nothing() {
    let (p, _) = analyse(PORTFOLIO);
    let r = report(&p);
    assert!(r.discloses.is_empty(), "a portfolio proving accreditation discloses nothing:\n{r}");
    assert_eq!(r.proves.len(), 1, "{r}");
    assert!(r.irreversible, "a proof is still irreversible:\n{r}");
    assert!(r.audiences.contains("broker.co.th"), "{r}");
}

/// Each of these is one of the numbered programs in `rejected.val`, and the
/// fragment is from the comment above it — written when the rule was decided,
/// not when this checker was.
#[test]
fn rejected_is_rejected_for_the_reasons_it_says() {
    let found = errors(REJECTED);
    let joined = found.join("\n");

    for want in [
        "effect and `compute` is pure",           // 1. an effect in a pure phase
        "functions are pure",                     // 2. an effect behind a function
        "used and never declared",                // 3. a capability never declared
        "performs 2 disclosures",                 // 9d. two disclosures in one action
        "may not use `default`",                  // 9. a wildcard over an enum
        "unreachable",                            // 9b. a dead switch arm
        "no such function",                       // 10. Date.now()
        "recursive",                              // 11. recursion
        "no floating-point type",                 // 12. a float
        "may not exist",                          // 7. an optional never narrowed
        "takes paths, not record literals",       // 9c. a record literal in a patch
        "no assignment in this language",         // 8. `state.member.points = 10`
        "patch path may not contain a list index", // 8b. `stamps[3].used:`
        "a list has no index in this language",   // inside the recursion example
        "expected `Verified<ReceiptFromMerchant>`, found `Credential<PurchaseReceipt>`", // 5.
        "found `Verified<SignatureOnly>`",        // 6. verified against the wrong policy
    ] {
        assert!(
            joined.contains(want),
            "rejected.val should be refused for {want:?}, and was not. What the compiler said:\n{joined}"
        );
    }
}

/// No cascade. A file of deliberately broken programs will have many errors —
/// that is the point of it — but one mistake that spills six messages down the
/// line buries the sentence that taught the rule, which is the failure mode
/// this file exists to prevent.
#[test]
fn no_line_says_more_than_two_things() {
    use std::collections::BTreeMap;
    let mut per_line: BTreeMap<&str, usize> = BTreeMap::new();
    let found = errors(REJECTED);
    for e in &found {
        *per_line.entry(e.split(':').next().unwrap_or("")).or_default() += 1;
    }
    let noisy: Vec<_> = per_line.iter().filter(|(_, n)| **n > 3).collect();
    assert!(noisy.is_empty(), "these lines cascade: {noisy:?}\n  {}", found.join("\n  "));

    // And no line says the same thing twice, which is the shape of it that
    // survives every other precaution.
    let mut seen = std::collections::HashSet::new();
    for e in &found {
        assert!(seen.insert(e.clone()), "said twice: {e}");
    }
}

/// Provenance is inferred and demanded only at the boundary — an issued claim.
#[test]
fn a_claim_may_demand_where_its_value_came_from() {
    let src = r#"
app "example"
version 1
capabilities { credential.read(Receipt) credential.issue(Card) }
credential Receipt { amount: int }
credential Card { points: int }
trust FromShop(r: Receipt) { anchor: "shop" require { r.signature.valid } }
action Earn {
  input { r: Credential<Receipt> }
  verify { const checked = r with FromShop }
  compute { const earned = checked.claims.amount }
  execute { credential.issue(Card { points: earned from { FromShop } }) }
}
"#;
    assert!(errors(src).is_empty(), "{:?}", errors(src));

    // The same program, with the value no longer descending from the policy.
    let broken = src.replace("const earned = checked.claims.amount", "const earned = 10");
    let e = errors(&broken).join("\n");
    assert!(e.contains("requires `FromShop`"), "provenance should be demanded here, got:\n{e}");
    assert!(e.contains("self-asserted"), "and it should say what it found instead:\n{e}");
}

/// A capability is its name and its argument. Comparing only the name let an
/// application declare one credential type and read another, which is not least
/// privilege — it is a different permission wearing the right label.
#[test]
fn declaring_one_credential_and_reading_another_is_refused() {
    let src = r#"
app "example.mismatch"
version 1
capabilities { credential.read(LoyaltyMember) }
credential LoyaltyMember { points: int }
credential Passport { document_number: string }
trust Whoever(p: Passport) { anchor: "th.go.dopa" require { p.signature.valid } }
action Peek {
  input  { passport: Credential<Passport> }
  verify { const checked = passport with Whoever }
  compute { const n = checked.claims.document_number }
}
"#;
    let e = errors(src).join("\n");
    assert!(e.contains("`credential.read(Passport)` is used and never declared"), "{e}");
    assert!(e.contains("is not least privilege"), "{e}");
}

#[test]
fn a_screen_declares_what_it_sees() {
    let (p, _) = analyse(WALLET);
    assert_eq!(p.screens.len(), 1);
    let s = &p.screens[0];
    assert_eq!(s.data.len(), 1);
    assert!(!s.tree.is_empty(), "a screen with no components would be a parse failure in disguise");
}
