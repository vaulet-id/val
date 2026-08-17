//! `valrun` — run one action and print the execution record.
//!
//! The wallet here is a stub: it mints a credential shaped like the
//! declaration, signed by nobody, and approves every batch. That is a
//! development convenience and it is labelled in the output, because the two
//! things this crate exists to be trusted about are who signed a credential and
//! who approved an effect.

use std::collections::BTreeMap;
use std::process::ExitCode;

use valang_runtime::fixture::Fixture;
use valang_runtime::merkle::hex;
use valang_runtime::value::Value;
use valang_runtime::{encode_record, run_action, Outcome};

fn hex_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path, action] = args.as_slice() else {
        eprintln!("usage: valrun <file.val> <Action>");
        return ExitCode::from(2);
    };
    let Ok(src) = std::fs::read_to_string(path) else {
        eprintln!("{path}: cannot read");
        return ExitCode::FAILURE;
    };

    let (program, diagnostics) = valang::analyse(&src);
    let errors: Vec<_> = diagnostics.iter().filter(|d| d.severity == valang::Severity::Error).collect();
    if !errors.is_empty() {
        for d in errors {
            println!("  {d}");
        }
        println!("would not build — the host runs these same checks and would refuse the package");
        return ExitCode::FAILURE;
    }

    // One wallet, shared with the tests and with the playground. Three separate
    // inventions of "what is on this phone" meant three answers and no way to
    // tell which one a bug was about.
    let host = match std::fs::read_to_string("fixtures/wallet.json").map_err(|e| e.to_string()).and_then(|t| Fixture::parse(&t)) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("fixtures/wallet.json: {e}");
            return ExitCode::FAILURE;
        }
    };
    let state = host.state();

    let run = run_action(&program, &src, action, &state, &BTreeMap::new(), &host);
    let r = &run.record;

    println!("{} v{} · {}", r.app, r.version, r.action);
    println!(
        "  outcome        {}",
        match &run.outcome {
            Outcome::Committed => "committed".to_string(),
            Outcome::Refused(w) => format!("refused — {w}"),
            Outcome::Failed(w) => format!("ordinary outcome — {w}"),
            Outcome::Defect(w) => format!("defect — {w}"),
            Outcome::Declined(k) => format!("declined — the app said no, showing \"{k}\""),
        }
    );
    println!("  code hash      {}", &hex(&r.code_hash)[..24]);
    println!("  previous root  {}", &hex(&r.previous_root)[..24]);
    println!("  next root      {}", &hex(&r.next_root)[..24]);
    println!("  effects        {} requested, {} executed", r.effects_requested.len(), r.effects_executed);
    for e in &r.effects_requested {
        println!("    {} {}{}", e.capability, match &e.payload { Value::Credential { ty, claims, .. } => format!("{ty} {}", Value::Map(claims.clone())), other => other.to_string() }, if e.reversible { "" } else { "   (irreversible)" });
    }
    println!("  record         {} bytes, signed {}", encode_record(r).len(), &hex_bytes(&r.signature)[..16]);
    println!("  state leaves");
    for l in &run.leaves {
        println!("    {:<22} {:<30} {}", l.path, l.value.to_string(), &hex(&l.hash)[..8]);
    }
    println!("\n  fixtures/wallet.json — credentials signed by nobody, every batch approved");
    ExitCode::SUCCESS
}
