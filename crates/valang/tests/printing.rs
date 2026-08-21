//! The printer, in the places a printer forgets.

use valang::print::print;

fn printed(src: &str) -> String {
    print(&valang::parse::parse(src).0)
}

fn stable(name: &str, src: &str) {
    let once = printed(src);
    let twice = printed(&once);
    assert_eq!(once, twice, "{name}: printing was not stable\n--- once\n{once}\n--- twice\n{twice}");
}

/// Comments in the places a printer forgets: the end of a block, between
/// `else` and its brace, inside a block with nothing in it, and the last line
/// of the file.
#[test]
fn comments_in_the_awkward_places_survive() {
    let src = r#"
app "x.y"
version 1

capabilities {
  // nothing yet
}

state {
  n: int default 0
}

action Go {
  compute {
    const a = 1
    // after the last statement
  }

  update {
    n: a
  }
}

@main
screen Home {
  column {
    if (state.n > 0) {
      card("yes")
      // inside the branch
    } else {
      // the other branch
      card("no")
    }
  }
}
// the last line of the file
"#;
    let once = printed(src);
    for want in [
        "// nothing yet",
        "// after the last statement",
        "// inside the branch",
        "// the other branch",
        "// the last line of the file",
    ] {
        assert!(once.contains(want), "{want:?} was dropped:\n{once}");
    }
    stable("awkward comments", src);
}

/// A file that is only comments, and one that is empty.
#[test]
fn a_file_with_nothing_in_it_prints() {
    stable("only comments", "// one\n// two\n");
    stable("empty", "");
    stable("blank lines", "\n\n\n");
    assert!(printed("// one\n// two\n").contains("// two"));
}

/// A combinator inside another one's body.
#[test]
fn a_combinator_inside_a_combinator() {
    let src = r#"
app "x.y"
version 1

capabilities {
}

state {
  n: int default 0
}

function inner(xs: List<int>): int {
  return xs.fold(0) { sum, r -> sum + r }
}

function outer(rows: List<int>): List<int> {
  return rows.map { r -> [r, r].fold(0) { sum, x -> sum + x } }
}

@main
screen Home {
  column {
    section("x")
  }
}
"#;
    let errors: Vec<String> = valang::analyse(src)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    stable("nested combinators", src);
}

/// A component that loops, exported and then imported.
#[test]
fn an_exported_component_may_loop() {
    let kit = r#"
app "org.kit"
version 1

capabilities {
}

export component Rows(labels: List<string>) {
  column {
    for (l in labels) {
      text(l)
    }
  }
}
"#;
    let app = r#"
app "org.app"
version 1

capabilities {
}

import "org.kit/1" { Rows }

@main
screen Home {
  column {
    Rows(labels: ["a", "b"])
  }
}
"#;
    let hosts = valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .unwrap()]);
    let packages =
        valang::expand::Packages::of(vec![valang::parse::parse(kit).0]);
    let errors: Vec<String> = valang::analyse_with_packages(app, None, &hosts, &packages)
        .1
        .into_iter()
        .filter(|d| d.severity == valang::diag::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(errors.is_empty(), "a component that loops could not be imported: {errors:?}");
}

/// Importing from a package that is an application in its own right — one
/// with screens, state and a `@main` of its own.
#[test]
fn importing_from_a_package_that_is_an_application() {
    let other = r#"
app "org.other"
version 1

capabilities {
}

state {
  points: int default 0
}

export component Chip(label: string) {
  section(label)
}

@main
screen Theirs {
  column {
    section("theirs")
  }
}
"#;
    let app = r#"
app "org.app"
version 1

capabilities {
}

import "org.other/1" { Chip }

@main
screen Home {
  column {
    Chip(label: "x")
  }
}
"#;
    let hosts = valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .unwrap()]);
    let packages = valang::expand::Packages::of(vec![valang::parse::parse(other).0]);
    let (program, d) = valang::analyse_with_packages(app, None, &hosts, &packages);
    let errors: Vec<String> = d
        .into_iter()
        .filter(|x| x.severity == valang::diag::Severity::Error)
        .map(|x| x.message)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    // And nothing of theirs came with it.
    assert_eq!(program.screens.len(), 1, "a screen came across the boundary");
    assert!(program.state.is_empty(), "state came across the boundary");
}

/// The formatter on disk: writing what it printed and reading it back has to
/// give the same bytes, which is a different question from the printer agreeing
/// with itself in memory.
#[test]
fn formatting_twice_on_disk_converges() {
    // Every example, printed and printed again through the same path the CLI
    // takes: read, parse, print, write.
    for src in [
        include_str!("../../../examples/loyalty.val"),
        include_str!("../../../examples/catalogue.val"),
        include_str!("../../../examples/portfolio.val"),
        include_str!("../../../examples/note.val"),
        include_str!("../../../examples/condo.val"),
        include_str!("../../../examples/transit.val"),
        include_str!("../../../examples/referendum.val"),
        include_str!("../../../examples/storefront.val"),
    ] {
        let once = printed(src);
        let twice = printed(&once);
        let thrice = printed(&twice);
        assert_eq!(once, twice);
        assert_eq!(twice, thrice);
    }
}

/// A `present` block, which holds statements that are written without
/// parentheses and nested inside an effect.
#[test]
fn a_present_block_round_trips() {
    let src = r#"
app "x.y"
version 1

capabilities {
  credential.read(Id)
  disclosure.present
}

credential Id as "https://org.vaulet.id/example/credential/id" {
  country:   string
  birthdate: date
}

trust Issued(id: Id) {
  anchor: "th.go.dopa"
  require {
    id.signature.valid
  }
}

state {
  n: int default 0
}

action Prove {
  input {
    id: Credential<Id>
  }

  verify {
    const checked = id with Issued
  }

  update {
    n: 1
  }

  execute {
    present {
      disclose checked.claims.country
      prove checked.claims.birthdate <= context.time.now - duration(years: 20)
    }
  }
}

@main
screen Home {
  column {
    button("go") { onTap: Prove }
  }
}
"#;
    let once = printed(src);
    assert!(once.contains("disclose checked.claims.country"), "{once}");
    assert!(once.contains("prove "), "{once}");
    stable("present", src);

    let hosts = valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .unwrap()]);
    let before = valang::report::report(&valang::analyse_fully(src, None, &hosts).0).to_string();
    let after = valang::report::report(&valang::analyse_fully(&once, None, &hosts).0).to_string();
    assert_eq!(before, after, "printing a present block changed what it discloses");
}
