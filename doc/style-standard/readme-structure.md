# Bilingual README Structure Constants

These templates are authoritative for Rust projects containing both
`README.md` and `README.zh_CN.md`. Replace only brace-delimited placeholders.

## Project Metadata

Resolve every placeholder from the project under review:

| Placeholder | Source |
| --- | --- |
| `{repository_url}` | The current project's canonical repository URL, preferably `package.repository` from `Cargo.toml`. |
| `{repository_owner}` | Repository owner parsed from `{repository_url}`. |
| `{repository_name}` | Repository name parsed from `{repository_url}`. |
| `{crate_name}` | Cargo package name from `Cargo.toml`. |
| `{rust_version}` | Minimum supported Rust version, without a leading `v`, from authoritative project configuration. |

Do not copy metadata from another project. Every CI, coverage, crates.io, and
repository link must target the current project. Report unresolved or
conflicting metadata instead of guessing.

## Opening Badge Block

Place the applicable language template immediately after the single H1 project
title, separated from it and the following introduction by one blank line. Keep
the six badges contiguous so Markdown renders them as one badge row.

English `README.md`:

```markdown
[![Rust CI]({repository_url}/actions/workflows/ci.yml/badge.svg)]({repository_url}/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://{repository_owner}.github.io/{repository_name}/coverage-badge.json)](https://{repository_owner}.github.io/{repository_name}/coverage/)
[![Crates.io](https://img.shields.io/crates/v/{crate_name}.svg?color=blue)](https://crates.io/crates/{crate_name})
[![Rust](https://img.shields.io/badge/rust-{rust_version}+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)
```

Chinese `README.zh_CN.md`:

```markdown
[![Rust CI]({repository_url}/actions/workflows/ci.yml/badge.svg)]({repository_url}/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://{repository_owner}.github.io/{repository_name}/coverage-badge.json)](https://{repository_owner}.github.io/{repository_name}/coverage/)
[![Crates.io](https://img.shields.io/crates/v/{crate_name}.svg?color=blue)](https://crates.io/crates/{crate_name})
[![Rust](https://img.shields.io/badge/rust-{rust_version}+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)
```

## Required Final Sections

The final four H2 sections must appear in the exact order below. No H2 section
or substantive content may follow the author section. Preserve the fixed text,
commands, copyright statement, and author identity; replace only
`{repository_url}`.

English `README.md`:

````markdown
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

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./align-ci.sh` to format code and
`./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [{repository_url}]({repository_url})
````

Chinese `README.zh_CN.md`:

````markdown
## 测试

```bash
# 使用默认 feature 集运行测试
cargo test

# 使用项目声明的全部 feature 运行测试
cargo test --all-features

# 运行项目 CI 检查
./ci-check.sh

# 检查代码覆盖率
./coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./align-ci.sh`格式化代码，运行`./ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[{repository_url}]({repository_url})
````

## Audit Rules

- Require both README files. Report a missing counterpart as a structural
  finding.
- Compare the badge block line by line after resolving project metadata.
- Compare the final four sections by heading order and exact template text.
- Treat wrong-project links, copied crate names, stale Rust versions, missing
  badges, renamed headings, reordered sections, and trailing H2 sections as
  findings.
- In correction mode, change only README structure and project-specific
  placeholders unless the user authorizes broader prose edits.
