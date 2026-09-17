# Rust 编码规范

本文档是 `reviewing-rust-code-style` 的唯一详细规则来源。每个规则都必须在审核台账中出现终态：`PASS`、`FAIL`、`FIXED`、`N/A` 或 `BLOCKED`。

## 规则索引

| 编号 | 主题 | 检查责任 |
| --- | --- | --- |
| ORG-001 | 类型、文件与内部实现组织 | 脚本 + AI |
| ORG-002 | 类型完整定义与 inherent `impl` | AI |
| ORG-003 | 聚合模块、命名空间与 re-export | 脚本 + AI |
| IMPORT-001 | import 声明与外部依赖引用 | 脚本 + AI |
| DOC-001 | Rust 文件头 | AI |
| DOC-002 | 生产代码 Rustdoc 完整性与语义 | AI |
| DOC-003 | Rustdoc 章节、示例与属性顺序 | 脚本 + AI |
| TEST-001 | 测试层级与放置边界 | AI |
| TEST-002 | 测试覆盖、命名、镜像与 cfg | 脚本 + AI |
| TEST-003 | 测试文件命名 | 脚本 |
| TEST-004 | 测试文件不得重定向源码 | 脚本 |
| BENCH-001 | benchmark 工作负载 | AI |
| FUZZ-001 | fuzz target 安全性与有效性 | AI |
| METHOD-001 | inherent 方法顺序与 builder | AI |
| ATTR-001 | `inline` 与 `must_use` | AI |
| ATTR-003 | coverage 专用 cfg | 脚本 |
| COMP-001 | 复杂度与内部模块 | AI |
| README-001 | 双语 README | AI |
| VERIFY-001 | 修正后的验证顺序 | AI |

## ORG-001：类型、文件与内部实现组织

**适用范围：** 生产 `src` 中的项目自有类型与别名；`src/tests`、root `tests`、benchmark 和 fuzz 中的测试辅助类型不适用一类型一文件限制。

**强制规则：** 每个 `struct`、`trait` 和 `enum` 在单独的 snake_case 文件中定义；相关 type alias 放入其唯一自然所有者文件，否则独立成文件并完整文档化。私有辅助类型放在当前逻辑目录的 `internal/` 子树。复杂对象的构建使用 `XxxBuilder` 与 `Xxx::builder()`；builder 配置方法使用字段名、返回 builder，并以消耗型 `build(self)` 完成构建。

**理由：** 类型定义、实现和文档可被局部理解，内部协作边界不会泄漏为公共结构。

**不合格示例：**

```rust
// src/model.rs
pub struct User { /* ... */ }
pub struct Team { /* ... */ }
```

**合格示例：**

```text
src/model/user.rs      # pub struct User
src/model/team.rs      # pub struct Team
src/model/internal/id.rs # private helper type
```

**例外：** 测试树中紧密相关的局部 helper type 可共存；必须仍保持可读性。空 `struct` 或 `trait` 的花括号体使用独立行和缩进的 `// empty`，但 unit-like `struct Marker;` 不适用。

## ORG-002：类型完整定义与 inherent `impl`

**适用范围：** 所有生产类型的 inherent `impl Type`，包括泛型与 feature-gated 变体。

**强制规则：** 类型声明和直接 inherent `impl` 位于类型定义文件。因规模按职责拆分时，定义文件必须显式声明每个实现子模块；子模块位于类型本地子树，并在 `mod` 声明前注释其职责。未被定义文件声明的领域、格式、适配器或集成模块不得重新打开 `impl Type`。外部领域能力应位于具名 adapter、builder、自由函数或 extension trait。

**理由：** 阅读类型定义文件即可发现完整 inherent API，无需搜索整个 crate。

**不合格示例：**

```rust
// src/formats/json/session.rs
impl RedactionSession {
    pub fn json_with(self) -> Self { self }
}
```

**合格示例：**

```rust
// src/runtime/redaction_session.rs
// Implements construction and lifecycle methods.
mod lifecycle;
// Implements bounded-output methods.
mod output;
```

**例外：** trait implementation 不属于 inherent API；但仍应放在语义最清晰的所有者或 trait 旁边。

## ORG-003：聚合模块、命名空间与 re-export

