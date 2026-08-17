//! The `.va` package.
//!
//! It answers, to somebody who did not build it: who published this, which
//! version is running, whether it was modified, what it may do, and **what is
//! actually executing** — which is why the sources are in it. A hash over
//! bytecode proves it is the bytecode somebody signed; it never proves it is
//! the program somebody read (§1).

use std::collections::BTreeMap;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use valang::report::Report;
use valang_runtime::canonical::{Canonical, DeterministicCbor};
use valang_runtime::decode::decode;
use valang_runtime::value::Value;

pub const FORMAT: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub app: String,
    pub version: String,
    /// `val` or `webview`. Immutable within a version: changing it is a new
    /// version and a fresh consent.
    pub kind: String,
    pub publisher: String,
    /// The component catalogue this was built against. A host shipping a later
    /// one renders these semantics or refuses.
    pub catalogue: String,
    pub locales: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub manifest: Manifest,
    /// Path to text. Several files, one scope, no imports across packages.
    pub sources: BTreeMap<String, String>,
    pub text_bundle: BTreeMap<String, BTreeMap<String, String>>,
    /// Derived, and shipped only so a store can list it. The host recomputes
    /// it and refuses on mismatch, because the host owns the checker.
    pub report: BTreeMap<String, Vec<String>>,
    pub integrity: BTreeMap<String, String>,
    pub signature: Option<Vec<u8>>,
    pub public_key: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A source in the package does not hash to what integrity says it does.
    Modified(String),
    /// The signature is absent, malformed, or not this publisher's.
    Unsigned(String),
    /// The program does not compile. A publisher's build passing proves
    /// nothing: the host runs the checks itself.
    WouldNotBuild(Vec<String>),
    /// The shipped report is not the one the code produces. Understating what
    /// an application does is the one lie a package could otherwise tell.
    ReportMismatch { line: String, shipped: Vec<String>, derived: Vec<String> },
    Malformed(String),
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Everything except the signature, canonically encoded. This is what is
/// signed, and what is verified.
fn signable(p: &Package) -> Vec<u8> {
    let mut m = BTreeMap::new();
    m.insert("format".into(), Value::Int(FORMAT));

    let mut man = BTreeMap::new();
    man.insert("app".into(), Value::Str(p.manifest.app.clone()));
    man.insert("version".into(), Value::Str(p.manifest.version.clone()));
    man.insert("kind".into(), Value::Str(p.manifest.kind.clone()));
    man.insert("publisher".into(), Value::Str(p.manifest.publisher.clone()));
    man.insert("catalogue".into(), Value::Str(p.manifest.catalogue.clone()));
    man.insert(
        "locales".into(),
        Value::List(p.manifest.locales.iter().cloned().map(Value::Str).collect()),
    );
    m.insert("manifest".into(), Value::Map(man));

    m.insert(
        "sources".into(),
        Value::Map(p.sources.iter().map(|(k, v)| (k.clone(), Value::Str(v.clone()))).collect()),
    );
    m.insert(
        "text".into(),
        Value::Map(
            p.text_bundle
                .iter()
                .map(|(k, v)| {
                    (k.clone(), Value::Map(v.iter().map(|(l, s)| (l.clone(), Value::Str(s.clone()))).collect()))
                })
                .collect(),
        ),
    );
    m.insert(
        "report".into(),
        Value::Map(
            p.report
                .iter()
                .map(|(k, v)| (k.clone(), Value::List(v.iter().cloned().map(Value::Str).collect())))
                .collect(),
        ),
    );
    m.insert(
        "integrity".into(),
        Value::Map(p.integrity.iter().map(|(k, v)| (k.clone(), Value::Str(v.clone()))).collect()),
    );

    DeterministicCbor.encode(&Value::Map(m))
}

