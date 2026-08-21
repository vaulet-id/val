//! The `.vapp` package: a compiled Micro App, signed by whoever published it.
//!
//! **It carries the module and no source.** A wallet has no compiler and is
//! never going to have one — compiling on a phone is not a thing anybody
//! ships — so what a wallet is handed is what runs, and every check it makes is
//! a check on those bytes: they hash to what the package says, the publisher
//! signed them, what they can reach is what the package claimed, and this is
//! that application at that version.
//!
//! The old worry — that a hash over bytecode proves it is the bytecode somebody
//! signed and never that it is the program somebody read — is answered by
//! **reproducible builds** rather than by shipping the source to every phone.
//! The publisher publishes their source; anybody who cares builds it and
//! compares the bytes, once. And a wallet never has to take the report on
//! trust either, because it derives it from the module's import section: what a
//! module can reach is what it imports, and it imports nothing else.
//!
//! It answers, to somebody who did not build it: who published this, which
//! version is running, whether it was modified, and what it may do.

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

    /// Whether a package of this kind carries a compiled module at all.
    ///
    /// The tier that does is the tier whose report can be derived; the tier
    /// that does not ships a declaration, and that is why its ceiling is lower.
    fn expects_module(&self, kind: &str) -> bool {
        kind == "val"
    }

    /// Whether this key really belongs to the publisher the manifest names.
    ///
    /// **`did:key` answers itself** — the name *is* the key, so `owns_key` is a
    /// comparison and this crate does it without asking anybody. Every other
    /// method needs a document fetched from somewhere, which is I/O and so is
    /// the host's; the default refuses them rather than admitting a package
    /// whose publisher is a claim.
    ///
    /// **A signature proves the bytes were not changed. It does not prove who
    /// made them.** Anybody can generate a key, sign a package and write
    /// `did:web:some.bank` in the manifest, and every check in this crate would
    /// pass — because every one of them is a check on the bytes. Binding a key
    /// to a name is somebody else's job: resolving the DID document, or a
    /// registry a host trusts, neither of which belongs in a crate that does no
    /// I/O.
    ///
    /// The default says yes, which is right for a build tool and wrong for a
    /// wallet. **A wallet that does not implement this is a wallet where any
    /// publisher can be any publisher.**
    fn owns_key(&self, publisher: &str, key: &[u8]) -> bool {
        match key_in_did(publisher) {
            // The name carries the key: it either is this one or it is not.
            Some(named) => named == key,
            // A name that has to be looked up. A host that resolves it says so
            // by overriding this; one that does not refuses, because admitting
            // it would mean any publisher could be any publisher.
            None => false,
        }
    }

    /// The largest module this host will read. Validating one costs time
    /// proportional to its size, and a package is something anybody can send.
    fn largest_module(&self) -> usize {
        4 * 1024 * 1024
    }
}

/// Admits everything, and publishes nothing. The default so that `verify` alone
/// answers the questions that are the language's, and a host that has no policy
/// yet is not silently given one.
///
/// It admits every kind and every catalogue, and — like every host — refuses a
/// publisher whose name has to be looked up, because looking one up is I/O and
/// this crate does none.
pub struct Permissive;
impl HostPolicy for Permissive {}

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
    /// **The compiled module, and no source.** A wallet has no compiler and is
    /// never going to have one: what it is handed is what runs. The source is
    /// the publisher's to publish, and anybody who wants to check them can
    /// build it and compare the bytes — which is a thing done once, by whoever
    /// cares, and not on every phone at every install.
    pub module: Vec<u8>,
    pub text_bundle: BTreeMap<String, BTreeMap<String, String>>,
    /// Derived, and shipped only so a store can list it. The host derives it
    /// again from the module and refuses on mismatch, because a publisher's
    /// copy is evidence of nothing.
    pub report: BTreeMap<String, Vec<String>>,
    /// What the module hashes to. One entry, because there is one artifact.
    pub integrity: String,
    pub signature: Option<Vec<u8>>,
    pub public_key: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The module does not hash to what integrity says it does.
    Modified(String),
    /// The signature is absent, malformed, or not this publisher's.
    Unsigned(String),
    /// The publisher's build did not produce something admissible: it does not
    /// compile, or the module and the manifest do not name the same
    /// application at the same version.
    WouldNotBuild(Vec<String>),
    /// The shipped report is not the one the code produces. Understating what
    /// an application does is the one lie a package could otherwise tell.
    ReportMismatch { line: String, shipped: Vec<String>, derived: Vec<String> },
    Malformed(String),
    /// The host will not admit this, and the reason is the host's.
    Refused { by: String },
}

