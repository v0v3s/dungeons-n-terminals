//! End-to-end tests of the terminal app: the scenarios of
//! `e2e/scenarios.toml`, each run in a pseudo-terminal from a known state
//! (see `seed.rs`). `cargo test -p dnt --test e2e`.

// Helpers some scenarios use and others do not.
#[allow(dead_code)]
mod harness;
#[allow(dead_code)]
mod seed;

mod campaigns;
mod characters;
mod compendium;
mod concentration;
mod data;
mod homebrew;
mod hp;
mod nav;
mod notes;
mod overview;
mod rest;
mod rolls;
mod setup;
#[allow(dead_code)]
mod sheet_play;
mod sources;