pub fn report_rows(r: &Report) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let put = |out: &mut BTreeMap<String, Vec<String>>, k: &str, v: &std::collections::BTreeSet<String>| {
        out.insert(k.to_string(), v.iter().cloned().collect());
    };
    put(&mut out, "reads", &r.reads);
    put(&mut out, "discloses", &r.discloses);
    put(&mut out, "proves", &r.proves);
    put(&mut out, "issues", &r.issues);
    put(&mut out, "talks to", &r.audiences);
    put(&mut out, "moves money", &r.payments);
    put(&mut out, "writes state", &r.writes);
    out.insert(
        "irreversible".into(),
        vec![if r.irreversible { "yes".into() } else { "none".into() }],
    );
    out
}

/// Build a package from sources. The report is derived here and recomputed by
/// the host; shipping it is a convenience for a store listing and evidence of
/// nothing on its own.
pub fn build(
    manifest: Manifest,
    sources: BTreeMap<String, String>,
    text_bundle: BTreeMap<String, BTreeMap<String, String>>,
    key: Option<&SigningKey>,
) -> Result<Package, Refusal> {
    let joined = sources.values().cloned().collect::<Vec<_>>().join("\n");
    let (program, diagnostics) = valang::analyse(&joined);
    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    if !errors.is_empty() {
        return Err(Refusal::WouldNotBuild(errors));
    }

    let integrity = sources
        .iter()
        .map(|(path, text)| (path.clone(), hex(&Sha256::digest(text.as_bytes()))))
        .collect();

    let mut pkg = Package {
        manifest,
        sources,
        text_bundle,
        report: report_rows(&valang::report::report(&program)),
        integrity,
        signature: None,
        public_key: None,
    };

    if let Some(k) = key {
        let sig: Signature = k.sign(&signable(&pkg));
        pkg.signature = Some(sig.to_bytes().to_vec());
        pkg.public_key = Some(k.verifying_key().to_bytes().to_vec());
    }
    Ok(pkg)
}

/// What a host does before it admits an application. Every step is one the
/// publisher could otherwise have been trusted about.
pub fn verify(p: &Package) -> Result<(), Refusal> {
    // 1. Nothing was modified after it was signed.
    for (path, text) in &p.sources {
        let want = p.integrity.get(path).ok_or_else(|| Refusal::Malformed(format!("{path} has no integrity entry")))?;
        if hex(&Sha256::digest(text.as_bytes())) != *want {
            return Err(Refusal::Modified(path.clone()));
        }
    }
    if p.integrity.len() != p.sources.len() {
        return Err(Refusal::Malformed("integrity names a file the package does not carry".into()));
    }

    // 2. The publisher is who the package says.
    let (Some(sig), Some(pk)) = (&p.signature, &p.public_key) else {
        return Err(Refusal::Unsigned("no signature".into()));
    };
    let key = VerifyingKey::from_bytes(&pk.as_slice().try_into().map_err(|_| Refusal::Unsigned("malformed key".into()))?)
        .map_err(|_| Refusal::Unsigned("malformed key".into()))?;
    let signature = Signature::from_slice(sig).map_err(|_| Refusal::Unsigned("malformed signature".into()))?;
    key.verify(&signable(p), &signature).map_err(|_| Refusal::Unsigned("the signature is not over these bytes".into()))?;

    // 3. It compiles — checked here, not taken on trust from a build we did not
    //    run.
    let joined = p.sources.values().cloned().collect::<Vec<_>>().join("\n");
    let (program, diagnostics) = valang::analyse(&joined);
    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    if !errors.is_empty() {
        return Err(Refusal::WouldNotBuild(errors));
    }

    // 4. The report it ships is the report its code produces.
    let derived = report_rows(&valang::report::report(&program));
    for (line, values) in &derived {
        let shipped = p.report.get(line).cloned().unwrap_or_default();
        if shipped != *values {
            return Err(Refusal::ReportMismatch {
                line: line.clone(),
                shipped,
                derived: values.clone(),
            });
        }
    }

    // 5. Every locale the manifest promises has every key.
    for (key, per_locale) in &p.text_bundle {
        for locale in &p.manifest.locales {
            if !per_locale.contains_key(locale) {
                return Err(Refusal::Malformed(format!(
                    "`{key}` has no {locale}. A market's language missing is a failed build, not a bug report"
                )));
            }
        }
    }

    Ok(())
}

