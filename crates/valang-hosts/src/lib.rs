//! The registries this project publishes.
//!
//! **The language does not have a favourite host** — `valang` is handed
//! registries and takes no view, because a second host implements VAL and the
//! front end must not have been written around the first one. But the documents
//! themselves have to live somewhere, and every build that compiles a package
//! for Vaulet needs the same two.
//!
//! They lived in two `include_str!` pairs, one per binary, which was fine until
//! a third consumer appeared that cannot reach the files at all: a server
//! depending on this repository over git has no `../../../hosts`. So they are
//! here, once, and the binaries read them from here too.

/// What every host provides: the components, and the capabilities that are not
/// about one product.
pub const CORE: &str = include_str!("../../../hosts/core.json");

/// What this wallet adds — the card, the avatar, the gesture it draws.
pub const VAULET: &str = include_str!("../../../hosts/vaulet.json");

/// Both, parsed, in the order a Vaulet build compiles against.
///
/// Panics if either document is malformed, which is a build that shipped a
/// broken registry rather than a caller that did anything wrong.
pub fn vaulet() -> valang::capability::Hosts {
    valang::capability::Hosts::of(vec![
        valang::capability::Host::parse(CORE).expect("the core registry parses"),
        valang::capability::Host::parse(VAULET).expect("the vaulet registry parses"),
    ])
}

/// The core alone, for a build that draws nothing product-specific.
pub fn core() -> valang::capability::Hosts {
    valang::capability::Hosts::of(vec![
        valang::capability::Host::parse(CORE).expect("the core registry parses")
    ])
}

#[cfg(test)]
mod tests {
    /// The documents are documents, and a build that shipped a broken one would
    /// fail at the first package rather than here.
    #[test]
    fn both_registries_parse() {
        assert!(super::vaulet().words().contains("credential.check"));
    }
}
