//! Lists the four folders as modules, so the tests in `tests/` can
//! call each command directly instead of through the binary.

pub mod board;
pub mod commands;
pub mod external;
pub mod shared;
