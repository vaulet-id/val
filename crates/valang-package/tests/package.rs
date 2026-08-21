//! What a host checks before admitting an application, and each thing it would
//! otherwise be trusting the publisher about.

use std::collections::BTreeMap;

use valang_package::*;

/// The signing key type, so a fixture can name the publisher it signs as.
use ed25519_dalek::SigningKey;

const LOYALTY: &str = include_str!("../../../examples/loyalty.val");

/// What this host draws with, which is what a package is compiled against.
fn registries() -> valang::capability::Hosts {
    valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")])
}

/// The publisher is the key, which is what `did:key` means. A fixture naming a
/// publisher it cannot prove would be a fixture testing the wrong thing.
fn manifest(key: &SigningKey) -> Manifest {
    Manifest {
        app: "th.co.codefin.loyalty".into(),
        version: "1".into(),
        kind: "val".into(),
        publisher: did_for(key),
        catalogue: "1".into(),
        locales: vec!["th".into(), "en".into()],
    }
}

fn sources() -> BTreeMap<String, String> {
    BTreeMap::from([("loyalty.val".to_string(), LOYALTY.to_string())])
}

/// Every key `loyalty.val` names, in every locale the manifest promises. The
/// compiler checks the bundle against the code because they are signed as one
/// package — which this fixture found out by being wrong.
fn text() -> BTreeMap<String, BTreeMap<String, String>> {
    let entry = |th: &str, en: &str| {
        BTreeMap::from([("th".to_string(), th.to_string()), ("en".to_string(), en.to_string())])
    };
    BTreeMap::from([
        ("balance".to_string(), entry("แต้ม {points}", "{points} points")),
        (
            "tooSmallToEarn".to_string(),
            entry("ยอดต่ำกว่า 20 บาท ยังไม่ได้แต้ม", "Purchases under ฿20 do not earn points"),
        ),
    ])
}

#[test]
fn a_signed_package_is_admitted() {
    let key = keygen();
    let pkg = build(manifest(&key), sources(), text(), &registries(), Some(&key)).expect("builds");
    verify(&pkg).expect("admitted");
}

#[test]
fn an_unsigned_package_is_not() {
    let key = keygen();
    let pkg = build(manifest(&key), sources(), text(), &registries(), None).expect("builds");
    assert!(matches!(verify(&pkg), Err(Refusal::Unsigned(_))));
}

#[test]
fn a_source_changed_after_signing_is_caught() {
    let key = keygen();
    let mut pkg = build(manifest(&key), sources(), text(), &registries(), Some(&key)).expect("builds");
    // The one attack the integrity table exists for: the reviewed program is
    // not the program that runs.
    let last = pkg.module.len() - 1;
    pkg.module[last] ^= 0xff;
    assert!(matches!(verify(&pkg), Err(Refusal::Modified(_))));
}

#[test]
fn a_report_that_understates_the_app_is_caught() {
    let key = keygen();
    let mut pkg = build(manifest(&key), sources(), text(), &registries(), Some(&key)).expect("builds");
    // A publisher shipping "issues: nothing" for an app that issues a
    // credential is the one lie a package could otherwise tell — and they sign
    // it themselves, so the signature is no help. Only recomputing is.
    pkg.report.insert("issues".into(), Vec::new());
    sign(&mut pkg, &key);
    match verify(&pkg) {
        Err(Refusal::ReportMismatch { line, derived, .. }) => {
            assert_eq!(line, "issues");
            assert_eq!(derived, vec!["LoyaltyMember".to_string()]);
        }
        other => panic!("should have caught the understatement, got {other:?}"),
    }
}

#[test]
fn a_missing_translation_is_a_failed_build() {
    let key = keygen();
    let mut text = text();
    text.get_mut("balance").unwrap().remove("th");
    let pkg = build(manifest(&key), sources(), text, &registries(), Some(&key)).expect("builds");
    match verify(&pkg) {
        Err(Refusal::Malformed(m)) => assert!(m.contains("no th"), "{m}"),
        other => panic!("a market's language missing should fail, got {other:?}"),
    }
}

#[test]
fn a_program_that_does_not_compile_is_never_packaged() {
    let key = keygen();
    let broken = BTreeMap::from([("bad.val".to_string(), "app \"x\"\nversion 1\ncapabilities { payment.request }\n".to_string())]);
    match build(manifest(&key), broken, text(), &registries(), Some(&key)) {
        Err(Refusal::WouldNotBuild(errors)) => {
            assert!(errors.iter().any(|e| e.contains("never used")), "{errors:?}");
        }
        other => panic!("should not have built, got {:?}", other.map(|p| p.manifest)),
    }
}

#[test]
fn a_package_written_out_is_the_package_read_back() {
    let key = keygen();
    let built = build(manifest(&key), sources(), text(), &registries(), Some(&key)).unwrap();
    let bytes = encode(&built);
    let back = read(&bytes).expect("reads");
    assert_eq!(back, built, "what was written is what comes back");
    verify(&back).expect("and it is still admitted");
}

#[test]
fn a_truncated_package_is_refused_rather_than_guessed_at() {
    let key = keygen();
    let bytes = encode(&build(manifest(&key), sources(), text(), &registries(), Some(&key)).unwrap());
    assert!(read(&bytes[..bytes.len() / 2]).is_err());
}

