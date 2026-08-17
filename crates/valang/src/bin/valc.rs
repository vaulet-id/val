//! `valc` — parse, check, and print the capability report.
//!
//! The same passes the host runs over a package it received. A publisher's
//! build passing proves nothing, so this is deliberately the whole of it and
//! not a convenience wrapper around something bigger.

use std::process::ExitCode;

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
        let (program, diagnostics) = valang::analyse(&src);

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