/// The key a `did:key` names, or nothing for a name that has to be looked up.
///
/// `did:key:z6Mk…` is base58btc over the multicodec prefix for an Ed25519
/// public key and the key itself. Nothing to fetch and nobody to trust: the
/// name and the key are the same fact written twice.
pub fn key_in_did(did: &str) -> Option<Vec<u8>> {
    let rest = did.strip_prefix("did:key:")?;
    // `z` is base58btc in multibase, and it is the only encoding `did:key`
    // uses. Another prefix is another encoding and this build does not know it.
    let bytes = bs58::decode(rest.strip_prefix('z')?).into_vec().ok()?;
    // 0xed 0x01 — multicodec `ed25519-pub`, little-endian varint.
    let key = bytes.strip_prefix(&[0xed, 0x01])?;
    (key.len() == 32).then(|| key.to_vec())
}

/// The `did:key` for a signing key, which is how a publisher who has no domain
/// says who they are.
pub fn did_for(key: &SigningKey) -> String {
    let mut bytes = vec![0xed, 0x01];
    bytes.extend_from_slice(&key.verifying_key().to_bytes());
    format!("did:key:z{}", bs58::encode(bytes).into_string())
}

/// What integrity says about a module: the hash, as it is written down.
pub fn hex_of(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// And as an execution record names it. The same bytes hashed the same way, so
/// somebody holding a package and a record can say they are one thing.
pub fn hash_of(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
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

    m.insert("module".into(), Value::Bytes(p.module.clone()));
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
    m.insert("integrity".into(), Value::Str(p.integrity.clone()));

    DeterministicCbor.encode(&Value::Map(m))
}

pub fn report_rows(r: &Report) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let put = |out: &mut BTreeMap<String, Vec<String>>, k: &str, v: &std::collections::BTreeSet<String>| {
        out.insert(k.to_string(), v.iter().cloned().collect());
    };
    put(&mut out, "reads", &r.reads);
    put(&mut out, "checks", &r.checks);
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

/// Build a package from sources — on the publisher's machine, which is the only
/// place a compiler runs.
///
/// The report is derived here from the module this produces, and derived again
/// by the host from the module it receives; shipping it is a convenience for a
/// store listing and evidence of nothing on its own.
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

    // The artifact. Compiling happens here, on the publisher's machine, once —
    // and never again anywhere else.
    //
    // A package with no sources produces no module rather than an empty one:
    // the tier that carries no code is the tier whose report cannot be derived,
    // and a module with nothing in it would look like a derivation that found
    // nothing to say.
    let module = if sources.is_empty() {
        Vec::new()
    } else {
        valang_wasm::compile::compile_program(&program).map_err(Refusal::WouldNotBuild)?.bytes
    };
    let integrity = hex(&Sha256::digest(&module));

    let mut pkg = Package {
        manifest,
        // Derived where there is something to derive it from, and left for the
        // publisher to declare where there is not. That difference is the whole
        // difference between the tiers, and it is why one of them has a lower
        // ceiling.
        report: if module.is_empty() {
            BTreeMap::new()
        } else {
            report_rows(&valang_wasm::compile::report_of_module(&module).ok_or_else(|| {
                Refusal::Malformed("the module it just built cannot be read".into())
            })?)
        },
        module,
        text_bundle,
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

/// What runs, and what a record says about it. They cannot be separated: the
/// record names the hash of exactly these bytes, so somebody holding the
/// package and the record can say the two are the same thing.
pub struct Code {
    pub module: Vec<u8>,
    pub about: valang_runtime::About,
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
/// **Nothing here compiles anything.** A wallet is handed a module and checks
/// it: that it hashes to what the package says, that the publisher signed those
/// bytes, that what it can do is what the package claimed, that it is this
/// application at this version, and that this host admits what it asks for.
/// Compiling happened once, on the publisher's machine — and whoever wants to
/// know that the module is the source the publisher published builds the source
/// and compares the bytes, which is a thing done by someone who cares rather
/// than by every phone at every install.
pub fn install_with(p: &Package, policy: &dyn HostPolicy) -> Result<Installed, Refusal> {
    // 1. The bytes are the bytes.
    if hex(&Sha256::digest(&p.module)) != p.integrity {
        return Err(Refusal::Modified("the module".into()));
    }

    if p.module.len() > policy.largest_module() {
        return Err(Refusal::Refused {
            by: format!(
                "this module is {} bytes and this host reads at most {}",
                p.module.len(),
                policy.largest_module()
            ),
        });
    }

    // 2. The publisher is who the package says.
    let (Some(sig), Some(pk)) = (&p.signature, &p.public_key) else {
        return Err(Refusal::Unsigned("no signature".into()));
    };
    let key = VerifyingKey::from_bytes(&pk.as_slice().try_into().map_err(|_| Refusal::Unsigned("malformed key".into()))?)
        .map_err(|_| Refusal::Unsigned("malformed key".into()))?;
    let signature = Signature::from_slice(sig).map_err(|_| Refusal::Unsigned("malformed signature".into()))?;
    key.verify(&signable(p), &signature).map_err(|_| Refusal::Unsigned("the signature is not over these bytes".into()))?;

    // …and that the key is the publisher's, which the signature does not say.
    if !policy.owns_key(&p.manifest.publisher, pk) {
        return Err(Refusal::Unsigned(format!(
            "this key does not belong to `{}`",
            p.manifest.publisher
        )));
    }

    // A package with no module is a tier where the next two checks cannot be
    // made — and that is the reason the tier has a lower ceiling, not a
    // consequence of it. **A report can only be derived from something that
    // runs.** For anything else it is a declaration, and a declaration is
    // exactly what a person cannot check.
    if !policy.expects_module(&p.manifest.kind) {
        if !p.module.is_empty() {
            return Err(Refusal::Refused {
                by: format!("a `{}` package carries no module", p.manifest.kind),
            });
        }
        ceiling(p, policy)?;
        locales(p)?;
        return Ok(Installed { manifest: p.manifest.clone(), code: None, text_bundle: p.text_bundle.clone() });
    }
    if p.module.is_empty() {
        return Err(Refusal::Refused {
            by: format!("a `{}` package carries a module, and this one carries none", p.manifest.kind),
        });
    }

    // 3. It is a module this host can describe, and everything it reaches for
    //    is something this host provides. A module importing a name we do not
    //    know is refused rather than linked: what the list says it can do would
    //    stop being the whole of what it can do.
    let about = valang_wasm::compile::about_of(&p.module)
        .ok_or_else(|| Refusal::Malformed("this is not a module this host can read".into()))?;
    let sheet = valang_wasm::compile::report_of_module(&p.module)
        .ok_or_else(|| Refusal::Refused { by: "this module reaches for something this host does not provide".into() })?;

    // The manifest is what a host reads before it reads the module, and a person
    // reads the name off it. Two answers to which application this is means the
    // one on the consent sheet is not the one that runs.
    let mut disagreements = Vec::new();
    if about.app != p.manifest.app {
        disagreements.push(format!(
            "the manifest calls this `{}` and the module calls it `{}`",
            p.manifest.app, about.app
        ));
    }
    if about.version != p.manifest.version {
        disagreements.push(format!(
            "the manifest says version {} and the module says {}",
            p.manifest.version, about.version
        ));
    }
    if !disagreements.is_empty() {
        return Err(Refusal::WouldNotBuild(disagreements));
    }

    // 4. The report it ships is the report its module produces.
    let derived = report_rows(&sheet);
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
        code: Some(Code { module: p.module.clone(), about }),
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

/// Read a `.vapp` back.
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
        module: match b.get("module") {
            Some(Value::Bytes(x)) => x.clone(),
            _ => Vec::new(),
        },
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
        integrity: str_at(b.get("integrity")),
        signature,
        public_key,
    })
}

pub fn keygen() -> SigningKey {
    SigningKey::generate(&mut rand::rngs::OsRng)
}