/// The package's identity: the hash of everything that was signed.
pub fn artifact_hash(p: &Package) -> String {
    hex(&Sha256::digest(signable(p)))
}

pub fn encode(p: &Package) -> Vec<u8> {
    let mut m = BTreeMap::new();
    m.insert("signed".into(), Value::Bytes(signable(p)));
    if let Some(s) = &p.signature {
        m.insert("signature".into(), Value::Bytes(s.clone()));
    }
    if let Some(k) = &p.public_key {
        m.insert("publisher_key".into(), Value::Bytes(k.clone()));
    }
    DeterministicCbor.encode(&Value::Map(m))
}

/// Sign, or re-sign. Exposed because the interesting adversary is the
/// publisher: they hold the key, so a false report arrives correctly signed and
/// the mismatch check is what catches it — not the signature.
pub fn sign(pkg: &mut Package, key: &SigningKey) {
    let sig: Signature = key.sign(&signable(pkg));
    pkg.signature = Some(sig.to_bytes().to_vec());
    pkg.public_key = Some(key.verifying_key().to_bytes().to_vec());
}

/// Read a `.va` back.
///
/// `verify` re-encodes what this parsed and checks the signature over that,
/// which is only sound because the decoder is strict: a package encoded any way
/// other than the deterministic one is refused before it gets here. Without
/// that, re-encoding would mean checking a signature against the verifier's own
/// idea of the package — a check that passes whatever the file said.
pub fn read(bytes: &[u8]) -> Result<Package, Refusal> {
    let outer = decode(bytes).map_err(|e| Refusal::Malformed(format!("{e:?}")))?;
    let Value::Map(m) = outer else { return Err(Refusal::Malformed("a package is a map".into())) };

    let Some(Value::Bytes(signed)) = m.get("signed") else {
        return Err(Refusal::Malformed("no signed section".into()));
    };
    let signature = match m.get("signature") {
        Some(Value::Bytes(b)) => Some(b.clone()),
        _ => None,
    };
    let public_key = match m.get("publisher_key") {
        Some(Value::Bytes(b)) => Some(b.clone()),
        _ => None,
    };

    let body = decode(signed).map_err(|e| Refusal::Malformed(format!("{e:?}")))?;
    let Value::Map(b) = body else { return Err(Refusal::Malformed("the signed section is a map".into())) };

    let str_at = |v: Option<&Value>| match v {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    };
    let Some(Value::Map(man)) = b.get("manifest") else {
        return Err(Refusal::Malformed("no manifest".into()));
    };

    let manifest = Manifest {
        app: str_at(man.get("app")),
        version: str_at(man.get("version")),
        kind: str_at(man.get("kind")),
        publisher: str_at(man.get("publisher")),
        catalogue: str_at(man.get("catalogue")),
        locales: match man.get("locales") {
            Some(Value::List(l)) => l.iter().map(|v| str_at(Some(v))).collect(),
            _ => Vec::new(),
        },
    };

    let map_of_str = |v: Option<&Value>| -> BTreeMap<String, String> {
        match v {
            Some(Value::Map(m)) => m.iter().map(|(k, v)| (k.clone(), str_at(Some(v)))).collect(),
            _ => BTreeMap::new(),
        }
    };

    Ok(Package {
        manifest,
        sources: map_of_str(b.get("sources")),
        text_bundle: match b.get("text") {
            Some(Value::Map(m)) => m.iter().map(|(k, v)| (k.clone(), map_of_str(Some(v)))).collect(),
            _ => BTreeMap::new(),
        },
        report: match b.get("report") {
            Some(Value::Map(m)) => m
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        match v {
                            Value::List(l) => l.iter().map(|x| str_at(Some(x))).collect(),
                            _ => Vec::new(),
                        },
                    )
                })
                .collect(),
            _ => BTreeMap::new(),
        },
        integrity: map_of_str(b.get("integrity")),
        signature,
        public_key,
    })
}

pub fn keygen() -> SigningKey {
    SigningKey::generate(&mut rand::rngs::OsRng)
}
