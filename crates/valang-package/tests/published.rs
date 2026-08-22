//! The package the deployment is actually serving, checked with the key its
//! document actually publishes.
//!
//! **Both halves came off the wire**, from `org.vaulet.id` on 2026-08-22: the
//! bytes are `GET /acme/app/th.co.acme.staff`, and the key is the
//! `publicKeyJwk` in `/acme/did.json`, converted to a SEC1 point the way the
//! wallet converts it. Nothing here signs anything, so what passes is a
//! deployment and a wallet agreeing rather than this crate agreeing with
//! itself — which is the whole reason to keep bytes instead of rebuilding them.
//!
//! Refresh them by republishing and fetching both again. A signature that stops
//! verifying means the organisation rotated its key, which is a thing to notice
//! rather than a test to relax.

use valang_package::*;

const PUBLISHED: &[u8] = include_bytes!("../../../fixtures/published-by-acme.vapp");

/// `did:web:org.vaulet.id:acme#issuing-main` — P-256, in a KMS, published in
/// the organisation's own document.
const POINT: &[u8] = include_bytes!("../../../fixtures/acme-issuing-key.bin");

struct Wallet;

impl HostPolicy for Wallet {
    /// What the wallet's `resolvePublisher` does: fetch the document the DID
    /// names, and compare the key it publishes with the key that signed.
    fn owns_key(&self, publisher: &str, key: &[u8]) -> bool {
        publisher == "did:web:org.vaulet.id:acme" && key == POINT
    }
}

#[test]
fn what_the_deployment_serves_is_what_this_wallet_admits() {
    let pkg = read(PUBLISHED).expect("it decodes");
    assert_eq!(pkg.manifest.app, "th.co.acme.staff");
    assert_eq!(pkg.manifest.publisher, "did:web:org.vaulet.id:acme");

    // P-256, because an organisation's issuing key is — and the package carries
    // the point rather than saying which curve it is.
    assert_eq!(pkg.public_key.as_deref(), Some(POINT));
    assert_eq!(POINT.len(), 65);
    assert_eq!(POINT[0], 0x04, "an uncompressed SEC1 point");

    let installed = install_with(&pkg, &Wallet).expect("this wallet admits it");
    let code = installed.code.expect("it carries a module");
    assert_eq!(code.about.admits.len(), 1, "the door this example is about");
    assert_eq!(code.about.admits[0].vct, "https://org.vaulet.id/acme/credential/employee-badge");
}

/// And a wallet that resolved somebody else's key refuses it, which is what
/// makes the check above mean anything.
#[test]
fn another_publishers_key_does_not_admit_it() {
    struct Stranger;
    impl HostPolicy for Stranger {
        fn owns_key(&self, _publisher: &str, _key: &[u8]) -> bool {
            false
        }
    }
    assert!(install_with(&read(PUBLISHED).expect("decodes"), &Stranger).is_err());
}
