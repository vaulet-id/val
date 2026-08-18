//! `valc` — parse, check, and print the capability report.
//!
//! The same passes the host runs over a package it received. A publisher's
//! build passing proves nothing, so this is deliberately the whole of it and
//! not a convenience wrapper around something bigger.

use std::process::ExitCode;

/// The text bundle beside the sources. Checking code without it is checking
/// half a package, and it is the half that says what a person reads.
fn parse_bundle(text: &str) -> Option<(valang::TextBundle, Vec<String>)> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let locales = json["locales"]
        .as_array()?
        .iter()
        .filter_map(|l| l.as_str().map(str::to_string))
        .collect();
    let keys = json["keys"]
        .as_object()?
        .iter()
        .map(|(key, per_locale)| {
            let inner = per_locale
                .as_object()
                .map(|m| {
                    m.iter()
                        .filter_map(|(l, t)| t.as_str().map(|t| (l.clone(), t.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            (key.clone(), inner)
        })
        .collect();
    Some((keys, locales))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: valc <file.val>…");
        return ExitCode::from(2);
    }

    let mut failed = false;
    for path in &args {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{path}: cannot read");
            failed = true;
            continue;
        };
        // The bundle beside the sources, if there is one. Checking the code
        // without it is checking half a package — and it is the half that says
        // what a person reads.
        let bundle = std::path::Path::new(path)
            .parent()
            .map(|dir| dir.join("text.json"))
            .filter(|p| p.exists())
            .and_then(|p| std::fs::read_to_string(p).ok());
        let bundle = bundle.as_deref().and_then(parse_bundle);

        let (program, diagnostics) = match &bundle {
            Some((keys, locales)) => {
                valang::analyse_against(&src, Some((keys, locales)), &catalogues())
            }
            None => valang::analyse_against(&src, None, &catalogues()),
        };

        println!("── {path}");
        for d in &diagnostics {
            println!("  {d}");
        }
        let errors = diagnostics.iter().filter(|d| d.severity == valang::Severity::Error).count();
        if errors > 0 {
            failed = true;
            println!("  {errors} error(s) — would not build");
            continue;
        }
        print!("{}", valang::report::report(&program));
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// The catalogues to check against.
///
/// A real host publishes its own and hands it to the compiler; what a command
/// line has is whatever is on disk beside the language. The core profile is
/// built in because a package that names nothing else is checked against it.
fn catalogues() -> valang::catalogue::Catalogues {
    const CORE: &str = include_str!("../../../../catalogues/core.json");
    const VAULET: &str = include_str!("../../../../catalogues/vaulet.json");

    let mut loaded = Vec::new();
    for source in [CORE, VAULET] {
        match valang::catalogue::Catalogue::parse(source) {
            Ok(c) => loaded.push(c),
            Err(e) => eprintln!("a catalogue did not parse: {e}"),
        }
    }
    valang::catalogue::Catalogues::of(loaded)
}
