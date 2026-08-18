//! Every language the runner accepts has a handler somebody has run.
//!
//! The end-to-end check — one record, four languages, one decision — is
//! `runner/try.sh`, because it needs the toolchains installed and a running
//! service. What this guards is the gap that opens quietly: a language added to
//! `lang.rs` that nothing was ever written in.

const LANG_RS: &str = include_str!("../src/lang.rs");

#[test]
fn every_language_has_an_sdk_and_an_entry_point() {
    let declared = LANG_RS.matches("ext: \"").count();
    assert_eq!(declared, 4, "four languages: TypeScript, Python, Go, Rust");

    for name in ["val.mjs", "val.py", "val.go", "val.rs"] {
        assert!(LANG_RS.contains(name), "{name} is not written beside a handler");
    }
    for entry in ["entry.mjs", "entry.py", "main.go", "main.rs"] {
        assert!(LANG_RS.contains(entry), "{entry} is not generated");
    }
}

/// Go and Rust need a manifest before their toolchain will look at a file. A
/// language whose manifest is missing fails at run time with a message about
/// packages rather than about the missing file.
#[test]
fn the_compiled_languages_carry_their_manifest() {
    assert!(LANG_RS.contains("go.mod"));
    assert!(LANG_RS.contains("Cargo.toml"));
}
