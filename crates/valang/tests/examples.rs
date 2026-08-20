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
    // `wallet.val` presses an action `loyalty.val` declares: it is the second
    // file of that package, not a program on its own. A package is several
    // files sharing one scope, so checking either alone would be checking half
    // of it — and the half would fail for the right reason.
    let loyalty_package = format!("{LOYALTY}\n{WALLET}");
    for (name, src) in [
        ("door", DOOR),
        ("portfolio", PORTFOLIO),
        ("the loyalty package", loyalty_package.as_str()),
    ] {
        let e = errors(src);
        assert!(e.is_empty(), "{name} should compile clean, got:\n  {}", e.join("\n  "));
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
    // The claims, not only the credential: "reads your receipts" and "reads the
    // amount and the date" are different sentences to whoever is deciding.
    assert!(
        r.reads.iter().any(|s| s == "PurchaseReceipt.amount, PurchaseReceipt.purchased_at under ReceiptFromMerchant"),
        "{r}"
    );
    assert!(r.issues.contains("LoyaltyMember"), "{r}");
    assert!(r.discloses.is_empty(), "{r}");
    assert!(!r.irreversible, "{r}");
    // The three lines a reviewer came to read, and nothing that was not written.
    assert_eq!(
        r.writes.iter().cloned().collect::<Vec<_>>(),
        ["lifetimePoints", "member.points", "member.tier"]
    );
}

/// A disclosure is reported in the person's terms, not the author's: they are
/// being asked about their national ID, not about a local binding called
/// `checked`.
#[test]
fn a_disclosure_names_the_credential_and_not_the_variable() {
    const DOOR: &str = include_str!("../../../examples/door.val");
    let (p, _) = analyse(DOOR);
    let r = report(&p);
    assert!(r.discloses.contains("NationalId.country"), "{r}");
    assert!(r.reads.iter().any(|s| s.contains("NationalId.birthdate")), "{r}");
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
        "is a `const`, so it is what it was defined as", // 13. writing a const again
        "is not where a value is worked out",     // 14. writing where nothing is worked out
        "more steps than a screen can be made of", // 15. a range longer than a screen
        "is given a function",                    // 16. a combinator with no function
        "a loop reads `for (row in rows)`",       // 17. a loop that says `of`
        "is a keyword, and a keyword is never a name", // 18. a keyword as a name
        "is pure. Effects may only appear in `execute`", // 21. a screen that acts
    ] {
        assert!(
            joined.contains(want),
            "rejected.val should be refused for {want:?}, and was not. What the compiler said:\n{joined}"
        );
    }
}

