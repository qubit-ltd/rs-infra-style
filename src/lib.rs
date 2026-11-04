// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fixed first-phase Rust style checks used by Qubit Rust projects.

pub mod analysis;
mod analysis_pairs;
mod analysis_project;
mod analysis_readme;
mod analysis_rules;
mod check_discovery;
mod check_failure;
mod check_projection;
mod diagnostic;
mod style_checker;
mod style_exceptions;
mod style_formatter;

pub use check_failure::CheckFailure;
pub use diagnostic::Diagnostic;
pub use style_checker::check;
pub use style_checker::check_project;
pub use style_checker::fix;
pub use style_checker::fix_project;
pub use style_checker::print_diagnostics;
