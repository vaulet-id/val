//! Where the cursor is, answered by the parser.
//!
//! An editor asking "what may I write here" was reading braces and indentation,
//! which is a guess about the grammar made by something that does not have one.
//! The parser knows what it opened.

use valang::ast::ScopeKind;

/// The blocks around a position, outermost first.
fn at(src: &str, line: u32, col: u32) -> Vec<(ScopeKind, String)> {
    let (program, _) = valang::parse::parse(src);
    program
        .scopes
        .iter()
        .filter(|s| {
            let after = (line, col) >= (s.from.line, s.from.col);
            let before = (line, col) <= (s.to.line, s.to.col);
            after && before
        })
        .map(|s| (s.kind, s.name.clone()))
        .collect()
}

const SRC: &str = r#"app "x.y"
version 1

capabilities {
  credential.read(R)
}

credential R {
  amount: int
}

state {
  n: int default 0
}

action Go {
  compute {
    const a = 1
  }

  update {
    n: a
  }
}

@main
screen Home {
  data {
    rows: credentials of R limit 2
  }

  column {
    button("go") {
      onTap: Go
    }
  }
}
"#;

#[test]
fn the_parser_says_which_block_a_position_is_in() {
    assert_eq!(at(SRC, 5, 3).last().map(|x| x.0), Some(ScopeKind::Capabilities));
    assert_eq!(at(SRC, 9, 3).last().map(|x| x.0), Some(ScopeKind::Fields));
    assert_eq!(at(SRC, 18, 5).last(), Some(&(ScopeKind::Phase, "compute".to_string())));
    assert_eq!(at(SRC, 29, 5).last().map(|x| x.0), Some(ScopeKind::ScreenData));
}

/// The innermost is what an editor offers from, and the ones around it are the
/// context: a prop inside a button inside a column inside a screen.
///
/// A screen's own tree is the screen's block and not a `Tree` of its own —
/// `Tree` is the shape that holds children and no props, which is what an
/// `if`/`else` branch and a component's body are.
#[test]
fn the_blocks_around_a_position_are_in_order() {
    let path: Vec<String> = at(SRC, 34, 7)
        .iter()
        .map(|(k, n)| if n.is_empty() { format!("{k:?}") } else { format!("{k:?}({n})") })
        .collect();
    assert_eq!(
        path,
        vec!["Screen(Home)", "Node(column)", "Node(button)"],
        "the path to a prop is not what was written"
    );
}

/// The point of it. A block nobody closed runs to the end of the file, which is
/// what every program looks like while somebody is typing it.
#[test]
fn a_block_left_open_still_says_where_the_cursor_is() {
    let half = "app \"x.y\"\nversion 1\n\n@main\nscreen Home {\n  column {\n    ";
    let path: Vec<ScopeKind> = at(half, 7, 5).iter().map(|(k, _)| *k).collect();
    assert!(
        path.contains(&ScopeKind::Screen) && path.contains(&ScopeKind::Node),
        "a half-written screen said {path:?}"
    );
}

/// And a phase, which is the block an editor offers effects in.
#[test]
fn a_phase_names_itself() {
    let half = "app \"x.y\"\nversion 1\n\naction Go {\n  execute {\n    ";
    let names: Vec<String> = at(half, 6, 5).iter().map(|(_, n)| n.clone()).collect();
    assert!(names.contains(&"execute".to_string()), "{names:?}");
    assert!(names.contains(&"Go".to_string()), "{names:?}");
}

/// The same fact the editor uses is an error for the compiler: a file that ends
/// inside a block is not a program, and it said so nowhere before.
#[test]
fn a_block_the_file_ends_inside_is_reported() {
    let (_, d) = valang::parse::parse("app \"x.y\"\nversion 1\n\naction Go {\n  execute {\n");
    let said: Vec<&str> = d.iter().map(|x| x.message.as_str()).collect();
    assert_eq!(
        said.iter().filter(|m| m.contains("never closed")).count(),
        2,
        "one for the action and one for the phase, and got {said:?}"
    );
}