**适用范围：** `lib.rs`、`mod.rs`、模块级 API 及同 crate re-export。

**强制规则：** `lib.rs` 与 `mod.rs` 仅承载模块文档、声明、同 crate re-export 和小型聚合，不承载业务类型或业务逻辑。无自然所有者的一组内聚无状态 API 可使用独立文件中的空 `enum` 命名空间；已有自然所有者或单个私有 helper 时使用该所有者方法或局部 helper。授权拆分后只用同 crate re-export 保持公共路径；不得公开 re-export 其他 crate 所有的项目。

**不合格示例：**

```rust
// src/foo/mod.rs
use crate::parser::Parser;
pub struct Service { parser: Parser }
```

**合格示例：**

```rust
// src/foo/mod.rs
pub mod service;
pub use service::Service;
```

**例外：** 小型聚合代码可保留在聚合文件；必须不隐藏具体业务实现。

## IMPORT-001：import 声明与外部依赖引用

**适用范围：** 项目自有 Rust 文件。

**强制规则：** 每条 `use` 只导入一个对象，不使用 brace list、通配符或 `#[allow(unused_imports)]`。import 分三组：标准库（`std`、`core`、`alloc`）、外部 crate、当前 crate（`self`、`super`、`crate`）；相邻组间恰有一空行，组内交给 Rust 2024 rustfmt 排序。外部对象在文件顶部导入后用导入名引用；发生冲突时使用语义明确的 alias。纯聚合 `mod.rs` 不收集子模块的私有 import。

**不合格示例：**

```rust
use crate::model::App;
use anyhow::{Result, Context};
use qubit_types::*;

fn load() -> qubit_types::EntityId { todo!() }
```

**合格示例：**

```rust
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use qubit_types::EntityId;

use crate::model::App;
```

**例外：** 无；Rust 版本感知的组内顺序由 rustfmt 决定，不使用字符串排序模拟。

## DOC-001：Rust 文件头

**适用范围：** 每个项目自有 `*.rs` 文件，包括生产、测试、benchmark、fuzz 和 example。

**强制规则：** 使用 [rust-file-header.md](rust-file-header.md) 的模板。保留已接受的年份或范围；缺失时使用 `2025 - {current_year}`。已有但格式错误的块原地修复，不在前面追加第二个头。生成文件优先修复生成器或模板； vendored 文件保留上游许可证并明确记录例外。

**不合格示例：**

```rust
// Copyright 2026
pub fn parse() {}
```

**合格示例：**

```rust
// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
```

**例外：** 仅 vendored 第三方代码按其上游许可证处理。

## DOC-002：生产代码 Rustdoc 完整性与语义

**适用范围：** 生产 `src`，不包括 `src/tests` 和 inline test module。

**强制规则：** 文档化每个类型、别名、字段、变体字段、函数、方法、trait 方法、私有 helper、getter、setter 和 forwarding method。文档解释目的、语义及适用的约束、所有权、生命周期、副作用、并发、IO 与性能；不得只是复述标识符。测试、benchmark、fuzz、example 只需角色相关说明和非显然 helper 文档。

**不合格示例：**

```rust
/// Gets the name.
pub fn name(&self) -> &str { &self.name }
```

**合格示例：**

```rust
/// Returns the display name borrowed from this immutable profile.
///
/// The reference remains valid while `self` is borrowed and performs no allocation.
pub fn name(&self) -> &str { &self.name }
```

**例外：** 无生产代码例外；非生产作用域不适用全条目文档化。

## DOC-003：Rustdoc 章节、示例与属性顺序

**适用范围：** 生产 `src` 的有文档 API；核心类型的类型级 Rustdoc。

**强制规则：** 文档紧邻所注释的 item 或 field，并位于 attributes 前。按适用性提供 `# Type Parameters`、`# Parameters`、`# Returns`（包括每个 `Option` 状态）、`# Errors`、`# Panics`、`# Safety`。每个当前 crate 核心类型必须在类型级 Rustdoc 中有可运行 `# Examples`：通过公共 crate 路径导入，以推荐 constructor 或 builder 构造，调用代表性核心 API，并处理或断言结果。其他 item 在示例可显著防止误用时提供可运行示例。

