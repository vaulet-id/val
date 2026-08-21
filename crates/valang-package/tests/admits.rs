//! A package that does not open for everybody.
//!
//! The gate is the one thing on the sheet the host evaluates rather than
//! measures, so these tests are about what keeps a stated gate honest: the
//! module has to carry the `check` import that names it, and the sentence the
//! person reads when the door does not open has to exist in the bundle.

use std::collections::BTreeMap;

use valang_package::*;
use serde_json;
use valang_runtime::value::Value;
use valang_runtime::host::{Context, EffectRequest, Host, Verdict};

const STAFF: &str = include_str!("../../../examples/staff.val");

fn registries() -> valang::capability::Hosts {
    valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")])
}

struct Wallet;

impl HostPolicy for Wallet {
    fn owns_key(&self, _publisher: &str, _key: &[u8]) -> bool {
        true
    }
}

fn manifest() -> Manifest {
    Manifest {
        app: "th.co.acme.staff".into(),
        version: "1".into(),
        kind: "val".into(),
        publisher: "did:web:acme.co.th".into(),
        catalogue: "1".into(),
        locales: vec!["th".into(), "en".into()],
    }
}

/// Read out of `examples/staff-text.json` rather than written again here: the
/// sentence a person is shown at a closed door is part of the package, and a
/// second copy of it in a test is a second thing to keep in step.
fn text() -> BTreeMap<String, BTreeMap<String, String>> {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/staff-text.json")).expect("parses");
    json["keys"]
        .as_object()
        .expect("keys")
        .iter()
        .map(|(key, per_locale)| {
            (
                key.clone(),
                per_locale
                    .as_object()
                    .expect("locales")
                    .iter()
                    .map(|(l, s)| (l.clone(), s.as_str().unwrap_or_default().to_string()))
                    .collect(),
            )
        })
        .collect()
}

fn published(text: BTreeMap<String, BTreeMap<String, String>>) -> Package {
    let key = keygen();
    let sources = BTreeMap::from([("staff.val".to_string(), STAFF.to_string())]);
    build(manifest(), sources, text, &registries(), Some(&key)).expect("builds")
}

