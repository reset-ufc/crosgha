// Here and not in Cargo.toml: the style guide allows panics in tests, which are separate crates.
#![warn(clippy::unwrap_used)]

pub mod cli;

mod config;

mod engine;
mod filesystem;
mod fix;
mod github;
mod hook;
mod migrate;
mod policy;
mod report;
mod rules;
mod workflow;
