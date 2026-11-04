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

## 确定性分析

`analyze` 是独立于 `check` 和 `fix` 的只读源码分析入口。输入为包含仓库相对
Rust 路径的 JSON 数组，精确清单可覆盖非默认 workspace 成员、自定义 Cargo
目标和独立 fuzz crate。关联源码和配置仅作为上下文读取，不执行 Cargo metadata、
依赖解析、构建或宏展开。

```sh
printf '["src/lib.rs", "tests/api_tests.rs"]' > /tmp/rust-source-list.json
rs-infra-style analyze --project . --files-from /tmp/rust-source-list.json \
  --standards /path/to/style-standard --format json
```

带版本的 JSON 报告区分证据充分的 `error` 诊断与信息级审查提示。每个输入文件
均包含规范及机械子检查覆盖：`pass`、`fail`、`needs_semantic_review`、
`not_applicable`、`blocked`。机械扫描无问题后，文档语义、领域边界、测试位置、
方法顺序和依赖上下文的属性仍须语义审查。既有 `Diagnostic`、`check`、`fix`
接口保持原有约定。

退出码 `0` 表示扫描完成且未发现确定违规，`1` 表示发现确定违规，`2` 表示分析
未完整完成。工具缺失、输入文件不可读或无法解析、配置无效及规范版本不匹配均
不会被当作通过。item 宏及条件声明保留明确的待审覆盖。惯例中的测试 support
目录须有明确例外或语义确认，不能仅按目录名自动判定豁免。

报告通过 SHA-256 摘要绑定分析器版本、实际读取的源码和配置及规范版本。
`--standards` 必须与二进制内嵌的七份规范文档一致；规范升级后须使用匹配的
分析器版本。程序通过项目配置的已安装工具链运行 rustfmt，使用 stdin 和
`skip_children=true`，不会写回源码；import 排序以 rustfmt 为准，不使用字符串
排序模拟。详细覆盖与边界见[分析器协议](doc/analyzer.md)。



项目优先级更高的版权规则可写入 `.infra/style/analysis.toml`，必须记录明确理由：

```toml
format = 1

[header]
owner = "Example Team"
minimum_year = 2020
reason = "本项目沿用既有的版权身份。"
```

七行模板的其他行保持精确匹配。已接受的单年份或以 `minimum_year` 开始的年份
范围均予保留；无效或缺少理由的覆盖配置会阻止分析。通过
`--repository-readme` 可显式增加独立的仓库 README 结构诊断；只检查源码时，
README-001 仍标为文件范围外。该选项将 README 的存在状态及项目元数据纳入
输入摘要，但不替代双语语义一致性审查。

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
