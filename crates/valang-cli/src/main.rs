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
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    // `--packages <dir>`, where every subdirectory is a package that names and
    // versions itself. Where a package comes from is not the language's
    // question — a registry, a directory, an editor's open projects are all
    // answers — so the command line answers it here and hands the result over.
    // `--surface <file>`, where a package records what it exports.
    //
    // Nothing else can enforce the rule that changing an exported component is
    // a new version rather than an edit: the packages that depend on it are not
    // here, and their authors are not in the room. What is here is what this
    // package published last time, if somebody kept it — so keeping it is the
    // mechanism.
    let mut surface_path = None;
    while let Some(i) = args.iter().position(|a| a == "--surface") {
        args.remove(i);
        if i < args.len() {
            surface_path = Some(args.remove(i));
        } else {
            eprintln!("--surface takes a file");
            return ExitCode::from(2);
        }
    }

    let mut package_dirs = Vec::new();
    while let Some(i) = args.iter().position(|a| a == "--packages") {
        args.remove(i);
        if i < args.len() {
            package_dirs.push(args.remove(i));
        } else {
            eprintln!("--packages takes a directory");
            return ExitCode::from(2);
        }
    }

    // `--format` writes each file back out in one shape. A formatter is worth
    // having on its own, and it is also how the parser is tested against
    // itself: printing what was parsed and parsing that again has to give the
    // same text.
    let format = args.iter().any(|a| a == "--format");
    args.retain(|a| a != "--format");

    if args.is_empty() {
        eprintln!("usage: valc [--format] [--packages <dir>] [--surface <file>] <file.val>…");
        return ExitCode::from(2);
    }

    if format {
        let mut failed = false;
        for path in &args {
            let Ok(src) = std::fs::read_to_string(path) else {
                eprintln!("{path}: cannot read");
                failed = true;
                continue;
            };
            let (program, diagnostics) = valang::parse::parse(&src);
            if diagnostics.iter().any(|d| d.severity == valang::Severity::Error) {
                // A file that does not parse is a file whose shape nobody
                // knows. Rewriting it would be the formatter guessing.
                eprintln!("{path}: does not parse, so it is left alone");
                for d in &diagnostics {
                    eprintln!("  {d}");
                }
                failed = true;
                continue;
            }
            let out = valang::print::print(&program);
            if out != src {
                if let Err(e) = std::fs::write(path, &out) {
                    eprintln!("{path}: {e}");
                    failed = true;
                } else {
                    println!("{path}");
                }
            }
        }
        return if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS };
    }

    // A package is several files sharing one scope: `wallet.val` presses an
    // action `loyalty.val` declares, and either alone is half a program that
    // fails for the wrong reason. They are analysed together, the way a host
    // analyses the package it received — checking them one at a time reported
    // that a screen's action did not exist, which was true only of that file.
    let mut sources = Vec::new();
    let mut failed = false;
    for path in &args {
        match std::fs::read_to_string(path) {
            Ok(src) => sources.push((path.clone(), src)),
            Err(_) => {
                eprintln!("{path}: cannot read");
                failed = true;
            }
        }
    }
    if sources.is_empty() {
        return ExitCode::from(2);
    }

    {
        let path = &sources[0].0;
        let src = sources.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join("\n");
        // The bundle beside the sources, if there is one. Checking the code
        // without it is checking half a package — and it is the half that says
        // what a person reads.
        let bundle = std::path::Path::new(path)
            .parent()
            .map(|dir| dir.join("text.json"))
            .filter(|p| p.exists())
            .and_then(|p| std::fs::read_to_string(p).ok());
        let bundle = bundle.as_deref().and_then(parse_bundle);

        let packages = packages(&package_dirs);
        let (program, diagnostics) = match &bundle {
            Some((keys, locales)) => {
                valang::analyse_with_packages(&src, Some((keys, locales)), &hosts(), &packages)
            }
            None => valang::analyse_with_packages(&src, None, &hosts(), &packages),
        };

        println!("── {}", args.join(" "));
        for d in &diagnostics {
            // With the line it is about. A position on its own is a thing to go
            // and look up, and the file it is in is the one this loop read.
            let text = sources
                .iter()
                .find(|(p, _)| *p == *path)
                .map(|(_, s)| s.as_str())
                .unwrap_or(&src);
            for line in d.render(text).lines() {
                println!("  {line}");
            }
        }
        let errors = diagnostics.iter().filter(|d| d.severity == valang::Severity::Error).count();
        if errors > 0 {
            failed = true;
            println!("  {errors} error(s) — would not build");
            return if failed { ExitCode::from(1) } else { ExitCode::SUCCESS };
        }
        // From the module, because that is where what an application does to
        // the person comes from — and because printing anything else here
        // would be printing a second answer.
        let report = match valang_wasm::report_of(&program) {
            Ok(r) => r,
            Err(missing) => {
                for what in missing {
                    eprintln!("error: {what}");
                }
                return ExitCode::FAILURE;
            }
        };
        print!("{report}");

        if let Some(path) = &surface_path {
            if let Err(message) = check_surface(path, &report) {
                println!("  {message}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}


/// What this package exported the last time it was built at this version.
///
/// A change here is a change to somebody else's build. The file is kept beside
/// the package and read on the next build, which is the only way to notice: the
/// packages that depend on this one are not present, and their authors are not
/// in the room to say that a parameter moved.
///
/// A new version records its own surface and says nothing about the old one — a
/// version is exactly the thing that is allowed to differ.
fn check_surface(path: &str, report: &valang::report::Report) -> Result<(), String> {
    let mut held: serde_json::Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let now: Vec<String> = report.exports.iter().cloned().collect();
    let key = format!("{}/{}", report.app, report.version);

    if let Some(before) = held[&key].as_array() {
        let before: Vec<String> =
            before.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        if before != now {
            let gone: Vec<&String> = before.iter().filter(|x| !now.contains(x)).collect();
            let new: Vec<&String> = now.iter().filter(|x| !before.contains(x)).collect();
            let mut said = String::new();
            for x in gone {
                said.push_str(&format!("\n      was  {x}"));
            }
            for x in new {
                said.push_str(&format!("\n      now  {x}"));
            }
            return Err(format!(
                "error: `{key}` published a different surface, and a package that imports it was built against the old one. That is a new version, not an edit:{said}"
            ));
        }
        return Ok(());
    }

    held[&key] = serde_json::json!(now);
    if let Err(e) = std::fs::write(path, format!("{:#}\n", held)) {
        return Err(format!("error: cannot write {path}: {e}"));
    }
    Ok(())
}

/// The packages under each `--packages` directory.
///
/// One subdirectory is one package: its `.val` files are read together, because
/// a package is several files sharing one scope, and it identifies itself by the
/// `app` and `version` it declares rather than by what the directory is called.
fn packages(dirs: &[String]) -> valang::expand::Packages {
    let mut loaded = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            eprintln!("{dir}: cannot read");
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let mut src = String::new();
            if let Ok(files) = std::fs::read_dir(&path) {
                let mut paths: Vec<_> = files
                    .flatten()
                    .map(|f| f.path())
                    .filter(|p| p.extension().is_some_and(|e| e == "val"))
                    .collect();
                // Read in a fixed order: a package is one scope, and a
                // diagnostic that moved because the filesystem listed
                // differently is a diagnostic nobody can cite.
                paths.sort();
                for p in paths {
                    if let Ok(text) = std::fs::read_to_string(&p) {
                        src.push_str(&text);
                        src.push('\n');
                    }
                }
            }
            if src.is_empty() {
                continue;
            }
            // Parsed, not checked. Whether that package builds is its own
            // build's answer, and running its checks here would report its
            // unrelated mistakes during somebody else's build. What is taken
            // from it is checked on the way in.
            let (program, d) = valang::parse::parse(&src);
            for x in d.iter().filter(|x| x.severity == valang::Severity::Error) {
                eprintln!("{}: {x}", path.display());
            }
            loaded.push(program);
        }
    }
    valang::expand::Packages::of(loaded)
}

/// What to check against.
///
/// A real host hands the compiler its own registry; what a command line has is
/// whatever is on disk beside the language. The core one is built in because a
/// package naming nothing else is checked against it.
fn hosts() -> valang::capability::Hosts {
    const CORE: &str = include_str!("../../../hosts/core.json");
    const VAULET: &str = include_str!("../../../hosts/vaulet.json");

    let mut loaded = Vec::new();
    for source in [CORE, VAULET] {
        match valang::capability::Host::parse(source) {
            Ok(h) => loaded.push(h),
            Err(e) => eprintln!("a host registry did not parse: {e}"),
        }
    }
    valang::capability::Hosts::of(loaded)
}
