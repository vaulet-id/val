//! A package this deployment published, read the way a wallet reads one.
//!
//! **Written by the other half.** The bytes come from the server compiling and
//! signing `examples/staff.val` with an organisation key; nothing here builds
//! them, so what is checked is that the two sides agree rather than that one of
//! them is self-consistent.

use valang_package::*;

/// Kept as bytes rather than rebuilt, because rebuilding it here would be this
/// crate checking its own arithmetic. `GET /acme/app/th.co.acme.staff` produced
/// these, from a backend that compiled `examples/staff.val` and signed it as
/// the organisation.
const PUBLISHED: &[u8] = include_bytes!("../../../fixtures/published-by-acme.vapp");

/// The wallet resolved this publisher's document and found this key.
struct Wallet(Vec<u8>);

impl HostPolicy for Wallet {
    fn owns_key(&self, _publisher: &str, key: &[u8]) -> bool {
        self.0 == key
    }
}

#[test]
fn a_published_package_is_one_a_wallet_admits() {
    let pkg = read(PUBLISHED).expect("it decodes");
    assert_eq!(pkg.manifest.app, "th.co.acme.staff");
    assert_eq!(pkg.manifest.publisher, "did:web:org.vaulet.id:acme");

    // P-256, because an organisation's issuing key is, and the package carries
    // the point rather than saying which curve it is.
    let key = pkg.public_key.clone().expect("it is signed");
    assert_eq!(key.len(), 65, "an uncompressed P-256 point");
    assert_eq!(key[0], 0x04);

    let installed = install_with(&pkg, &Wallet(key)).expect("this wallet admits it");
    let code = installed.code.expect("it carries a module");
    assert_eq!(code.about.admits.len(), 1, "the door this example is about");
    assert_eq!(code.about.admits[0].vct, "https://org.vaulet.id/acme/credential/employee-badge");
}

/// And a wallet that resolved somebody else's key refuses it, which is what
/// makes the check above mean anything.
#[test]
fn another_publishers_key_does_not_admit_it() {
    let pkg = read(PUBLISHED).expect("it decodes");
    assert!(install_with(&pkg, &Wallet(vec![0x04; 65])).is_err());
}