**不合格示例：**

```rust
#[derive(Debug)]
/// Represents the client.
pub struct Client;
```

**合格示例：**

```rust
/// Sends bounded requests with one shared policy.
///
/// # Examples
///
/// ```
/// use my_crate::Client;
/// let client = Client::builder().build();
/// assert!(client.is_ready());
/// ```
#[derive(Debug)]
pub struct Client;
```

**例外：** 核心类型无“API 很直观”例外；方法片段或其他位置的示例不能替代类型级示例。

## TEST-001：测试层级与放置边界

**适用范围：** root `tests/`、`src/tests/` 与 inline `#[cfg(test)]` modules。

**强制规则：** 在能够完整观察行为的最外层稳定边界放置测试。公开行为使用 root `tests/` 和仅公共路径；真实 `pub(crate)` 合同且无法由公开 API 构建/观察时使用 `src/tests/`；只有必须访问普通 private 或必要 `pub(super)` 项时才使用 inline test。不得仅为移动测试扩大可见性、增加 test hook 或测试 re-export。

**不合格示例：**

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn public_parse() { assert!(crate::parse("ok").is_ok()); }
}
```

**合格示例：**

```rust
// tests/parser_tests.rs
#[test]
fn parses_public_input() { assert!(my_crate::parse("ok").is_ok()); }
```

**例外：** 仅确实需要 private access 的断言保持 inline；混合测试按责任拆分。

## TEST-002：测试覆盖、命名、镜像与 cfg

**适用范围：** 所有测试层级。

**强制规则：** 覆盖适用的成功、错误、边界、回归、所有权和策略行为。默认不重复测试同一行为；多层保留只在合同不同或内层明显改善定位时合理。生产 `src/foo/bar.rs` 的公开/内部行为分别镜像到 `tests/foo/bar_tests.rs` 与 `src/tests/foo/bar_tests.rs`；纯聚合文件不要求空镜像。使用行为导向的 `test_*` 名称、显式 import、精确结构断言和有意义失败消息。feature gate 位于最窄依赖处，并验证默认和最小 feature 的发现。

**不合格示例：**

```rust
use crate::*;
#[test]
fn test1() { assert!(parse("x").is_ok()); }
```

**合格示例：**

```rust
use my_crate::parse;

#[test]
fn test_parse_rejects_empty_input() {
    assert_eq!(parse(""), Err(ParseError::Empty));
}
```

**例外：** 测试 helper 可同文件存在；共享、大型或分散 helper 移到对应测试树的 `internal/`。

## TEST-003：测试文件命名

**适用范围：** root `tests/` 和 `src/tests/`。

**强制规则：** 具体测试文件名为 `*_tests.rs` 或 `mod.rs`；support、common、fixtures 和 coverage support 目录按项目配置排除。

**不合格示例：** `tests/parser.rs`

**合格示例：** `tests/parser_tests.rs`

**例外：** 明确的测试 support 目录。

## TEST-004：测试文件不得重定向源码

**适用范围：** 具体测试文件。

**强制规则：** 测试文件直接包含测试，不用 `include!` 或 `#[path]` 重定向源码来隐藏真实测试位置。

**不合格示例：**

```rust
include!("../../src/parser_tests.rs");
```

**合格示例：**

```rust
#[test]
fn parses_valid_input() { /* concrete assertion */ }
```

**例外：** 无。

## BENCH-001：benchmark 工作负载

**适用范围：** `benches/**/*.rs`。

**强制规则：** benchmark 只测性能，不替代 correctness test；使用稳定代表性输入与 `black_box`，把 setup 放在计时循环外（除非 setup 正是被测负载），不以 wall-clock threshold 作为正确性门槛，计时路径不使用断言。编译每个声明的 benchmark feature 配置。

**不合格示例：**

```rust
assert!(started.elapsed() < Duration::from_millis(5));
```

**合格示例：**

```rust
group.bench_function("parse", |b| b.iter(|| parse(black_box(input))));
```

**例外：** harness 必须验证 setup 时，可在计时区外断言。

## FUZZ-001：fuzz target 安全性与有效性

**适用范围：** `fuzz/**/*.rs`。