/// A wallet holding whatever it is told to hold.
struct Holding(Option<&'static str>);

impl Host for Holding {
    fn context(&self) -> Context {
        Context { time_now: 0, random_uuid: String::new() }
    }
    fn credential(&self, ty: &str, _policy: Option<&str>) -> Option<BTreeMap<String, Value>> {
        if self.0 == Some(ty) {
            Some(BTreeMap::from([("employee_id".to_string(), Value::Str("e-1".into()))]))
        } else {
            None
        }
    }
    fn decide(&self, _effects: &[EffectRequest]) -> Verdict {
        Verdict::Approved
    }
    // A gate is answered before anything runs, so nothing here signs.
    fn sign(&self, _bytes: &[u8]) -> Vec<u8> {
        Vec::new()
    }
    fn device_key(&self) -> Vec<u8> {
        Vec::new()
    }
}

/// The gate is on the sheet, and the module carries the import that names it —
/// which is what makes it a claim about this application rather than a sentence
/// its publisher typed.
#[test]
fn the_module_looks_at_the_credential_the_gate_names() {
    let pkg = published(text());
    let installed = install_with(&pkg, &Wallet).expect("this wallet admits it");
    let code = installed.code.expect("it carries a module");

    assert_eq!(code.about.admits.len(), 1);
    assert_eq!(code.about.admits[0].credential, "EmployeeBadge");
    assert_eq!(code.about.admits[0].policy, "EmployedByAcme");
    assert_eq!(code.about.admits[0].phrase, "notStaff");
    assert_eq!(code.about.admits[0].line(), "EmployeeBadge under EmployedByAcme");

    let wants = valang_wasm::wants_of(&code.module).expect("the module reads");
    assert!(
        wants.checks.contains("EmployeeBadge under EmployedByAcme"),
        "the gate has to be in the import section: {:?}",
        wants.checks
    );
    // And it is a check, never a read: this application is told the door
    // opened, not who walked through it.
    assert!(wants.reads.is_empty(), "{:?}", wants.reads);
}

/// Where a module's sections begin and end. Enough of the format to move one
/// section from one module into another, and no more.
fn sections(bytes: &[u8]) -> Vec<(u8, usize, usize)> {
    let mut out = Vec::new();
    let mut i = 8; // magic and version
    while i < bytes.len() {
        let start = i;
        let id = bytes[i];
        i += 1;
        let mut len = 0usize;
        let mut shift = 0;
        loop {
            let b = bytes[i];
            i += 1;
            len |= ((b & 0x7f) as usize) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        out.push((id, start, i + len));
        i += len;
    }
    out
}

/// The last custom section — which is `val.about`, because the compiler emits
/// it last. Taken out of one module and put into another rather than encoded
/// here: a fixture written by the same reasoning as the code it tests is two
/// things agreeing with each other.
fn about_section(bytes: &[u8]) -> (usize, usize) {
    let custom: Vec<_> = sections(bytes).into_iter().filter(|(id, _, _)| *id == 0).collect();
    let (_, from, to) = *custom.last().expect("a module carries its about section");
    (from, to)
}

/// The sheet says the door is there and the module has no door. Nothing about
/// the bytes is wrong — the signature is good and the report matches — so this
/// is the check that has to catch it.
#[test]
fn a_gate_the_module_does_not_back_is_refused() {
    // A second application, identical except for who it opens for. Its
    // `about` section is what gets spliced in: both halves are the compiler's
    // own output, so what is being tested is the wallet noticing they disagree
    // rather than this test's idea of how metadata is encoded.
    let other = STAFF
        .replace("EmployeeBadge with EmployedByAcme", "Passport with EmployedByAcme")
        .replace("credential.check(EmployeeBadge)", "credential.check(Passport)")
        .replace("credential EmployeeBadge", "credential Passport")
        .replace("EmployedByAcme(badge: EmployeeBadge)", "EmployedByAcme(badge: Passport)");
    let key = keygen();
    let elsewhere = build(
        manifest(),
        BTreeMap::from([("staff.val".to_string(), other)]),
        text(),
        &registries(),
        Some(&key),
    )
    .expect("the second one builds too");

    let mut pkg = published(text());
    let (from, to) = about_section(&pkg.module);
    let (their_from, their_to) = about_section(&elsewhere.module);
    let mut spliced = pkg.module[..from].to_vec();
    spliced.extend_from_slice(&elsewhere.module[their_from..their_to]);
    let _ = to;
    pkg.module = spliced;
    // Integrity is over the module, so a publisher editing one re-hashes and
    // re-signs. Both are done here, because a check that only caught the lazy
    // version of this would catch nothing.
    pkg.integrity = hex_of(&pkg.module);
    sign(&mut pkg, &key);

    match install_with(&pkg, &Wallet) {
        Err(Refusal::Refused { by }) => {
            assert!(by.contains("never looks at one"), "{by}")
        }
        Ok(_) => panic!("a door that is not there was admitted"),
        Err(e) => panic!("refused, but not for the gate: {e:?}"),
    }
}

/// A door that closes without saying why leaves the person with a fault report.
#[test]
fn a_gate_whose_sentence_is_in_no_language_is_refused() {
    let mut without = text();
    without.remove("notStaff");
    // `build` checks the bundle itself, so the package cannot be built without
    // the key. What is tested here is the wallet's own check: it does not
    // depend on the publisher having run one.
    let mut stripped = published(text());
    stripped.text_bundle = without;
    let key = keygen();
    sign(&mut stripped, &key);

    match install_with(&stripped, &Wallet) {
        Err(Refusal::Refused { by }) => assert!(by.contains("in no language"), "{by}"),
        Ok(_) => panic!("a silent door was admitted"),
        Err(e) => panic!("refused, but not for the sentence: {e:?}"),
    }
}

/// The gate itself: the host answers it, and the answer is the person's wallet.
#[test]
fn the_door_opens_for_whoever_holds_one_and_for_nobody_else() {
    let pkg = published(text());
    let installed = install_with(&pkg, &Wallet).expect("admitted");
    let about = installed.code.expect("module").about;

    assert!(valang_runtime::admission(&about, &Holding(Some("EmployeeBadge"))).is_none());

    let closed = valang_runtime::admission(&about, &Holding(None)).expect("this door stays shut");
    assert_eq!(closed.phrase, "notStaff");
}

/// An application with no gate opens for everybody, which is almost every
/// application and has to stay the cheap case.
#[test]
fn an_application_with_no_gate_opens() {
    let about = valang_runtime::About { admits: Vec::new(), ..Default::default() };
    assert!(valang_runtime::admission(&about, &Holding(None)).is_none());
}
