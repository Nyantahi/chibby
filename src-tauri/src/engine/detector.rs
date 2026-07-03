//! Repository detection and pipeline generation.
//!
//! Detect common scripts, build files, and CI/CD configs in a repository
//! and generate a draft pipeline that the user can review and edit.
//!
//! This module is split into focused submodules; every public item is
//! re-exported here so `crate::engine::detector::*` paths remain stable.

mod deploy_gen;
mod deployment;
mod draft;
mod folders;
mod scripts;
mod validation;
mod workflows;

pub use deploy_gen::*;
pub use deployment::*;
pub use draft::*;
pub use folders::*;
pub use scripts::*;
pub use validation::*;
pub use workflows::*;