**强制规则：** fuzz 真实公开或刻意暴露的 API，不复制生产 parser、validator 或业务逻辑。输入在分配或昂贵解析前有边界；执行可复现，不使用时间、网络、sleep、未控文件系统或非输入随机性；断言 no-panic、round-trip、idempotence、symmetry 或独立实现一致性等有效 invariant。将重要 regression seed 落到普通测试。

**不合格示例：**

```rust
fuzz_target!(|bytes: Vec<u8>| { my_copy_of_parser(&bytes); });
```

**合格示例：**

```rust
fuzz_target!(|bytes: &[u8]| {
    let input = &bytes[..bytes.len().min(4096)];
    let _ = my_crate::parse(input);
});
```

**例外：** 仅共享 input decoding 可抽出 helper；生产逻辑不可复制。

## METHOD-001：inherent 方法顺序与 builder

**适用范围：** 每个 inherent `impl Type`，不同 generic constraint 的 block 分别处理。

**强制规则：** 先 constructors/factories，再其他方法；每组内按 `pub`、受限可见性、private；同一可见性组内 getter 在 setter 前，再是相关 builder 配置、`take_*`、`clear_*` 或 validation。按语义识别 `new`、`try_new`、`from_*`、`parse`、`builder`、`load_default`，不移动 trait method。

**不合格示例：**

```rust
impl Config {
    pub fn validate(&self) {}
    pub fn new() -> Self { Self {} }
}
```

**合格示例：**

```rust
impl Config {
    pub fn new() -> Self { Self {} }
    pub fn validate(&self) {}
}
```

**例外：** trait implementation 方法不参与此排序。

## ATTR-001：`inline` 与 `must_use`

**适用范围：** 生产、受限、private 和 trait API。

**强制规则：** 纯转发的 getter/setter、字段或常量 accessor，以及本体只做少量廉价操作的极薄函数，应使用 `#[inline]`。极薄函数包括简单转换、构造迭代器或 `Option` 的查询，以及短小的纯计算；不要求必须是单表达式。调用频繁的短小函数即使有少量分支或循环，也可根据调用场景加 `#[inline]`。对其他函数，根据本体成本、调用频率和调用边界判断：若主要成本在 I/O、分配、锁、重试等昂贵操作中，仅仅把这些操作包在单表达式 wrapper 中，不足以要求内联。较大函数和普通冷路径由编译器自行决定。不得按行数、语法形状或 `pub`/private 可见性批量增删属性。

**`inline(always)` 的边界：** `#[inline]` 是常规内联提示；不要把 `#[inline(always)]` 当作默认升级。只有在目标构建配置下，以 `#[inline]` 为对照的性能分析显示强制内联带来稳定、明确的收益，且代码体积等代价可接受时才保留或新增，并记录证据。没有这类证据时，符合上述候选条件的函数降为 `#[inline]`；其余函数去掉提示。已有 `#[inline]` 也按候选条件判断，不因追求少加属性而机械删除。

**`must_use`：** 每个明确 getter 使用 `#[must_use]`；调用后丢弃结果没有实际意义的函数和必须被观察的返回类型（尤其自定义 error enum）使用 `#[must_use]`。有实际副作用、丢弃返回值仍有意义的函数不因非 unit 返回而机械标记。

