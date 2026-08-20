//! Diagnostics, compared exactly.
//!
//! A test that asks whether a message *contains* a phrase passes when the rest
//! of the message has gone to pieces, and the message is part of the language:
//! a rule that produces only "type error" has not been taught to anybody.
//!
//! Each `ui/*.val` is a program, and `ui/*.expected` beside it is what the
//! compiler says about it, rendered — the message, the line, and the underline.
//! To adopt a change, read it and then:
//!
//! ```text
//! VAL_BLESS=1 cargo test -p valang --test ui
//! ```

use std::path::{Path, PathBuf};

fn hosts() -> valang::capability::Hosts {
    valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")])
}

fn rendered(src: &str) -> String {
    let (_, d) = valang::analyse_fully(src, None, &hosts());
    if d.is_empty() {
        return "(nothing to say)\n".to_string();
    }
    let mut out = String::new();
    for x in &d {
        out.push_str(&x.render(src));
        out.push_str("\n\n");
    }
    out
}

fn cases() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ui");
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .map(|f| f.path())
        .filter(|p| p.extension().is_some_and(|e| e == "val"))
        .collect();
    out.sort();
    out
}

#[test]
fn every_case_says_what_it_said_before() {
    let bless = std::env::var("VAL_BLESS").is_ok();
    let mut wrong = Vec::new();

    for case in cases() {
        let src = std::fs::read_to_string(&case).expect("readable");
        let want_path = case.with_extension("expected");
        let got = rendered(&src);

        if bless {
            std::fs::write(&want_path, &got).expect("writable");
            continue;
        }

        let want = std::fs::read_to_string(&want_path).unwrap_or_default();
        if want != got {
            let name = case.file_name().unwrap().to_string_lossy().to_string();
            wrong.push(format!("── {name}\nexpected:\n{want}\ngot:\n{got}"));
        }
    }

    assert!(
        wrong.is_empty(),
        "{}\n{} case(s) say something else now. Read the change, then `VAL_BLESS=1 cargo test -p valang --test ui`",
        wrong.join("\n"),
        wrong.len()
    );
}

/// A case with no expectation beside it is a case nobody has read.
#[test]
fn every_case_has_an_expectation() {
    for case in cases() {
        let want = case.with_extension("expected");
        assert!(
            want.exists(),
            "{} has no .expected beside it",
            case.file_name().unwrap().to_string_lossy()
        );
    }
}
