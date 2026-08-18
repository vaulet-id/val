//! Where a handler actually runs.
//!
//! Every language is given the same shape: a directory holding the author's
//! files, the SDK module and a generated entry point; the payload on stdin; one
//! JSON decision on stdout. What differs is the command.
//!
//! The isolation here is a fresh directory, no arguments from the caller, and a
//! wall-clock limit. On a host with KVM the same command runs inside a
//! Firecracker microVM instead — see `runner/README.md` — and nothing above
//! this module changes, because the contract is a process with stdin and
//! stdout.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt as _;
use tokio::process::Command;

use crate::lang::Lang;

/// A handler answers in well under a second or it is not answering. The limit
/// is generous enough for a cold `go run`, which compiles before it runs.
const LIMIT: Duration = Duration::from_secs(30);

pub async fn execute(
    lang: &Lang,
    files: &[crate::File],
    payload: &str,
) -> Result<String, String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let root = dir.path();

    for f in files {
        // The author's own file names, unmodified: a handler that imports
        // `./sign` has to find `sign` where it left it.
        let path = root.join(lang.files_dir).join(&f.name);
        write(&path, &f.source).await?;
    }

    write(&root.join(lang.sdk_name), lang.sdk).await?;
    write(&root.join(lang.entry_name), lang.entry).await?;
    for (name, source) in lang.extra {
        write(&root.join(name), source).await?;
    }

    run(lang.run, root, payload).await
}

/// The few variables a toolchain needs, and nothing else. A handler is given
/// its record; inheriting the service's environment would hand it whatever
/// credentials that process is holding.
fn passthrough(dir: &std::path::Path) -> Vec<(String, String)> {
    let mut out = vec![
        ("GOCACHE".into(), dir.join("go-build").display().to_string()),
        ("GOPATH".into(), dir.join("go").display().to_string()),
        ("GOFLAGS".into(), "-mod=mod".into()),
        // No module downloads: a handler's dependencies are the SDK beside it.
        ("GOPROXY".into(), "off".into()),
        // Cargo writes its build here rather than beside the author's files.
        ("CARGO_TARGET_DIR".into(), dir.join("target").display().to_string()),
    ];
    for k in ["RUSTUP_TOOLCHAIN", "GOROOT"] {
        if let Ok(v) = std::env::var(k) {
            out.push((k.into(), v));
        }
    }

    // HOME points at the sandbox, so rustup would look for its toolchains
    // inside it, find none, and download a whole channel before compiling. Say
    // where they already are.
    let real_home = std::env::var("HOME").unwrap_or_default();
    for (k, dir_name) in [("RUSTUP_HOME", ".rustup"), ("CARGO_HOME", ".cargo")] {
        let value = std::env::var(k).unwrap_or_else(|_| format!("{real_home}/{dir_name}"));
        out.push((k.into(), value));
    }
    out
}

async fn write(path: &std::path::Path, source: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::write(path, source).await.map_err(|e| e.to_string())
}

async fn run(argv: &[&str], dir: &std::path::Path, payload: &str) -> Result<String, String> {
    let mut child = Command::new(argv[0])
        .args(&argv[1..])
        .current_dir(dir)
        // A handler is given its record and nothing else. Inheriting the
        // service's environment would hand it whatever credentials that process
        // is holding.
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", dir)
        // A toolchain has to be able to find itself. rustc here is a rustup
        // shim, which without these picks no toolchain and says so instead of
        // compiling; go wants a cache it can write to.
        .envs(passthrough(dir))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{} is not installed here: {e}", argv[0]))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
    }

    let done = tokio::time::timeout(LIMIT, child.wait_with_output()).await;
    let out = match done {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(e.to_string()),
        Err(_) => return Err(format!("the handler did not answer within {}s", LIMIT.as_secs())),
    };

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(stderr.trim().to_string());
    }

    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
