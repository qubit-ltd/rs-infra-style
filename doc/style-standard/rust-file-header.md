# Rust File Header Constant

Use this seven-line copyright and license header template when a Rust source
file has no copyright or license header block. Replace `{current_year}` with
the current four-digit calendar year when adding the header:

```rust
// =============================================================================
//    Copyright (c) 2025 - {current_year} Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
```

Classify an existing file header before changing it.

**Complete existing header.** Accept a complete seven-line header when its other
six lines match the template exactly and its copyright line is either of the
following. Preserve its year or year range when every shown `{year}` is a
four-digit year greater than or equal to 2025:

```rust
//    Copyright (c) 2025 - {year} Haixing Hu.
//    Copyright (c) {year} Haixing Hu.
```

The two lines above are alternatives, not consecutive header lines. Preserve an
accepted existing copyright line exactly; do not update it to `{current_year}`.

**No header block.** Only when the file has no copyright or license header block,
add the complete template and resolve it to `2025 - {current_year}` at that time.

**Incomplete or nonconforming header.** Treat an existing incomplete or
nonconforming copyright or license header as a violation, not as a missing
header. In correction mode, repair that block in place rather than prepend a
second block. Preserve an accepted existing copyright line while repairing other
lines. Do not invent a replacement year for a nonconforming copyright line
unless a higher-priority task or repository rule supplies one; otherwise report
it as unresolved.

Never leave either placeholder in a Rust source file. Preserve every line of an
accepted complete existing header exactly. Make every added or repaired block
match the template exactly, except that an accepted existing copyright line
keeps its original year or year range. A higher-priority task or repository rule
may override the owner or year range; neighboring files do not override this
constant.
