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

Projects may define reviewed, path-scoped exceptions in
`.infra/style/exceptions.toml`:

```toml
format = 1

[[exceptions]]
rule = "test-redirect"
path = "tests/legacy_tests.rs"
reason = "The legacy implementation is intentionally shared."
```

Each exception must name a supported rule, an exact project-relative path, and
a non-empty reason. Glob patterns, global exemptions, and source-code allow
comments are not supported.

## Capabilities and limitations

This first release provides the focused behavior described above. It is intentionally a small building block: project-specific policy belongs in `.infra`, and orchestration belongs in `rs-infra-ci`. It does not promise compatibility with the legacy `rs-ci` scripts beyond the commands currently covered by tests.

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
