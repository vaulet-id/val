# Fuzzing

Two targets, for a machine with `cargo fuzz`:

```
cargo +nightly fuzz run parse
cargo +nightly fuzz run round_trip
```

`parse` requires the front end to answer rather than fall over. `round_trip`
requires that whatever parses cleanly prints and reparses to the same text.

**The same two properties are checked in `cargo test`**, over the corpus rather
than over arbitrary bytes: `crates/valang/tests/fuzz.rs` damages each example
the ways a file arrives damaged — a character changed, one lost, one added, a
run repeated, the end cut off — ten thousand times from a fixed seed. That runs
everywhere and needs no toolchain, and it is what found `data { x: query }`
printing to something that parsed back differently.

Use both. The seeded one is the regression net; this one goes looking.
