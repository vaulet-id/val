//! Who an application opens for.
//!
//! The expected values here were read off `examples/staff.val` by hand, and the
//! refusals were written from the specification's rule rather than from the
//! checker: a fixture written by the same reasoning as the code it tests is two
//! things agreeing with each other.

use valang::{analyse, Severity};

const STAFF: &str = include_str!("../../../examples/staff.val");

fn errors(src: &str) -> Vec<String> {
    analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message)
        .collect()
}

/// The line, as it is written and as it parses.
#[test]
fn a_gate_is_a_credential_a_policy_and_what_the_person_is_told() {
    let (p, _) = analyse(STAFF);
    assert_eq!(p.admits.len(), 1);
    assert_eq!(p.admits[0].credential, "EmployeeBadge");
    assert_eq!(p.admits[0].policy, "EmployedByAcme");
    assert_eq!(p.admits[0].phrase, "notStaff");
    assert!(errors(STAFF).is_empty(), "{:?}", errors(STAFF));
}

/// A policy over the wrong credential is the case that fails quietly: the
/// signature on something else is checked, and it passes. A door that opens for
/// the wrong badge is worse than one that does not open.
#[test]
fn a_policy_over_another_credential_is_refused() {
    let src = r#"
app "x"
version 1
capabilities { credential.check(Badge) }
credential Badge as "https://org.vaulet.id/example/credential/badge" { id: string }
credential Ticket as "https://org.vaulet.id/example/credential/ticket" { id: string }
trust IssuedByUs(t: Ticket) {
  anchor: "x"
  require { t.signature.valid }
}
admits { Badge with IssuedByUs else "no" }
"#;
    let e = errors(src);
    assert!(
        e.iter().any(|m| m.contains("policy over `Ticket`") && m.contains("`Badge`")),
        "{e:?}"
    );
}

#[test]
fn a_gate_over_a_credential_nothing_declares_is_refused() {
    let src = r#"
app "x"
version 1
capabilities { credential.check(Badge) }
admits { Badge with Whatever else "no" }
"#;
    assert!(errors(src).iter().any(|m| m.contains("not a credential this package declares")), "{:?}", errors(src));
}

/// Both halves are required, and neither has a default worth having: a gate
/// with no policy admits anything of the right shape, and one with no sentence
/// closes a door in silence.
#[test]
fn a_gate_without_a_policy_or_without_words_is_refused() {
    let bare = r#"
app "x"
version 1
credential Badge as "https://org.vaulet.id/example/credential/badge" { id: string }
admits { Badge }
"#;
    assert!(errors(bare).iter().any(|m| m.contains("checked against a policy")), "{:?}", errors(bare));

    let silent = r#"
app "x"
version 1
credential Badge as "https://org.vaulet.id/example/credential/badge" { id: string }
trust Ours(b: Badge) { anchor: "x" require { b.signature.valid } }
admits { Badge with Ours }
"#;
    assert!(
        errors(silent).iter().any(|m| m.contains("else")),
        "{:?}",
        errors(silent)
    );
}

/// One door, one list. Two blocks are two answers to the question a person is
/// asking.
#[test]
fn a_package_says_who_it_opens_for_once() {
    let src = r#"
app "x"
version 1
credential Badge as "https://org.vaulet.id/example/credential/badge" { id: string }
trust Ours(b: Badge) { anchor: "x" require { b.signature.valid } }
admits { Badge with Ours else "no" }
admits { Badge with Ours else "no" }
"#;
    assert!(errors(src).iter().any(|m| m.contains("a door has one list")), "{:?}", errors(src));
}

/// A gate looks at a credential, so it is a use of the capability that says so —
/// and `credential.check` is not `credential.read`: the application is told the
/// door opened and never what opened it.
#[test]
fn a_gate_uses_the_check_capability_rather_than_the_read_one() {
    let undeclared = r#"
app "x"
version 1
capabilities { }
credential Badge as "https://org.vaulet.id/example/credential/badge" { id: string }
trust Ours(b: Badge) { anchor: "x" require { b.signature.valid } }
admits { Badge with Ours else "no" }
"#;
    let e = errors(undeclared);
    assert!(e.iter().any(|m| m.contains("credential.check")), "{e:?}");

    let read_instead = r#"
app "x"
version 1
capabilities { credential.read(Badge) }
credential Badge as "https://org.vaulet.id/example/credential/badge" { id: string }
trust Ours(b: Badge) { anchor: "x" require { b.signature.valid } }
admits { Badge with Ours else "no" }
"#;
    let e = errors(read_instead);
    assert!(e.iter().any(|m| m.contains("credential.check")), "{e:?}");
}
