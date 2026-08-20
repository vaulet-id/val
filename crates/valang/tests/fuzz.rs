//! Mutation fuzzing, in the test suite.
//!
//! `cargo fuzz` needs a nightly toolchain and libFuzzer, and a fuzz target
//! nobody can run is a fuzz target nobody runs. This is the part that fits in
//! `cargo test`: take the corpus, damage it in the ways a file gets damaged,
//! and require the front end to answer rather than fall over.
//!
//! Seeded, so a failure is a number somebody can put back in and reproduce.

const CORPUS: &[&str] = &[
    include_str!("../../../examples/loyalty.val"),
    include_str!("../../../examples/wallet.val"),
    include_str!("../../../examples/door.val"),
    include_str!("../../../examples/portfolio.val"),
    include_str!("../../../examples/catalogue.val"),
    include_str!("../../../examples/syntax.val"),
    include_str!("../../../examples/kit.val"),
    include_str!("../../../examples/rejected.val"),
];

/// xorshift64*. Small, deterministic, and nobody's cryptography.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

/// The ways a file arrives damaged: a character changed, one lost, one added,
/// a run repeated, the end cut off.
fn damage(src: &str, rng: &mut Rng) -> String {
    let mut bytes: Vec<u8> = src.bytes().collect();
    if bytes.is_empty() {
        return String::new();
    }
    const INTERESTING: &[u8] = b"{}()[]<>\"`'\\.,:;=->?!|&+*/%@$\n\t 0";

    match rng.below(6) {
        0 => {
            let i = rng.below(bytes.len());
            bytes[i] = INTERESTING[rng.below(INTERESTING.len())];
        }
        1 => {
            let i = rng.below(bytes.len());
            bytes.remove(i);
        }
        2 => {
            let i = rng.below(bytes.len());
            bytes.insert(i, INTERESTING[rng.below(INTERESTING.len())]);
        }
        3 => {
            let len = bytes.len();
            let a = rng.below(len);
            let b = (a + 1 + rng.below(64)).min(len);
            let run: Vec<u8> = bytes[a..b].to_vec();
            let at = rng.below(len);
            for (k, byte) in run.into_iter().enumerate() {
                bytes.insert((at + k).min(bytes.len()), byte);
            }
        }
        4 => {
            let cut = rng.below(bytes.len());
            bytes.truncate(cut);
        }
        _ => {
            let i = rng.below(bytes.len());
            bytes[i] = rng.next() as u8;
        }
    }

    String::from_utf8_lossy(&bytes).into_owned()
}

fn hosts() -> valang::capability::Hosts {
    valang::capability::Hosts::of(vec![valang::capability::Host::parse(include_str!(
        "../../../hosts/core.json"
    ))
    .expect("the core registry parses")])
}

/// Ten thousand damaged files, and the front end answers every one.
#[test]
fn damaged_input_is_answered_not_survived() {
    let hosts = hosts();
    let mut rng = Rng(0x5EED_1234_5678_9ABC);

    for round in 0..10_000u32 {
        let base = CORPUS[rng.below(CORPUS.len())];
        let mut src = base.to_string();
        for _ in 0..1 + rng.below(4) {
            src = damage(&src, &mut rng);
        }
        // The seed and the round are what a failure is reported with: both are
        // in this line, so a crash is reproducible from the panic message.
        let _ = std::panic::catch_unwind(|| {
            let (_, d) = valang::analyse_fully(&src, None, &hosts);
            d.len()
        })
        .unwrap_or_else(|_| panic!("round {round} fell over. Seed 0x5EED123456789ABC"));
    }
}

/// And whatever survives the damage still prints and reparses to itself. A
/// parser that accepts something the printer cannot write is the disagreement
/// this catches, on inputs nobody would have thought to write.
#[test]
fn damaged_input_that_parses_still_round_trips() {
    let mut rng = Rng(0x0FF1_CE00_1234_5678);

    for round in 0..2_000u32 {
        let base = CORPUS[rng.below(CORPUS.len())];
        let src = damage(base, &mut rng);

        let (program, d) = valang::parse::parse(&src);
        if d.iter().any(|x| x.severity == valang::Severity::Error) {
            continue;
        }
        let once = valang::print::print(&program);
        let twice = valang::print::print(&valang::parse::parse(&once).0);
        assert_eq!(once, twice, "round {round}: printing was not stable");
    }
}
