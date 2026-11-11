//! Holds one module per outside system the hub calls. No other folder
//! runs a program or opens a socket, so each system's calls and
//! failures are in one place.

pub mod cards;
pub mod git;
pub mod herdr;
pub mod opencode;
pub mod requests;
