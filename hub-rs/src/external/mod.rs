//! Holds one module per external system the hub calls. No other folder
//! runs a program or opens a socket, so each system's calls and
//! failures are in one place. The one exception is `watch`, which
//! keeps its own calls, since no other command uses them.

pub mod cards;
pub mod git;
pub mod herdr;
pub mod opencode;
pub mod processes;
pub mod requests;
