//! Who signed a package, and with what.
//!
//! **The algorithm is the key's shape.** A 32-byte key is Ed25519; a 65-byte
//! one beginning `0x04` is an uncompressed P-256 point. There is nothing in a
//! package for a publisher to declare here and therefore nothing to declare
//! wrongly.

use std::collections::BTreeMap;

use valang_package::*;

const STAFF: &str = include_str!("../../../examples/staff.val");

fn registries() -> valang::capability::Hosts {
    valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")])
}

/// A publisher whose document this host resolved. `did:web`, because a
/// `did:key` is an Ed25519 key by construction and this is about the other one.
struct Wallet(Vec<u8>);

impl HostPolicy for Wallet {
    fn owns_key(&self, _publisher: &str, key: &[u8]) -> bool {
        self.0 == key
    }
}

fn manifest() -> Manifest {
    Manifest {
        app: "th.co.acme.staff".into(),
        version: "1".into(),
        kind: "val".into(),
        publisher: "did:web:acme.co.th".into(),
        catalogue: "1".into(),
        locales: vec!["th".into(), "en".into()],
    }
}

fn text() -> BTreeMap<String, BTreeMap<String, String>> {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/staff-text.json")).expect("parses");
    json["keys"]
        .as_object()
        .expect("keys")
        .iter()
        .map(|(key, per_locale)| {
            (
                key.clone(),
                per_locale
                    .as_object()
                    .expect("locales")
                    .iter()
                    .map(|(l, s)| (l.clone(), s.as_str().unwrap_or_default().to_string()))
                    .collect(),
            )
        })
        .collect()
}

/// Fixed rather than generated. A dev-dependency that wanted randomness once
/// leaked `getrandom` across this workspace's Cargo graph and broke a build
/// that has no operating system under it.
fn signing_key() -> p256::ecdsa::SigningKey {
    p256::ecdsa::SigningKey::from_bytes(&[7u8; 32].into()).expect("a valid scalar")
}

fn point(key: &p256::ecdsa::SigningKey) -> Vec<u8> {
    key.verifying_key().to_encoded_point(false).as_bytes().to_vec()
}

/// Built unsigned, then signed with P-256 the way a publisher whose key is in a
/// KMS would: the package's own bytes, and the point their document publishes.
fn published() -> (Package, Vec<u8>) {
    let key = signing_key();
    let sources = BTreeMap::from([("staff.val".to_string(), STAFF.to_string())]);
    let mut pkg = build(manifest(), sources, text(), &registries(), None).expect("builds");

    use p256::ecdsa::signature::Signer as _;
    let signature: p256::ecdsa::Signature = key.sign(&signable_bytes(&pkg));
    pkg.signature = Some(signature.to_bytes().to_vec());
    pkg.public_key = Some(point(&key));
    (pkg, point(&key))
}

/// What a signature is over, from the crate rather than reassembled here.
fn signable_bytes(p: &Package) -> Vec<u8> {
    let mut unsigned = p.clone();
    unsigned.signature = None;
    unsigned.public_key = None;
    // `encode` writes `signed` — the canonical bytes everything else signs and
    // verifies — as its own field, so this reads it back out rather than being
    // a second implementation of it.
    let Ok(valang_runtime::value::Value::Map(m)) = valang_runtime::decode::decode(&encode(&unsigned))
    else {
        panic!("a package encodes to a map")
    };
    match m.get("signed") {
        Some(valang_runtime::value::Value::Bytes(b)) => b.clone(),
        other => panic!("no signed bytes: {other:?}"),
    }
}

#[test]
fn a_package_may_be_signed_with_the_curve_a_kms_holds() {
    let (pkg, key) = published();
    assert_eq!(pkg.public_key.as_ref().map(|k| k.len()), Some(65));
    install_with(&pkg, &Wallet(key)).expect("this wallet admits it");
}

/// The key travels with the signature, so a package signed by somebody else's
/// key is caught by the publisher check rather than by the maths.
#[test]
fn a_p256_key_that_is_not_the_publishers_is_refused() {
    let (pkg, _) = published();
    match install_with(&pkg, &Wallet(vec![0x04; 65])) {
        Err(Refusal::Unsigned(why)) => assert!(why.contains("does not belong"), "{why}"),
        Ok(_) => panic!("a package signed by a stranger was admitted"),
        Err(e) => panic!("refused for the wrong reason: {e:?}"),
    }
}

#[test]
fn a_p256_signature_over_other_bytes_is_refused() {
    let (mut pkg, key) = published();
    // One byte of the module, which the signature covers through `integrity`.
    let last = pkg.module.len() - 1;
    pkg.module[last] ^= 0xff;
    pkg.integrity = hex_of(&pkg.module);

    match install_with(&pkg, &Wallet(key)) {
        Err(Refusal::Unsigned(why)) => assert!(why.contains("not over these bytes"), "{why}"),
        Ok(_) => panic!("edited bytes were admitted"),
        Err(e) => panic!("refused for the wrong reason: {e:?}"),
    }
}

/// Neither curve. A key of the wrong length is refused for being one, rather
/// than being read as the other and failing somewhere less legible.
#[test]
fn a_key_of_neither_shape_is_refused_for_being_neither() {
    let (mut pkg, _) = published();
    pkg.public_key = Some(vec![0x04; 33]);
    match install_with(&pkg, &Wallet(vec![0x04; 33])) {
        Err(Refusal::Unsigned(why)) => assert!(why.contains("neither"), "{why}"),
        Ok(_) => panic!("a key of no known shape was admitted"),
        Err(e) => panic!("refused for the wrong reason: {e:?}"),
    }
}
