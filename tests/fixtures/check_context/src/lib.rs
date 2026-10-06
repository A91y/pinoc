// Typed-context instructions: accounts validated in `try_from`, used in `process`.
// Checks are written inline in some and through `guards` helpers in others.

pub mod guards;
pub mod instructions;
pub mod processor;
pub mod state;
