//! A Wasm back end.
//!
//! The same front end, a different tail. Nothing about the language changes:
//! this compiles the typed AST that `valang` produced, and the module it emits
//! computes what the tree-walking evaluator computes — which is the only test
//! of a second back end worth having.
//!
//! **Values stay host-side and Wasm passes `i32` handles.** Wasm core has four
//! numeric types and no allocator; the alternative is writing one in the module
//! and then owning its bugs forever. With handles the compiler emits calls and
//! control flow and nothing else (§8).

pub mod compile;
pub mod run;

pub use compile::{compile_function, konsts_of, Module};
pub use run::{run_function, run_with_fuel, Wasm};
