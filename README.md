# rs-infra-style

[![Rust CI](https://github.com/qubit-ltd/rs-infra-style/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-infra-style/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-infra-style/coverage-badge.json)](https://qubit-ltd.github.io/rs-infra-style/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-infra-style.svg?color=blue)](https://crates.io/crates/qubit-infra-style)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Check and safely correct the first-generation fixed Rust style rules used by Qubit projects.

The authoritative Rust style standard shipped with this tool is in
[`doc/style-standard/`](doc/style-standard/). It is synchronized from the
`reviewing-rust-code-style` skill and includes the coding rules, Rust file
header template, and bilingual README structure. The checker is a mechanical
gate: it reports only violations that it can prove from the project files.
Semantic, performance, ownership, and test-quality judgments remain part of
the full skill review.

## Installation

```bash
cargo install --git https://github.com/qubit-ltd/rs-infra-style.git --tag v0.1.0 qubit-infra-style
```

## Quick Start

From a Rust project root:

```bash
cargo run --manifest-path /path/to/rs-infra-style/Cargo.toml -- --project . check
cargo run --manifest-path /path/to/rs-infra-style/Cargo.toml -- --project . fix
```

`check` combines rustfmt check with the fixed project style rules. `fix` runs
rustfmt and then validates the fixed rules. Migration-generated `style-check.sh`
and `align-ci.sh` wrappers can call these two stable subcommands directly.

The project's `.infra` configuration remains the source of truth; this tool does not copy project configuration into the tool repository.

Specific existing violations may be exempted in
`.infra/style/exceptions.toml`, using one exact project-relative path and one
supported rule per entry. Each exception requires a reason; source-code
`qubit-style: allow` comments are not supported.

```toml
format = 1

[[exceptions]]
rule = "test-redirect"
path = "tests/fixtures_tests.rs"
reason = "This test entry point must include the fixture shared by the external harness."
```

Supported rules are `inline-tests`, `test-file-name`, `test-redirect`,
`source-test-pair`, `explicit-imports`, `coverage-cfg`, `aggregation-files`,
`public-type-layout`, `multiple-public-types`, `type-file-name`, and
`internal-test-module`. Exceptions suppress only diagnostics matching
both the configured path and rule; malformed or unsupported entries fail the
check.

## Shared formatter configuration

Run the project's updater to install the configuration maintained at
[`conf/rustfmt.toml`](conf/rustfmt.toml) into `.infra/style/rustfmt.toml`:

```bash
./update-infra.sh --yes
```

Both `check` and `fix` read that project file and pass its absolute path to
rustfmt. A missing file fails with an update instruction. Select the required
toolchain separately when needed:

```bash
export RS_INFRA_STYLE_TOOLCHAIN=nightly-2026-06-05
rs-infra-style --project . check
rs-infra-style --project . fix
```

Install that toolchain with its rustfmt component before running the commands.
`RS_INFRA_STYLE_TOOLCHAIN` selects `cargo +<toolchain> fmt`; unset or empty
values retain Cargo's toolchain selection. Formatter failures propagate to the
command's exit status, and `fix --dry-run` displays the command without running
Cargo.

## Capabilities and limitations

This release preserves the mechanical legacy rules and the opt-in
`STYLE_ENFORCE_SOURCE_TEST_PAIRS` compatibility switch. Inline test placement
is not rejected automatically because the skill allows inline tests when
private access is necessary. Project-specific exceptions belong in
`.infra/style/exceptions.toml`; coverage exceptions additionally require a
source `qubit-style: allow coverage-cfg` comment. Orchestration such as
Clippy, coverage, and Cargo.lock updates remains the responsibility of
`rs-infra-ci`.

## Learn More

See the command help and source tests for the supported interface. Switch to [中文文档](README.zh_CN.md).

## Deterministic analysis

`analyze` is a read-only source analysis entry point independent of `check` and
`fix`. Supply a JSON array of project-relative Rust file paths; this exact list
can include non-default workspace members, custom Cargo targets, and standalone
fuzz crates. Associated sources and configuration are read as context, without
Cargo metadata, dependency resolution, builds, or macro expansion.

```sh
printf '["src/lib.rs", "tests/api_tests.rs"]' > /tmp/rust-source-list.json
rs-infra-style analyze --project . --files-from /tmp/rust-source-list.json \
  --standards /path/to/style-standard --format json
```

The versioned JSON report separates proved `error` diagnostics from informational
review signals. Every requested file has normative and machine-subcheck coverage:
`pass`, `fail`, `needs_semantic_review`, `not_applicable`, or `blocked`. A clean
mechanical scan still requires semantic review of documentation meaning, domain
boundaries, test placement, method ordering, and context-dependent attributes.
The existing `Diagnostic`, `check`, and `fix` interfaces retain their contract.

Exit status is `0` for a completed scan without proved violations, `1` for proved
violations, and `2` for incomplete analysis. Missing tooling, unreadable or
unparseable requested files, invalid configuration, and unsupported standard
snapshots never silently pass. Item macros and conditional declarations retain
explicit review coverage. Conventional test-support directories require an
explicit exception or semantic confirmation rather than an assumed exemption.

The report binds the analyzer version, all read source/configuration bytes, and
standard authority to SHA-256 digests. `--standards` must match the seven-document
snapshot embedded in this binary; a newer authority requires a matching analyzer
release. Project rustfmt is invoked through its configured installed toolchain,
using stdin and `skip_children=true`; it never rewrites source. Import order is
compared against rustfmt rather than simulated with a string sort. Detailed
coverage and limitations are documented in [the analyzer contract](doc/analyzer.md).



A higher-priority project copyright policy can be recorded in
`.infra/style/analysis.toml`; overrides require an explicit reason:

```toml
format = 1

[header]
owner = "Example Team"
minimum_year = 2020
reason = "This project uses its established copyright identity."
```

The exact seven-line template remains required. Accepted single years and ranges
beginning at `minimum_year` are preserved. Invalid or unexplained overrides block
analysis. Use `--repository-readme` to opt into separate repository README
structure findings; source-only scans continue to mark README-001 as outside
file scope. This option binds README presence and metadata to the input digest;
it does not replace bilingual semantic review.

## Testing

```bash
# Run tests with the default feature set
cargo test

# Run tests with all declared features
cargo test --all-features

# Project CI checks
./ci-check.sh

# Check code coverage
./coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public API documentation and tests current, and run `./align-ci.sh` to format code and `./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-infra-style](https://github.com/qubit-ltd/rs-infra-style)
