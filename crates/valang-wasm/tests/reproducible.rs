//! Two builds of one source are one module.
//!
//! Everything about the way a Micro App is shipped rests on this. A wallet runs
//! bytes it did not compile; the only thing tying those bytes to the source
//! somebody published is that anybody can build the source and get the same
//! bytes back. A compiler that varied — in the order of a map, in a name it
//! generated, in anything at all — would make that check impossible, and nobody
//! would notice, because the comparison would simply always fail.

use valang::capability::{Host, Hosts};

fn registries() -> Hosts {
    Hosts::of(vec![Host::parse(include_str!("../../../hosts/core.json")).expect("core parses")])
}

fn build(src: &str) -> Vec<u8> {
    let (program, _) = valang::analyse_fully(src, None, &registries());
    valang_wasm::compile::compile_program(&program).expect("emits").bytes
}

#[test]
fn the_same_source_builds_the_same_bytes() {
    for (name, src) in [
        ("loyalty", include_str!("../../../examples/loyalty.val")),
        ("door", include_str!("../../../examples/door.val")),
        ("condo", include_str!("../../../examples/condo.val")),
        ("portfolio", include_str!("../../../examples/portfolio.val")),
        ("syntax", include_str!("../../../examples/syntax.val")),
    ] {
        assert_eq!(build(src), build(src), "{name} did not build the same twice");
    }
}

/// And a different source is a different module — otherwise the check above
/// would pass for a compiler that emitted nothing.
#[test]
fn a_changed_source_is_a_changed_module() {
    let src = include_str!("../../../examples/loyalty.val");
    let edited = src.replace("satangPerBaht = 100", "satangPerBaht = 50");
    assert_ne!(src, edited.as_str(), "the sample edit did not apply");
    assert_ne!(build(src), build(&edited), "an edited source built the same module");
}
