# rs-infra-style

[![Rust CI](https://github.com/qubit-ltd/rs-infra-style/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-infra-style/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-infra-style/coverage-badge.json)](https://qubit-ltd.github.io/rs-infra-style/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-infra-style.svg?color=blue)](https://crates.io/crates/qubit-infra-style)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

检查并安全修正 Qubit 项目当前第一阶段的固定 Rust 风格规则。

## 安装

```bash
cargo install --git https://github.com/qubit-ltd/rs-infra-style.git --tag v0.1.0 qubit-infra-style
```

## 快速开始

在 Rust 项目根目录查看命令帮助：

```bash
cargo run --manifest-path /path/to/rs-infra-style/Cargo.toml -- --project . check
cargo run --manifest-path /path/to/rs-infra-style/Cargo.toml -- --project . fix
```

`check` 将 rustfmt 检查与固定项目风格规则合并执行；`fix` 执行 rustfmt
后再验证固定规则。迁移生成的 `style-check.sh` 与 `align-ci.sh` 可以直接调用这两个稳定子命令。

项目的 `.infra` 配置仍然是行为的唯一来源；工具仓库不会复制项目配置。具体策略由项目配置决定。

确需豁免现存问题时，可在 `.infra/style/exceptions.toml` 中为每条例外指定一个精确的项目相对路径和一条受支持的规则，并说明原因。不支持源码中的 `qubit-style: allow` 注释。

```toml
format = 1

[[exceptions]]
rule = "test-redirect"
path = "tests/fixtures_tests.rs"
reason = "该测试入口必须引入外部测试框架共用的 fixture。"
```

支持的规则为 `test-file-name`、`test-redirect`、`explicit-imports`、
`coverage-cfg`、`aggregation-files`、`public-type-layout`、`type-file-name`
和 `internal-test-module`。只有路径和规则同时精确匹配时才会忽略诊断；格式错误或不支持的例外会使检查失败。

## 固定格式化工具和配置

迁移后的 wrapper 可通过以下设置，让 `check` 和 `fix` 使用同一套 rustfmt 工具链与配置：

```bash
export RS_INFRA_STYLE_TOOLCHAIN=nightly-2026-06-05
export RS_INFRA_STYLE_RUSTFMT_CONFIG="$PWD/.infra/style/rustfmt.toml"
rs-infra-style --project . check
rs-infra-style --project . fix
```

运行前须安装指定工具链及其 rustfmt 组件。`RS_INFRA_STYLE_TOOLCHAIN` 用于选择
`cargo +<toolchain> fmt` 的工具链；`RS_INFRA_STYLE_RUSTFMT_CONFIG` 通过
`--config-path` 指定 rustfmt 配置。这两个变量可分别使用；未设置或值为空时，
对应部分仍沿用 Cargo 的工具链选择方式或 rustfmt 的默认配置查找方式。

配置路径可以包含空格，相对路径以 `--project` 指定的目录为基准。项目已有根目录
`rustfmt.toml` 时，可将配置变量指向该文件；工具不会复制或替换配置文件。
格式化失败会使命令返回失败状态。`fix --dry-run` 只显示配置后的命令，不执行格式化。

## 能力与限制

当前版本只提供上文列出的专门能力，刻意保持为小型基础设施组件：项目策略放在 `.infra`，任务编排交给 `rs-infra-ci`。对于旧版 `rs-ci` 脚本，只有测试覆盖的命令可视为兼容。

## 延伸阅读

可通过命令帮助和源码测试了解实际接口。切换到 [English README](README.md)。

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

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交 Pull Request 前运行 `./align-ci.sh` 格式化代码，运行 `./ci-check.sh` 满足 CI 要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-infra-style](https://github.com/qubit-ltd/rs-infra-style)