**官方依据：** Rust Reference 说明三种 `inline` 形式都是提示，编译器可能忽略，且编译器会自行内联；不合适的内联决定可能使程序变慢。参见 [Rust Reference 的 inline 属性](https://doc.rust-lang.org/reference/attributes/codegen.html#the-inline-attribute)。

**覆盖率与 inline 等级：** CI 函数覆盖率（如 llvm-cov）可能因 `#[inline(always)]` 将函数体内联到调用方而缺少独立计数。先确认行为测试与缺计数原因；没有强制内联的性能证据时，按上述规则评估降为 `#[inline]` 或去掉提示。确有性能证据时，不要只为覆盖率计数牺牲该收益，应按 ATTR-003 处理例外；也不要在 `src` 增加仅为计数的 `#[cfg(test)]` 锚点。

**不合格示例：**

```rust
pub fn name(&self) -> &str { &self.name }
pub fn emit_event(&self) -> bool { true }
```

**合格示例：**

```rust
#[must_use]
#[inline]
pub fn name(&self) -> &str { &self.name }

pub fn emit_event(&self) -> bool { true }
```

**内联选择示例：**

```rust
// 不合格：常规 always
#[inline(always)]
pub fn label(&self) -> &str { &self.label }

// 合格：纯转发 getter 保留内联机会
#[must_use]
#[inline]
pub fn label(&self) -> &str { &self.label }

// 合格：短小、廉价的分支函数也可 inline
#[inline]
pub fn is_marker(byte: u8) -> bool { byte == b'<' || byte == b'>' }

// 合格：外层转发主要成本在重试与 I/O 中，无须内联提示
pub fn run(&self, input: Input) -> Result<Output, Error> {
    self.execute_with_retries(input)
}
```

**例外：** `emit_event` 之类有意义副作用的 API 不要求 `#[must_use]`。

## ATTR-003：coverage 专用 cfg

**适用范围：** 生产 `src`。

**强制规则：** 不使用 coverage 专用 `cfg` 或 `cfg_attr` 改变生产路径；用行为测试表达 coverage 需求。例外必须同时有文件内 allow 注释和项目 allowlist 的带理由条目。

**覆盖率不足时的补救顺序：** ① 在 root `tests/`（或适用的 `src/tests/`）确认或补充真实行为测试，并核实缺计数原因；② 没有强制内联性能证据时，按 ATTR-001 评估将 `#[inline(always)]` 改为 `#[inline]` 或去掉提示；③ 若有证据需要保留 `inline(always)`，或无法通过合理的内联级别解决计数问题，且仍无法在不违反 TEST 规则的前提下达标，才申请经审查的 coverage-cfg 双重 allow。

**不合格示例：**

```rust
#[cfg(coverage)]
fn only_for_coverage() {}
```

**合格示例：**

```rust
#[test]
fn covers_error_path() { assert!(run_bad_input().is_err()); }
```

**例外：** 经审查的双重 allow 记录。

## COMP-001：复杂度与内部模块

**适用范围：** 生产实现。

**强制规则：** 按领域职责拆分，而非只按行数。深层嵌套、重复策略分支、重复转换映射、解析/验证/错误构造混杂、大型 match arm、循环内 collection scan 都是审查信号。使用可文档化、可测试的具名 helper，私有实现放入 `internal/`。优化前先添加 characterization 或 regression test，不用 wall-clock unit test 作为复杂度证据。

**不合格示例：**

```rust
fn handle(input: &str) { /* parse, validate, map, log, retry and format */ }
```

**合格示例：**

```rust
fn handle(input: &str) -> Result<Output, Error> {
    let parsed = parse_input(input)?;
    validate_input(&parsed)?;
    render_output(parsed)
}
```

**例外：** 无；抽取不得扩大公开接口。

## README-001：双语 README

**适用范围：** 项目 README。

**强制规则：** 同时有 `README.md` 与 `README.zh_CN.md`，并完整遵循 [readme-structure.md](readme-structure.md)。从当前项目解析仓库 URL、owner/name、Cargo crate 与最低 Rust 版本，不复制其他项目元数据。两份 README 的功能表、限制、安全/资源限制、规范格式、示例和公开行为语义一致。

**不合格示例：** 英文 README 声明一个 feature，中文 README 缺失该 feature。

**合格示例：** 两份 README 以各自自然语言描述同一组 feature、限制和示例。

**例外：** 无。

## VERIFY-001：修正后的验证顺序

**适用范围：** 已授权的 correction mode。

**强制规则：** 依次运行聚焦回归与最小 feature 测试、项目 alignment/format、立即检查 diff、`git diff --check`、项目 CI、文档/doctest/benchmark/fuzz/相关 feature，最后仅在 CI 覆盖率不足或用户要求时运行 coverage。deadline 不能改变顺序。review-only 仅检查脚本存在性，不运行会写文件的格式化/alignment，不添加测试。

**不合格示例：**

```text
先运行 CI；如果通过则不运行 cargo fmt。
```

**合格示例：**

```text
focused tests → alignment → inspect diff → diff --check → CI → doctests/features
```

**例外：** 不可用工具和省略 feature 必须记录为 `BLOCKED` 或未运行，不能声称通过。
