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

/// What a host will admit.
///
/// The package format carries `kind` and a catalogue version and takes no view
/// about either — that is host policy, and a language repository holding one
/// would be the first host leaking into the language (§1). A host supplies this
/// and gets its own ceiling.
pub trait HostPolicy {
    /// Whether an application of this kind may hold this capability. The first
    /// host answers no to `credential.issue` for a webview, not as a preference
    /// but because it cannot say what ran.
    fn allows(&self, _kind: &str, _capability: &str) -> bool {
        true
    }

    /// Whether this host can render the catalogue the package was built
    /// against. Refusing is the honest answer: an application signed against v1
    /// gets v1's semantics or nothing.
    fn supports_catalogue(&self, _version: &str) -> bool {
        true
    }

    /// Whether a package of this kind may carry VAL sources at all.
    fn expects_sources(&self, kind: &str) -> bool {
        kind == "val"
    }

    /// What this host draws with. A package is compiled against it before the
    /// host admits it — there is no other copy, and a package checked against
    /// somebody else's catalogue has been checked against nothing.
    fn registries(&self) -> valang::capability::Hosts;
}

/// Admits everything, and publishes nothing. The default so that `verify` alone
/// answers the questions that are the language's, and a host that has no policy
/// yet is not silently given one.
///
/// Its registry is empty, which means a screen is checked against no catalogue.
/// That is a real answer rather than an oversight: a host with nothing to draw
/// with admits nothing worth drawing.
pub struct Permissive;
impl HostPolicy for Permissive {
    fn registries(&self) -> valang::capability::Hosts {
        Default::default()
    }
}

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
    /// The host will not admit this, and the reason is the host's.
    Refused { by: String },
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
    hosts: &valang::capability::Hosts,
    key: Option<&SigningKey>,
) -> Result<Package, Refusal> {
    let joined = sources.values().cloned().collect::<Vec<_>>().join("\n");
    // Against the registries this package names, because a screen is checked
    // against what a host publishes and against nothing else. Compiled without
    // them, a package drawing something no wallet ships was admitted and failed
    // on somebody's phone.
    let (program, diagnostics) =
        valang::analyse_fully(&joined, Some((&text_bundle, &manifest.locales)), hosts);
    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    if !errors.is_empty() {
        return Err(Refusal::WouldNotBuild(errors));
    }

    // The manifest is what a host reads before it reads the code, and a person
    // reads the name off it. Two answers to which application this is means the
    // one on the consent sheet is not the one that runs.
    //
    // Only where there is code to disagree with: a webview package carries none,
    // and what it does is checked from its report instead.
    let mut disagreements = Vec::new();
    if !sources.is_empty() {
        match &program.app {
            Some(app) if *app != manifest.app => disagreements.push(format!(
                "the manifest calls this `{}` and the code calls it `{app}`",
                manifest.app
            )),
            None => disagreements
                .push("the manifest names an application and the code names none".to_string()),
            _ => {}
        }
        match &program.version {
            Some(v) if *v != manifest.version => disagreements.push(format!(
                "the manifest says version {} and the code says {v}",
                manifest.version
            )),
            None => {
                disagreements.push("the manifest has a version and the code has none".to_string())
            }
            _ => {}
        }
    }
    if !disagreements.is_empty() {
        return Err(Refusal::WouldNotBuild(disagreements));
    }

    let integrity = sources
        .iter()
        .map(|(path, text)| (path.clone(), hex(&Sha256::digest(text.as_bytes()))))
        .collect();

    let mut pkg = Package {
        manifest,
        sources,
        text_bundle,
        report: report_rows(&valang_wasm::report_of(&program).map_err(Refusal::WouldNotBuild)?),
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
/// A package this host has admitted, and the program it admitted.
///
/// **The compile that was checked is the compile that runs.** Verifying and
/// then running meant compiling twice, and two compiles of the same text are
/// the same program only for as long as that is true — a host that checked one
/// and ran the other would have checked nothing. So the check hands its
/// program over rather than dropping it.
pub struct Installed {
    pub manifest: Manifest,
    /// `None` for a `webview` package, which carries no code — that is the
    /// whole difference between the tiers, and the reason such a package's
    /// report is a declaration rather than something derived.
    pub code: Option<Code>,
    pub text_bundle: BTreeMap<String, BTreeMap<String, String>>,
}

/// A program and the text it was compiled from, which cannot be separated: the
/// runtime hashes exactly this text into every execution record, so a verifier
/// can say which code ran.
pub struct Code {
    pub program: valang::ast::Program,
    pub source: String,
}

pub fn verify(p: &Package) -> Result<(), Refusal> {
    verify_with(p, &Permissive)
}

/// Everything `install_with` does, for a caller that wants the answer and not
/// the program — a store listing packages, or a publisher's build.
pub fn verify_with(p: &Package, policy: &dyn HostPolicy) -> Result<(), Refusal> {
    install_with(p, policy).map(|_| ())
}

/// Admit a package, or refuse it, and hand back what to run.
///
/// This is what a host does when somebody installs an application: check that
/// nothing was modified after it was signed, that the publisher is who it says,
/// that it compiles **against this host's catalogue**, that the manifest and
/// the code name the same application, that the report it ships is the report
/// its code produces, and that this host admits what it asks for.
pub fn install_with(p: &Package, policy: &dyn HostPolicy) -> Result<Installed, Refusal> {
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

    // A package with no sources is a tier where none of the next two checks can
    // be made — and that is the reason the tier has a lower ceiling, not a
    // consequence of it. **The report can only be derived from code the host
    // compiled.** For anything else it is a declaration, and a declaration is
    // exactly what a person cannot check.
    if !policy.expects_sources(&p.manifest.kind) {
        if !p.sources.is_empty() {
            return Err(Refusal::Refused {
                by: format!("a `{}` package carries no VAL sources", p.manifest.kind),
            });
        }
        ceiling(p, policy)?;
        locales(p)?;
        return Ok(Installed { manifest: p.manifest.clone(), code: None, text_bundle: p.text_bundle.clone() });
    }
    if p.sources.is_empty() {
        return Err(Refusal::Refused {
            by: format!("a `{}` package carries VAL sources, and this one carries none", p.manifest.kind),
        });
    }

    // 3. It compiles — checked here, not taken on trust from a build we did not
    //    run.
    let joined = p.sources.values().cloned().collect::<Vec<_>>().join("\n");
    // The host's own registries, which is the only copy that matters: what this
    // wallet ships is what this package has to have been written against.
    let (program, diagnostics) =
        valang::analyse_fully(&joined, Some((&p.text_bundle, &p.manifest.locales)), &policy.registries());
    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    if !errors.is_empty() {
        return Err(Refusal::WouldNotBuild(errors));
    }

    // The manifest is what a host reads before it reads the code, and a person
    // reads the name off it. Two answers to which application this is means the
    // one on the consent sheet is not the one that runs.
    //
    // Only where there is code to disagree with: a webview package carries none,
    // and what it does is checked from its report instead.
    let mut disagreements = Vec::new();
    if !p.sources.is_empty() {
        match &program.app {
            Some(app) if *app != p.manifest.app => disagreements.push(format!(
                "the manifest calls this `{}` and the code calls it `{app}`",
                p.manifest.app
            )),
            None => disagreements
                .push("the manifest names an application and the code names none".to_string()),
            _ => {}
        }
        match &program.version {
            Some(v) if *v != p.manifest.version => disagreements.push(format!(
                "the manifest says version {} and the code says {v}",
                p.manifest.version
            )),
            None => {
                disagreements.push("the manifest has a version and the code has none".to_string())
            }
            _ => {}
        }
    }
    if !disagreements.is_empty() {
        return Err(Refusal::WouldNotBuild(disagreements));
    }

    // 4. The report it ships is the report its code produces.
    // From the module: what an application does to the person is the import
    // section of the thing that runs, and a package whose code will not compile
    // to one cannot be admitted at all.
    let derived = report_rows(&valang_wasm::report_of(&program).map_err(Refusal::WouldNotBuild)?);
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

    // 5. What this host will admit, which is not the language's business.
    ceiling(p, policy)?;

    // 6. Every locale the manifest promises has every key.
    locales(p)?;

    Ok(Installed {
        manifest: p.manifest.clone(),
        code: Some(Code { program, source: joined }),
        text_bundle: p.text_bundle.clone(),
    })
}

fn ceiling(p: &Package, policy: &dyn HostPolicy) -> Result<(), Refusal> {
    if !policy.supports_catalogue(&p.manifest.catalogue) {
        return Err(Refusal::Refused {
            by: format!(
                "this host does not render catalogue {}. An application signed against a catalogue gets that catalogue's semantics or nothing — a component that means something else on a later version is a screen the person did not consent to",
                p.manifest.catalogue
            ),
        });
    }
    for (line, values) in &p.report {
        if line == "irreversible" || values.is_empty() {
            continue;
        }
        let capability = match line.as_str() {
            "issues" => "credential.issue",
            "moves money" => "payment.request",
            "discloses" | "proves" => "disclosure.present",
            "talks to" => "api.query",
            _ => continue,
        };
        if !policy.allows(&p.manifest.kind, capability) {
            return Err(Refusal::Refused {
                by: format!(
                    "a `{}` application may not `{capability}`. Capabilities follow verifiability rather than preference: a host that cannot state what ran cannot offer the capabilities whose safety depends on saying it",
                    p.manifest.kind
                ),
            });
        }
    }
    Ok(())
}

fn locales(p: &Package) -> Result<(), Refusal> {
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
