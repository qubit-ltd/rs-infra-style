# rs-infra-style

[![Rust CI](https://github.com/qubit-ltd/rs-infra-style/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-infra-style/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-infra-style/coverage-badge.json)](https://qubit-ltd.github.io/rs-infra-style/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-infra-style.svg?color=blue)](https://crates.io/crates/qubit-infra-style)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

检查并安全修正 Qubit 项目当前第一阶段的固定 Rust 风格规则。

本工具随附的 Rust 风格权威标准位于
[`doc/style-standard/`](doc/style-standard/)。该目录同步自
`reviewing-rust-code-style` skill，包含编码规范、Rust 文件头模板和双语
README 结构。检查器是机械门禁，只报告能够根据项目文件确定证明的违规；
语义、性能、所有权和测试质量仍由完整 skill 审查判断。

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

支持的规则为 `inline-tests`、`test-file-name`、`test-redirect`、
`source-test-pair`、`explicit-imports`、`coverage-cfg`、`aggregation-files`、
`public-type-layout`、`multiple-public-types`、`type-file-name` 和
`internal-test-module`。只有路径和规则同时精确匹配时才会忽略诊断；格式错误或不支持的例外会使检查失败。

## 统一格式化配置

先运行项目更新脚本，将本仓库维护的 [`conf/rustfmt.toml`](conf/rustfmt.toml)
安装到项目的 `.infra/style/rustfmt.toml`：

```bash
./update-infra.sh --yes
```

`check` 和 `fix` 都读取项目中的该文件，并把绝对路径传给 rustfmt。文件缺失时，
工具会报错并提示运行更新脚本。如需固定 rustfmt 工具链，可单独设置：

```bash
export RS_INFRA_STYLE_TOOLCHAIN=nightly-2026-06-05
rs-infra-style --project . check
rs-infra-style --project . fix
```

运行前须安装指定工具链及其 rustfmt 组件。`RS_INFRA_STYLE_TOOLCHAIN` 选择
`cargo +<toolchain> fmt`；未设置或值为空时沿用 Cargo 的工具链选择方式。
格式化失败会使命令返回失败状态。`fix --dry-run` 只显示命令，不执行 Cargo。

## 能力与限制

当前版本保留可机械化处理的旧 `rs-ci/style-check.sh` 规则和可选的
`STYLE_ENFORCE_SOURCE_TEST_PAIRS` 兼容开关。由于 skill 允许确有 private
访问需求的 inline test，工具不会自动禁止所有 inline test。项目例外放在
`.infra/style/exceptions.toml`；coverage 例外还必须有源码中的
`qubit-style: allow coverage-cfg` 注释。Clippy、coverage 和 Cargo.lock 更新等
编排仍由 `rs-infra-ci` 负责。

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
