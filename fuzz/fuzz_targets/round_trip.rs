//! Whatever parses, prints and reparses to the same text.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else { return };
    let (program, d) = valang::parse::parse(src);
    if d.iter().any(|x| x.severity == valang::Severity::Error) {
        return;
    }
    let once = valang::print::print(&program);
    let twice = valang::print::print(&valang::parse::parse(&once).0);
    assert_eq!(once, twice, "printing was not stable");
});
