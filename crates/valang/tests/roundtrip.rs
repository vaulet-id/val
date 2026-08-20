//! Print, parse, print — and the two texts have to match.
//!
//! The strongest test a parser has, and the cheapest once a printer exists: a
//! production the parser reads but the printer cannot write is a production one
//! of them does not really have, and a shape the printer writes but the parser
//! reads differently shows up as two different texts.

use valang::print::print;

const SOURCES: &[(&str, &str)] = &[
    ("loyalty", include_str!("../../../examples/loyalty.val")),
    ("wallet", include_str!("../../../examples/wallet.val")),
    ("door", include_str!("../../../examples/door.val")),
    ("portfolio", include_str!("../../../examples/portfolio.val")),
    ("catalogue", include_str!("../../../examples/catalogue.val")),
    ("note", include_str!("../../../examples/note.val")),
    ("referendum", include_str!("../../../examples/referendum.val")),
    ("condo", include_str!("../../../examples/condo.val")),
    ("transit", include_str!("../../../examples/transit.val")),
    ("syntax", include_str!("../../../examples/syntax.val")),
    ("kit", include_str!("../../../examples/kit.val")),
    ("storefront", include_str!("../../../examples/storefront.val")),
];

fn printed(src: &str) -> String {
    print(&valang::parse::parse(src).0)
}

#[test]
fn printing_is_the_same_the_second_time() {
    for (name, src) in SOURCES {
        let once = printed(src);
        let twice = printed(&once);
        if once != twice {
            let (a, b) = once
                .lines()
                .zip(twice.lines())
                .enumerate()
                .find(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| (format!("{}: {a}", i + 1), format!("{}: {b}", i + 1)))
                .unwrap_or_else(|| (format!("{} lines", once.lines().count()), format!("{} lines", twice.lines().count())));
            panic!("{name} printed differently the second time\n  once  {a}\n  twice {b}");
        }
    }
}

/// And what it printed still parses without new complaints.
#[test]
fn what_is_printed_still_compiles() {
    for (name, src) in SOURCES {
        let before = valang::parse::parse(src)
            .1
            .into_iter()
            .filter(|d| d.severity == valang::Severity::Error)
            .count();
        let after = valang::parse::parse(&printed(src))
            .1
            .into_iter()
            .filter(|d| d.severity == valang::Severity::Error)
            .map(|d| d.message)
            .collect::<Vec<_>>();
        assert!(
            after.len() <= before,
            "{name}: printing it introduced {} complaint(s): {after:?}",
            after.len()
        );
    }
}

/// Idempotence says the printer agrees with itself. This says it did not change
/// the program: the capability report is derived from the whole of it — what it
/// reads, discloses, proves, issues, talks to, moves and writes — so a node the
/// printer dropped shows up as a line that went missing.
#[test]
fn printing_does_not_change_what_the_program_does() {
    let hosts = valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")]);

    for (name, src) in SOURCES {
        let (before, _) = valang::analyse_fully(src, None, &hosts);
        let printed = print(&valang::parse::parse(src).0);
        let (after, _) = valang::analyse_fully(&printed, None, &hosts);

        let a = valang::report::report(&before).to_string();
        let b = valang::report::report(&after).to_string();
        assert_eq!(a, b, "{name}: printing it changed what the report says");
    }
}

/// And the errors are the same errors. A printer that quietly repaired
/// something would be a formatter nobody could trust to leave a mistake alone.
#[test]
fn printing_neither_fixes_nor_breaks_anything() {
    let hosts = valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")]);

    for (name, src) in SOURCES {
        let said = |s: &str| {
            valang::analyse_fully(s, None, &hosts)
                .1
                .into_iter()
                .filter(|d| d.severity == valang::Severity::Error)
                .map(|d| d.message)
                .collect::<Vec<_>>()
        };
        let printed = print(&valang::parse::parse(src).0);
        assert_eq!(said(src), said(&printed), "{name}");
    }
}
