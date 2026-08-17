//! `valpack` — build a `.va`, or verify one the way a host would.

use std::collections::BTreeMap;
use std::process::ExitCode;

use valang_package::{artifact_hash, build, encode, keygen, read, verify, Manifest, Package, Refusal};

fn usage() -> ExitCode {
    eprintln!("usage:\n  valpack build  <dir> [-o out.va]\n  valpack verify <file.va>");
    ExitCode::from(2)
}

fn show(pkg: &Package) {
    println!("{} v{} — admitted", pkg.manifest.app, pkg.manifest.version);
    println!("  every source hashes to what integrity says");
    println!("  the signature is over these bytes");
    println!("  it compiles, checked here and not taken on trust");
    println!("  the report it ships is the report its code produces");
    for (line, values) in &pkg.report {
        println!("    {line:<14} {}", if values.is_empty() { "—".to_string() } else { values.join(", ") });
    }
}

fn read_sources(dir: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().is_some_and(|x| x == "val") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                out.insert(path.file_name().unwrap().to_string_lossy().to_string(), text);
            }
        }
    }
    out
}

fn describe(r: &Refusal) -> String {
    match r {
        Refusal::Modified(p) => format!("`{p}` does not hash to what integrity says it does — it was changed after it was signed"),
        Refusal::Unsigned(w) => format!("unsigned: {w}"),
        Refusal::WouldNotBuild(errors) => format!("would not build:\n    {}", errors.join("\n    ")),
        Refusal::ReportMismatch { line, shipped, derived } => format!(
            "the report understates this app. `{line}` ships as {shipped:?} and the code says {derived:?}"
        ),
        Refusal::Malformed(w) => format!("malformed: {w}"),
        Refusal::Refused { by } => by.clone(),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(String::as_str) else { return usage() };
    let Some(dir) = args.get(1) else { return usage() };

    // Verifying reads the artifact. Rebuilding from a directory and checking
    // that would be checking our own work, which is the one thing verification
    // must not be.
    if cmd == "verify" {
        let Ok(bytes) = std::fs::read(dir) else {
            eprintln!("{dir}: cannot read");
            return ExitCode::FAILURE;
        };
        return match read(&bytes).and_then(|p| verify(&p).map(|()| p)) {
            Ok(pkg) => {
                show(&pkg);
                ExitCode::SUCCESS
            }
            Err(r) => {
                println!("refused — {}", describe(&r));
                ExitCode::FAILURE
            }
        };
    }

    let sources = read_sources(dir);
    if sources.is_empty() {
        eprintln!("{dir}: no .val sources");
        return ExitCode::FAILURE;
    }

    // One package per app, so the manifest's identity comes from the sources
    // rather than from a flag somebody could get wrong.
    let joined = sources.values().cloned().collect::<Vec<_>>().join("\n");
    let (program, _) = valang::analyse(&joined);

    let manifest = Manifest {
        app: program.app.clone().unwrap_or_default(),
        version: program.version.clone().unwrap_or_default(),
        kind: "val".into(),
        publisher: "did:web:codefin.io".into(),
        catalogue: "1".into(),
        locales: vec!["th".into(), "en".into()],
    };

    let key = keygen();
    let built = build(manifest, sources, BTreeMap::new(), Some(&key));

    let pkg: Package = match built {
        Ok(p) => p,
        Err(r) => {
            println!("refused — {}", describe(&r));
            return ExitCode::FAILURE;
        }
    };

    match cmd {
        "build" => {
            let bytes = encode(&pkg);
            let out = args
                .iter()
                .position(|a| a == "-o")
                .and_then(|i| args.get(i + 1).cloned())
                .unwrap_or_else(|| format!("{}.va", pkg.manifest.app));
            if let Err(e) = std::fs::write(&out, &bytes) {
                eprintln!("{out}: {e}");
                return ExitCode::FAILURE;
            }
            println!("{} v{}", pkg.manifest.app, pkg.manifest.version);
            println!("  sources        {}", pkg.sources.len());
            println!("  artifact hash  {}", &artifact_hash(&pkg)[..24]);
            println!("  signed by      {}", pkg.manifest.publisher);
            println!("  written        {out} ({} bytes)", bytes.len());
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}
