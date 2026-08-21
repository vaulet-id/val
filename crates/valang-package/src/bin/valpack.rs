//! `valpack` — build a `.vapp`, or verify one the way a host would.

use std::collections::BTreeMap;
use std::process::ExitCode;

use valang_package::{
    artifact_hash, build, did_for, encode, keygen, read, verify, Manifest, Package, Refusal,
};

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  \
         valpack build      <dir> [-o out.vapp]\n  \
         valpack verify     <file.vapp>            what a wallet checks\n  \
         valpack reproduce  <dir> <file.vapp>      what a wallet cannot: rebuild and compare"
    );
    ExitCode::from(2)
}

fn show(pkg: &Package) {
    println!("{} v{} — admitted", pkg.manifest.app, pkg.manifest.version);
    println!("  the module hashes to what integrity says");
    println!("  the signature is over these bytes");
    println!("  it imports only what this host provides");
    println!("  the report it ships is the report its module produces");
    for (line, values) in &pkg.report {
        println!("    {line:<14} {}", if values.is_empty() { "—".to_string() } else { values.join(", ") });
    }
}

/// `valpack reproduce <dir> <app.vapp>` — build that source and compare the bytes.
///
/// **The check a wallet cannot make.** It has no compiler, which is the whole
/// reason a package carries a module and not a source; so this is where the
/// module is tied back to the source somebody published, by anybody who has
/// both. Once, by whoever cares — not on every phone at every install.
fn reproduce(dir: &str, file: &str) -> ExitCode {
    let Ok(bytes) = std::fs::read(file) else {
        eprintln!("{file}: cannot read");
        return ExitCode::FAILURE;
    };
    let pkg = match read(&bytes) {
        Ok(p) => p,
        Err(r) => {
            eprintln!("{file}: {}", describe(&r));
            return ExitCode::FAILURE;
        }
    };
    let sources = read_sources(dir);
    if sources.is_empty() {
        eprintln!("{dir}: no .val sources");
        return ExitCode::FAILURE;
    }
    let joined = sources.values().cloned().collect::<Vec<_>>().join("\n");
    let (text_bundle, locales) = read_text(dir);
    let bundle = if text_bundle.is_empty() { None } else { Some((&text_bundle, &locales[..])) };

    match valang_wasm::compile::reproduces(&joined, bundle, &registries(), &pkg.module) {
        Ok(()) => {
            println!("{} v{} — reproduced", pkg.manifest.app, pkg.manifest.version);
            println!("  {dir} builds exactly the module {file} carries");
            println!("  signed by      {}", pkg.manifest.publisher);
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{file}: not what {dir} builds — {why}");
            ExitCode::from(2)
        }
    }
}

/// The registries a package is built against. A command line has whatever is on
/// disk beside the language; a wallet has its own.
fn registries() -> valang::capability::Hosts {
    use valang_hosts::{CORE, VAULET};
    let loaded = [CORE, VAULET]
        .into_iter()
        .filter_map(|s| valang::capability::Host::parse(s).ok())
        .collect();
    valang::capability::Hosts::of(loaded)
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

/// The signed text bundle beside the sources: `text.json`.
///
/// Read with the language's own reader, because a second one disagreed with it
/// and built packages whose bundle had three entries called `_comment`,
/// `locales` and `keys`.
fn read_text(dir: &str) -> (BTreeMap<String, BTreeMap<String, String>>, Vec<String>) {
    let path = std::path::Path::new(dir).join("text.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return (BTreeMap::new(), vec!["en".to_string()]);
    };
    match valang::read_bundle(&text) {
        Some((keys, locales)) if !locales.is_empty() => (keys, locales),
        _ => {
            eprintln!("{}: not a text bundle", path.display());
            (BTreeMap::new(), vec!["en".to_string()])
        }
    }
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

    if cmd == "reproduce" {
        let Some(file) = args.get(2) else { return usage() };
        return reproduce(dir, file);
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

    let key = keygen();
    let (text_bundle, locales) = read_text(dir);
    let manifest = Manifest {
        app: program.app.clone().unwrap_or_default(),
        version: program.version.clone().unwrap_or_default(),
        kind: "val".into(),
        // The key this build signs with, written as a name. A command line has
        // no domain and no registry to be found in, so the publisher is the
        // only thing it can prove it is.
        publisher: did_for(&key),
        catalogue: "1".into(),
        locales,
    };

    let built = build(manifest, sources, text_bundle, &registries(), Some(&key));

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
                .unwrap_or_else(|| format!("{}.vapp", pkg.manifest.app));
            if let Err(e) = std::fs::write(&out, &bytes) {
                eprintln!("{out}: {e}");
                return ExitCode::FAILURE;
            }
            println!("{} v{}", pkg.manifest.app, pkg.manifest.version);
            println!("  module         {} bytes", pkg.module.len());
            println!("  artifact hash  {}", &artifact_hash(&pkg)[..24]);
            println!("  signed by      {}", pkg.manifest.publisher);
            println!("  written        {out} ({} bytes)", bytes.len());
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}
