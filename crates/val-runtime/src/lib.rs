//! The VAL back end.
//!
//! Walks a typed AST and returns `(new state, output, effects)`. It never
//! performs an effect: it describes one and hands it to the host, which is the
//! only reason an execution record can be trusted.
//!
//! Empty. See `README.md` for what goes here and in what order.
