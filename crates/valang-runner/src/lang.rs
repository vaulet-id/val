//! One language per file: what to write beside the handler, and what to run.
//!
//! The handler a publisher wrote is never modified. What changes per language
//! is the SDK module written beside it and the entry file that calls it, so a
//! handler can be moved from the playground to a real deployment unchanged.

use std::path::Path;

pub struct Lang {
    pub ext: &'static str,
    /// The SDK module, written beside the handler under this name.
    pub sdk_name: &'static str,
    pub sdk: &'static str,
    /// The file that reads stdin, calls the handler and prints the decision.
    pub entry_name: &'static str,
    pub entry: &'static str,
    /// argv, with `{dir}` replaced by the working directory.
    pub run: &'static [&'static str],
    /// Written once before running: a manifest the toolchain needs.
    pub extra: &'static [(&'static str, &'static str)],
    /// Where the author's own files go, relative to the working directory. Go
    /// wants one package per directory, and the entry point is `package main`.
    pub files_dir: &'static str,
}

const TS_ENTRY: &str = r#"import { main } from './val.mjs'
import handler from './handler.ts'

await main(handler)
"#;

const PY_ENTRY: &str = r#"import val
from handler import handle

val.main(handle)
"#;

const GO_ENTRY: &str = r#"package main

import (
	"runner/handler"
	"runner/val"
)

func main() {
	val.Main(handler.Handle)
}
"#;

const RS_ENTRY: &str = r#"mod handler;
mod val;

fn main() {
    val::main_with(handler::handle);
}
"#;

const GO_MOD: &str = r#"module runner

go 1.21
"#;

/// Rust is the one language whose SDK has a dependency, because it is the one
/// with no JSON in its standard library. Cargo builds offline against the
/// registry cache the image ships, so a handler still cannot pull anything of
/// its own.
const CARGO_TOML: &str = r#"[package]
name = "handler-bin"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "handler-bin"
path = "main.rs"

[dependencies]
serde_json = "1"

[profile.dev]
debug = false
"#;

pub const LANGS: &[Lang] = &[
    Lang {
        ext: "ts",
        sdk_name: "val.mjs",
        sdk: include_str!("../sdk/val.mjs"),
        entry_name: "entry.mjs",
        entry: TS_ENTRY,
        // Node strips the types rather than compiling them, so what runs is the
        // file the author wrote and there is no build step to keep faithful.
        run: &["node", "--experimental-strip-types", "--no-warnings", "entry.mjs"],
        extra: &[],
        files_dir: "",
    },
    Lang {
        ext: "py",
        sdk_name: "val.py",
        sdk: include_str!("../sdk/val.py"),
        entry_name: "entry.py",
        entry: PY_ENTRY,
        run: &["python3", "entry.py"],
        extra: &[],
        files_dir: "",
    },
    Lang {
        ext: "go",
        sdk_name: "val/val.go",
        sdk: include_str!("../sdk/val.go"),
        entry_name: "main.go",
        entry: GO_ENTRY,
        run: &["go", "run", "."],
        extra: &[("go.mod", GO_MOD)],
        files_dir: "handler",
    },
    Lang {
        ext: "rs",
        sdk_name: "val.rs",
        sdk: include_str!("../sdk/val.rs"),
        entry_name: "main.rs",
        entry: RS_ENTRY,
        run: &["cargo", "run", "--quiet", "--offline"],
        extra: &[("Cargo.toml", CARGO_TOML)],
        files_dir: "",
    },
];

pub fn lang_of(entry: &str) -> Option<&'static Lang> {
    let ext = Path::new(entry).extension()?.to_str()?;
    LANGS.iter().find(|l| l.ext == ext)
}