/// 19 needs the registry: a name is told from a word by asking the host what
/// its words are, and `analyse` on its own has no host to ask.
#[test]
fn a_misspelt_name_in_a_tree_is_refused_against_a_registry() {
    let hosts = valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")]);
    let msgs: Vec<String> = valang::analyse_fully(REJECTED, None, &hosts)
        .1
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(msgs.iter().any(|m| m.contains("nor a word this host has")), "{msgs:?}");
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

/// Three grades of data, and only two of them are somebody's word. An empty
/// provenance set used to cover both "this app computed it" and "an API said
/// so", which are not the same fact and must not prove the same way.
#[test]
fn a_proof_over_an_apis_answer_is_refused() {
    let src = r#"
app "example.oracle"
version 1
capabilities {
  api.query(audience: "broker.co.th", presenting: Holding)
  disclosure.present
}
credential Holding { market_value: int }
action Bad {
  verify  { const quotes = query broker.quotes() }
  compute { const total = quotes.fold(0) { sum, q -> sum + 1 } }
  execute { present { prove total >= 100 } }
}
"#;
    let e = errors(src).join("\n");
    assert!(e.contains("data from broker.co.th, which nobody signed"), "{e}");
    // Named by the audience in the manifest, not by the call — the person is
    // being told who saw their data, and `broker.quotes` is not a party.
    assert!(!e.contains("broker.quotes"), "{e}");
}

/// One batch, offered together, so there is no moment between two effects for
/// one to read what the other produced. An application written as if there were
/// is one whose author believes the state commits halfway.
#[test]
fn an_effect_cannot_read_another_effects_result() {
    let src = r#"
app "example.chain"
version 1
capabilities { credential.issue(Card) }
credential Card { points: int }
action Two {
  execute {
    const issued = credential.issue(Card { points: 1 })
    credential.issue(Card { points: issued })
  }
}
"#;
    let e = errors(src).join("\n");
    assert!(e.contains("requested, not performed, so there is nothing here to bind"), "{e}");
    assert!(e.contains("that is two actions"), "{e}");
}

/// One `capabilities` block per package, and one `app`. A person consented to a
/// list rather than to a sum of lists, and which file said what would otherwise
/// depend on the order they were read in.
#[test]
fn a_package_says_what_it_may_do_once() {
    let e = errors(&format!("{LOYALTY}\n{LOYALTY}")).join("\n");
    assert!(e.contains("declares its capabilities once"), "{e}");
    let renamed = errors(&format!("{LOYALTY}\napp \"somebody.else\"\n")).join("\n");
    assert!(renamed.contains("already calls itself"), "{renamed}");
}

/// Declaring an action binds nothing. If no screen names it, nothing can reach
/// it — and the capabilities it needs are still on the consent sheet, which is
/// the part that matters.
#[test]
fn an_action_no_screen_names_is_reported() {
    let src = r#"
app "x"
version 1
capabilities { }
action Reachable { compute { const a = 1 } }
action Orphan    { compute { const b = 2 } }
screen S { column { button(text: "go", onTap: Reachable) } }
"#;
    // Against a registry: which props hold an action is the registry's answer,
    // and without one nothing here can tell a press from any other argument.
    let hosts = valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")]);
    let (_, d) = valang::analyse_fully(src, None, &hosts);
    let said: Vec<&str> = d.iter().map(|d| d.message.as_str()).collect();
    assert!(said.iter().any(|m| m.contains("no screen names `Orphan`")), "{said:?}");
    assert!(!said.iter().any(|m| m.contains("Reachable")), "{said:?}");

    // A file may be a library: `wallet.val` presses an action `loyalty.val`
    // declares, and a package is one scope.
    let package = format!("{LOYALTY}\n{WALLET}");
    let (_, d) = analyse(&package);
    assert!(!d.iter().any(|d| d.message.contains("no screen names")), "{d:?}");

    // And a package with no screens is a fragment, not an unreachable action.
    let (_, d) = analyse(LOYALTY);
    assert!(!d.iter().any(|d| d.message.contains("no screen names")), "{d:?}");
}

#[test]
fn a_screen_declares_what_it_sees() {
    let (p, _) = analyse(&format!("{LOYALTY}\n{WALLET}"));
    assert_eq!(p.screens.len(), 1);
    let s = &p.screens[0];
    assert_eq!(s.data.len(), 1);
    assert!(!s.tree.is_empty(), "a screen with no components would be a parse failure in disguise");
}

/// Errors are outcomes, not values. There is no `Result` and no propagation,
/// because an action has nowhere to propagate to: it is a transaction, and it
/// either happens or it does not. What was missing was the third way for one
/// not to happen — the application declining for its own reasons, which is
/// neither a defect nor a trust failure.
#[test]
fn an_application_may_decline_for_its_own_reasons() {
    let src = r#"
app "example.decline"
version 1
capabilities { credential.read(Receipt) }
credential Receipt { amount: int }
trust FromShop(r: Receipt) { anchor: "shop" require { r.signature.valid } }
action Earn {
  input   { r: Credential<Receipt> }
  verify  { const checked = r with FromShop }
  compute { if (checked.claims.amount < 2_000) { refuse "tooSmall" } }
}
"#;
    assert!(errors(src).is_empty(), "{:?}", errors(src));

    // The message is a key, because a sentence assembled in code is a sentence
    // nobody signed — and this one is read by the person being declined.
    let inline = src.replace(r#"refuse "tooSmall""#, r#"refuse tooSmall"#);
    let e = errors(&inline).join("\n");
    assert!(e.contains("names a key in the text bundle"), "{e}");

    // And it belongs before `execute`: by there the batch is built and the host
    // is about to be offered it.
    let late = src.replace(
        r#"compute { if (checked.claims.amount < 2_000) { refuse "tooSmall" } }"#,
        r#"execute { refuse "tooSmall" }"#,
    );
    let e = errors(&late).join("\n");
    assert!(e.contains("too late to be one"), "{e}");
}

/// The bundle and the code are signed as one package, so they are checked as
/// one: a key nothing translates is a screen that says `missing key` to
/// somebody, and both are knowable before it ships.
#[test]
fn a_key_nothing_translates_is_a_failed_build() {
    use std::collections::BTreeMap;

    let src = r#"
app "example.decline"
version 1
capabilities { }
action Earn { compute { refuse "tooSmall" } }
"#;
    let locales = vec!["th".to_string(), "en".to_string()];

    let empty: valang::TextBundle = BTreeMap::new();
    let (_, d) = valang::analyse_with(src, Some((&empty, &locales)));
    // Two languages promised and nothing translating this one: the words are
    // written in place, which is fine in one language and not in two.
    assert!(
        d.iter().any(|d| d.message.contains("written here as words")),
        "{d:?}"
    );

    let half: valang::TextBundle =
        BTreeMap::from([("tooSmall".into(), BTreeMap::from([("en".to_string(), "Too small".to_string())]))]);
    let (_, d) = valang::analyse_with(src, Some((&half, &locales)));
    assert!(d.iter().any(|d| d.message.contains("has no th")), "{d:?}");

    let full: valang::TextBundle = BTreeMap::from([(
        "tooSmall".into(),
        BTreeMap::from([("en".to_string(), "Too small".into()), ("th".to_string(), "น้อยไป".into())]),
    )]);
    let (_, d) = valang::analyse_with(src, Some((&full, &locales)));
    assert!(d.iter().all(|d| d.severity != valang::Severity::Error), "{d:?}");
}
