// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private project analysis collectors.

pub(crate) mod complexity;
pub(crate) mod declaration;
pub(crate) mod index;
pub(crate) mod method_inventory;

/// Collects imports for comparison against project rustfmt output.
pub(crate) mod import_inventory;
