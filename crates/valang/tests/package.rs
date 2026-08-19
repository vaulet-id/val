//! A package is several files, and the tool that checks one checks them
//! together.
//!
//! `wallet.val` presses an action `loyalty.val` declares. Checked apart, the
//! screen's press names an action that does not exist and the capabilities look
//! unused — both true of the file, neither true of the package.

const LOYALTY: &str = include_str!("../../../examples/loyalty.val");
const WALLET: &str = include_str!("../../../examples/wallet.val");

fn errors(src: &str) -> Vec<String> {
    let (_, d) = valang::analyse(src);
    d.iter()
        .filter(|x| x.severity == valang::diag::Severity::Error)
        .map(|x| x.message.clone())
        .collect()
}

#[test]
fn the_two_files_of_one_package_check_together() {
    let joined = format!("{LOYALTY}\n{WALLET}");
    assert!(errors(&joined).is_empty(), "{:?}", errors(&joined));
}

/// Held apart they fail, and the message is about the half that is missing —
/// which is what `valc` printed for a whole package until it joined them.
#[test]
fn the_screen_alone_is_half_a_program() {
    let msgs = errors(WALLET);
    assert!(
        msgs.iter().any(|m| m.contains("neither an action nor a screen")),
        "got {msgs:?}"
    );
}
