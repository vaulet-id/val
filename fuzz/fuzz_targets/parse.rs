//! The front end answers, whatever the bytes are.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else { return };
    let hosts = valang::capability::Hosts::of(
        valang::capability::Host::parse(include_str!("../../hosts/core.json")).into_iter().collect(),
    );
    let (_, d) = valang::analyse_fully(src, None, &hosts);
    let _ = d.len();
});
