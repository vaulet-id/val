//! The import section, read as a capability report.
//!
//! The modules here are built by hand rather than compiled, on purpose: this is
//! the check a wallet runs on bytes somebody sent it, and bytes somebody sent it
//! are not bytes this compiler wrote.

use valang_wasm::{wants_of, Cap};
use wasm_encoder::{EntityType, ImportSection, Module, TypeSection, ValType};

/// A module that imports exactly these names from exactly these namespaces, and
/// does nothing at all. What a host is handed is a list of names; whether the
/// module around them is useful is not what this check is about.
fn module_importing(entries: &[(&str, &str, usize)]) -> Vec<u8> {
    let mut types = TypeSection::new();
    for arity in 0..=3usize {
        types.ty().function(vec![ValType::I32; arity], vec![ValType::I32]);
    }
    let mut imports = ImportSection::new();
    for (ns, name, arity) in entries {
        imports.import(ns, name, EntityType::Function(*arity as u32));
    }
    let mut m = Module::new();
    m.section(&types);
    m.section(&imports);
    m.finish()
}

#[test]
fn a_capability_survives_being_written_down_and_read_back() {
    let all = [
        Cap::Read("PurchaseReceipt.amount".into()),
        Cap::Disclose("NationalId.country".into()),
        Cap::Prove("age >= 20".into()),
        Cap::Issue("LoyaltyMember".into()),
        Cap::Query("broker.co.th".into()),
        Cap::Pay("merchant".into()),
        Cap::Write("member.points".into()),
    ];
    for cap in all {
        assert_eq!(Cap::parse(&cap.name()), Some(cap.clone()), "{}", cap.name());
    }
}

/// The parameter is inside the name, and a name is one string — so a claim path
/// with dots in it comes back whole. `read:A.b.c` is one capability, not three.
#[test]
fn a_path_with_dots_in_it_is_one_capability() {
    assert_eq!(Cap::parse("read:A.b.c"), Some(Cap::Read("A.b.c".into())));
}

#[test]
fn the_report_is_the_import_section() {
    let bytes = module_importing(&[
        ("val", "add", 2),
        ("cap", "read:PurchaseReceipt.amount", 0),
        ("cap", "read:PurchaseReceipt.purchased_at", 0),
        ("cap", "write:member.points", 1),
        ("cap", "issue:LoyaltyMember", 1),
    ]);

    let wants = wants_of(&bytes).expect("a module this host can describe");
    assert_eq!(
        wants.reads.iter().cloned().collect::<Vec<_>>(),
        vec!["PurchaseReceipt.amount", "PurchaseReceipt.purchased_at"]
    );
    assert_eq!(wants.writes.iter().cloned().collect::<Vec<_>>(), vec!["member.points"]);
    assert_eq!(wants.issues.iter().cloned().collect::<Vec<_>>(), vec!["LoyaltyMember"]);
    assert!(wants.discloses.is_empty() && wants.proves.is_empty() && wants.payments.is_empty());
}

/// Arithmetic says nothing about anybody, so it is a different namespace and it
/// is not in the report. Every module imports all of it.
#[test]
fn arithmetic_is_not_a_capability() {
    let bytes = module_importing(&[("val", "add", 2), ("val", "truthy", 1), ("val", "at", 2)]);
    assert_eq!(wants_of(&bytes), Ok(Default::default()));
}

/// The refusal that makes the rest of it mean anything. If a module could
/// import something this host does not know, the list would stop being the
/// whole of what the module can do — and the sheet the person agreed to would
/// be describing a subset.
#[test]
fn a_module_that_imports_anything_else_is_refused() {
    let bytes = module_importing(&[("wasi_snapshot_preview1", "fd_write", 3)]);
    assert!(wants_of(&bytes).is_err(), "a module reaching outside the ABI was accepted");
}

#[test]
fn a_capability_this_host_does_not_know_is_refused() {
    let bytes = module_importing(&[("cap", "exfiltrate:everything", 1)]);
    assert!(wants_of(&bytes).is_err());
}

/// Nothing about the report is a promise the module makes: it is what the
/// module *can* reach. Two calls to one capability are one import, so the list
/// cannot be padded, and one call it never makes is still on the list.
#[test]
fn the_same_capability_twice_is_one_line() {
    let bytes = module_importing(&[("cap", "read:R.a", 0), ("cap", "read:R.a", 0)]);
    assert_eq!(wants_of(&bytes).unwrap().reads.len(), 1);
}
