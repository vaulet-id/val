//! `valrun` — run one action and print the execution record.
//!
//! The wallet here is a stub: it mints a credential shaped like the
//! declaration, signed by nobody, and approves every batch. That is a
//! development convenience and it is labelled in the output, because the two
//! things this crate exists to be trusted about are who signed a credential and
//! who approved an effect.

use std::collections::BTreeMap;
use std::process::ExitCode;

use valang_runtime::host::{Context, EffectRequest, Host, Verdict};
use valang_runtime::merkle::hex;
use valang_runtime::value::Value;
use valang_runtime::{encode_record, run_action, Outcome};

struct StubWallet;

impl Host for StubWallet {
    fn context(&self) -> Context {
        Context { time_now: 1_755_426_600_000, random_uuid: "0f2a-c71b".into() }
    }
    fn credential(&self, _ty: &str, _policy: Option<&str>) -> Option<BTreeMap<String, Value>> {
        let mut c = BTreeMap::new();
        c.insert("merchant".into(), Value::Str("Codefin Coffee".into()));
        c.insert("amount".into(), Value::Int(12_500));
        c.insert("market_value".into(), Value::Int(89_900));
        c.insert("cost_basis".into(), Value::Int(64_000));
        c.insert("symbol".into(), Value::Str("PTT".into()));
        c.insert("purchased_at".into(), Value::Int(1_755_335_520_000));
        c.insert("valued_at".into(), Value::Int(1_755_400_000_000));
        c.insert("birthdate".into(), Value::Int(820_454_400_000));
        c.insert("country".into(), Value::Str("TH".into()));
        Some(c)
    }
    fn decide(&self, _effects: &[EffectRequest]) -> Verdict {
        Verdict::Approved
    }
    // A stub, and it says so in the output: a real device signs with a key in
    // secure hardware, and this hashes. Enough to show the record has a shape
    // that is signed over, not enough to be believed by anybody.
    fn sign(&self, bytes: &[u8]) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        Sha256::digest(bytes).to_vec()
    }
    fn device_key(&self) -> Vec<u8> {
        b"stub-device".to_vec()
    }
}

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

    let mut state = BTreeMap::new();
    for f in &program.state {
        // A wallet that already holds a membership, so the interesting path runs.
        if f.ty.optional {
            let mut m = BTreeMap::new();
            m.insert("member_id".into(), Value::Str("M-2891".into()));
            m.insert("points".into(), Value::Int(1_240));
            m.insert("tier".into(), Value::Enum("Tier".into(), "bronze".into()));
            state.insert(f.name.clone(), Value::Map(m));
        } else {
            state.insert(f.name.clone(), Value::Int(1_240));
        }
    }

    let run = run_action(&program, &src, action, &state, &BTreeMap::new(), &StubWallet);
    let r = &run.record;

    println!("{} v{} · {}", r.app, r.version, r.action);
    println!(
        "  outcome        {}",
        match &run.outcome {
            Outcome::Committed => "committed".to_string(),
            Outcome::Refused(w) => format!("refused — {w}"),
            Outcome::Failed(w) => format!("ordinary outcome — {w}"),
            Outcome::Defect(w) => format!("defect — {w}"),
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
    println!("\n  the wallet here is a stub: credentials signed by nobody, every batch approved");
    ExitCode::SUCCESS
}
