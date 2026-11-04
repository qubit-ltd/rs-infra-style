// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private traversal and literal evidence used by local syntax analysis.

// Defines the source-local diagnostic visitor and its scope state.
pub(super) mod inspector;
// Tracks literal token ranges to distinguish comments from string contents.
pub(super) mod literal_lines;