#[test]
fn the_same_package_is_the_same_bytes() {
    let key = keygen();
    let a = build(manifest(&key), sources(), text(), &registries(), Some(&key)).unwrap();
    let b = build(manifest(&key), sources(), text(), &registries(), Some(&key)).unwrap();
    // Reproducible, which is what lets two people check they are holding the
    // same application without asking each other.
    assert_eq!(encode(&a), encode(&b));
    assert_eq!(artifact_hash(&a), artifact_hash(&b));
}

/// The first host's ceiling, written as a test rather than shipped in the crate:
/// the package format carries `kind` and takes no view about it, and a language
/// repository holding one host's policy is the leak §1 is about.
struct Vaulet;

impl HostPolicy for Vaulet {

    fn allows(&self, kind: &str, capability: &str) -> bool {
        if kind != "webview" {
            return true;
        }
        // Capabilities follow verifiability, not preference. A webview runs code
        // the host did not compile and draws screens it did not draw, so the
        // host cannot state what ran — and every capability below depends on
        // saying it.
        !matches!(capability, "credential.issue" | "payment.request")
    }
    fn supports_catalogue(&self, version: &str) -> bool {
        version == "1"
    }
}

/// A webview carries no VAL, so its report is a *declaration* — nobody
/// compiled anything to derive it from. That is not a consequence of the lower
/// ceiling, it is the reason for it: what a person cannot have checked, a host
/// can only enforce.
#[test]
fn a_webview_may_not_issue_a_credential() {
    let key = keygen();
    let mut m = manifest(&key);
    m.kind = "webview".into();

    let mut pkg = build(m, BTreeMap::new(), text(), &registries(), Some(&key)).unwrap();
    pkg.report.insert("issues".into(), vec!["Card".into()]);
    sign(&mut pkg, &key);

    match verify_with(&pkg, &Vaulet) {
        Err(Refusal::Refused { by }) => {
            assert!(by.contains("may not `credential.issue`"), "{by}");
            assert!(by.contains("follow verifiability rather than preference"), "{by}");
        }
        other => panic!("a webview that issues credentials should not be admitted, got {other:?}"),
    }

    // The same claim from a `val` package is admitted, because there it was
    // derived from code this host compiled itself.
    let ok = build(manifest(&key), sources(), text(), &registries(), Some(&key)).unwrap();
    verify_with(&ok, &Vaulet).expect("a val app that issues is admitted");
}

#[test]
fn a_webview_carrying_a_module_is_refused() {
    let key = keygen();
    let mut m = manifest(&key);
    m.kind = "webview".into();
    let pkg = build(m, sources(), text(), &registries(), Some(&key)).unwrap();
    match verify_with(&pkg, &Vaulet) {
        Err(Refusal::Refused { by }) => assert!(by.contains("carries no module"), "{by}"),
        other => panic!("got {other:?}"),
    }
}

#[test]
fn a_catalogue_this_host_cannot_render_is_refused() {
    let key = keygen();
    let mut m = manifest(&key);
    m.catalogue = "3".into();
    let pkg = build(m, sources(), text(), &registries(), Some(&key)).unwrap();
    match verify_with(&pkg, &Vaulet) {
        Err(Refusal::Refused { by }) => assert!(by.contains("does not render catalogue 3"), "{by}"),
        other => panic!("a host should refuse a catalogue it cannot render, got {other:?}"),
    }
    // And the language's own `verify` still says yes: this is host policy, and
    // the crate holds none.
    verify(&pkg).expect("the package itself is well formed");
}

/// A manifest is what a host reads before it reads the code. If the two
/// disagree about which application this is, the one a person sees named on the
/// consent sheet is not the one that runs.
#[test]
fn a_manifest_that_names_another_application_is_refused() {
    let key = keygen();
    let mut m = manifest(&key);
    m.app = "th.co.somebody.else".into();

    let built = build(m, sources(), text(), &registries(), None);
    match built {
        Err(_) => {}
        Ok(p) => panic!(
            "a package was built whose manifest says `{}` and whose code says `th.co.codefin.loyalty`",
            p.manifest.app
        ),
    }
}

/// And the same about the version: a manifest saying 2 over code saying 1 is a
/// package whose records name a version nobody published.
#[test]
fn a_manifest_that_names_another_version_is_refused() {
    let key = keygen();
    let mut m = manifest(&key);
    m.version = "9".into();
    assert!(
        build(m, sources(), text(), &registries(), None).is_err(),
        "a package was built whose manifest and code disagree about its version"
    );
}

/// A host admits a package by compiling it — against its own registry, which is
/// the only copy that matters. Compiled against none, a package drawing
/// something this wallet does not ship is admitted and fails on the phone.
#[test]
fn a_package_is_compiled_against_the_registry_that_will_run_it() {
    let src = r#"
app "th.co.codefin.loyalty"
version 1

capabilities {
}

state {
  n: int default 0
}

@main
screen Home {
  column {
    tabs {
      section("x")
    }
  }
}
"#;
    let sources = BTreeMap::from([("only.val".to_string(), src.to_string())]);
    let key = keygen();
    let mut m = manifest(&key);
    m.locales = vec!["en".into()];

    let built = build(m, sources, BTreeMap::new(), &registries(), None);
    match built {
        Err(_) => {}
        Ok(_) => panic!(
            "a package drawing `tabs`, which no host ships, was built — so nothing checked it against a catalogue"
        ),
    }
}
