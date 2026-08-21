//! What a wallet is told an application can do, from the module and nothing
//! else.
//!
//! The values here were read off the examples by hand. There is one route now —
//! the walk over the source is gone — so a fixture taken from either end of it
//! would be the compiler agreeing with itself.

use std::collections::BTreeSet;

use valang::capability::{Host, Hosts};

fn registries() -> Hosts {
    Hosts::of(vec![Host::parse(include_str!("../../../hosts/core.json")).expect("core parses")])
}

fn report(src: &str) -> valang::report::Report {
    let (program, diagnostics) = valang::analyse_fully(src, None, &registries());
    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    assert!(errors.is_empty(), "the example does not compile: {errors:?}");
    valang_wasm::report_of(&program).unwrap_or_else(|missing| panic!("not emitted: {missing:?}"))
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// The example the specification is written around. `loyalty.val` reads two
/// claims of a receipt checked against the merchant's policy, writes three
/// fields of state, and issues a membership. It discloses nothing and proves
/// nothing, so there is nothing about it a person cannot take back.
#[test]
fn a_loyalty_card_reads_two_claims_and_issues_one_credential() {
    let r = report(include_str!("../../../examples/loyalty.val"));

    assert_eq!(
        r.reads,
        set(&["PurchaseReceipt.amount, PurchaseReceipt.purchased_at under ReceiptFromMerchant"])
    );
    assert_eq!(r.issues, set(&["LoyaltyMember"]));
    assert_eq!(r.writes, set(&["lifetimePoints", "member.points", "member.tier"]));
    assert!(r.discloses.is_empty(), "{r}");
    assert!(r.proves.is_empty(), "{r}");
    assert!(!r.irreversible, "nothing here is irreversible:\n{r}");
}

/// The one that carries the rule. `door.val` proves somebody is over twenty
/// **without disclosing when they were born**, and discloses only the country.
///
/// The birthdate is not read, and it used to be reported as read: the claim is
/// written in the predicate, and a walk over the source saw it there. So the
/// sheet said "reads your birthdate" about an application that cannot. `prove`
/// is a host call that takes nothing — the host evaluates the statement and
/// builds the proof, because it is the only party that can — so the module has
/// no import for the birthdate and no way to reach it.
#[test]
fn proving_an_age_is_not_reading_a_birthdate() {
    let r = report(include_str!("../../../examples/door.val"));

    assert_eq!(r.discloses, set(&["NationalId.country"]));

    // **It reads nothing.** It checks the ID against the government's key —
    // its own line — and reads no claim off it at all. Disclosing is not
    // reading: the host fetches the country and hands it to whoever is being
    // shown it, so the module never holds it and cannot keep it, compute on it,
    // or put it anywhere else. Neither is proving.
    assert!(r.reads.is_empty(), "an application that reads nothing:\n{r}");
    assert_eq!(r.checks, set(&["NationalId under GovernmentIssued"]));
    assert_eq!(r.proves.len(), 1, "one statement is proved:\n{r}");
    assert!(r.irreversible, "a disclosure cannot be taken back:\n{r}");
}

/// A screen's `data` is a capability too — it reads credentials before anything
/// is drawn — and a query names the audience the manifest fixed rather than the
/// operation asked of them.
#[test]
fn a_screen_reads_before_it_draws() {
    let r = report(include_str!("../../../examples/portfolio.val"));
    assert_eq!(r.audiences, set(&["broker.co.th"]));
}
