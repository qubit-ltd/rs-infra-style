# rs-infra-style

[![Rust CI](https://github.com/qubit-ltd/rs-infra-style/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-infra-style/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-infra-style/coverage-badge.json)](https://qubit-ltd.github.io/rs-infra-style/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-infra-style.svg?color=blue)](https://crates.io/crates/qubit-infra-style)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Check and safely correct the first-generation fixed Rust style rules used by Qubit projects.

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

## Pinning the formatter

Migration wrappers can preserve a project's rustfmt contract for both `check`
and `fix`:

```bash
export RS_INFRA_STYLE_TOOLCHAIN=nightly-2026-06-05
export RS_INFRA_STYLE_RUSTFMT_CONFIG="$PWD/.infra/style/rustfmt.toml"
rs-infra-style --project . check
rs-infra-style --project . fix
```

Install that toolchain with its rustfmt component before running the commands.
`RS_INFRA_STYLE_TOOLCHAIN` selects `cargo +<toolchain> fmt`; the configuration
variable adds `--config-path` to rustfmt's arguments. Either variable can be
used independently. Unset or empty variables preserve the existing Cargo
toolchain selection and rustfmt configuration discovery, respectively.
Relative configuration paths are resolved from `--project`; paths containing
spaces are supported. To keep a project's root `rustfmt.toml`, point the
configuration variable at that file instead. The tool does not copy or replace
configuration files. Formatter failures propagate to the command's exit status,
and `fix --dry-run` displays the configured command without running it.

## Capabilities and limitations

This release preserves the legacy `rs-ci/style-check.sh` rules, including the
opt-in environment switches `STYLE_ENFORCE_INLINE_TESTS` and
`STYLE_ENFORCE_SOURCE_TEST_PAIRS`. Project-specific exceptions belong in
`.infra/style/exceptions.toml`; orchestration such as Clippy, coverage, and
Cargo.lock updates remains the responsibility of `rs-infra-ci`.

## Learn More

See the command help and source tests for the supported interface. Switch to [中文文档](README.zh_CN.md).

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
