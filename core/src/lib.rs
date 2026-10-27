//! Swimlie core: all data handling and calculations. Swift only does UI and Apple APIs.

#![allow(
    clippy::needless_pass_by_value,
    reason = "UniFFI exports take owned arguments"
)]
#![cfg_attr(
    test,
    allow(
        clippy::excessive_nesting,
        clippy::many_single_char_names,
        clippy::too_many_lines,
        clippy::too_many_arguments,
        reason = "tests read top to bottom as arrange, act, assert"
    )
)]

pub mod ai;
pub mod ai_models;
pub mod coach;
pub mod engine;
pub mod exercises;
mod i18n;
pub mod metrics;
pub mod model;
pub mod review;
pub mod store;

uniffi::setup_scaffolding!();
