# Rust 开发编码规范

本目录是 Qubit Rust 项目的统一规范与唯一事实源；规范维护入口为
[qubit-ltd/rs-infra-style](https://github.com/qubit-ltd/rs-infra-style/tree/main/doc/style-standard)。
技能、脚本和机械检查器消费本规范，不另行定义或维护规则副本。

“必须”“不得”是强制要求；“应”表示默认要求，偏离时须说明适用边界和依据。
具体模板以本目录被链接的模板文件为准；工具实现、历史行为和检查通过结果不能覆盖规范。
当前任务及项目明确约定优先；与统一规范不同的项目例外须记录来源、范围和理由。

完整开发验收还应遵守 [开发验证要求](development-validation.md)。
源码风格审查只评估规则的源码适用部分：不以评估测试充分性、运行 benchmark、
fuzz 或覆盖率代替源码审查，不自动扩展任务范围。
每个规则在适用层级记录 `PASS`、`FAIL`、`FIXED`、`N/A` 或 `BLOCKED`；
文件级无法评估的仓库规则说明 N/A 原因，在仓库级统一核对。

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
| TEST-002 | 测试源码命名、镜像与 Cargo 发现 | 脚本 + AI |
| TEST-003 | 测试文件命名 | 脚本 |
| TEST-004 | 测试文件不得重定向源码 | 脚本 |
| BENCH-001 | benchmark 源码组织 | AI |
| FUZZ-001 | fuzz 源码组织 | AI |
| METHOD-001 | inherent 方法顺序与 builder | AI |
| ATTR-001 | `inline` 与 `must_use` | AI |
| ATTR-003 | coverage 专用 cfg | 脚本 + AI |
| COMP-001 | 复杂度与内部模块 | AI |
| README-001 | 双语 README | 脚本 + AI |
| VERIFY-001 | 风格修正后的验证 | AI |

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

**例外：** 小型聚合代码可保留在聚合文件；必须不隐藏具体业务实现。对外导出的宏若必须在下游展开时引用某外部 crate，且宏调用方无法通过依赖路径直接解析该 crate，可在 `#[doc(hidden)]` 的支持命名空间中重导出该依赖，供宏实现使用。此重导出必须限定为宏展开所需的最小支持路径，不得作为普通用户 API 宣传或用于绕开模块所有权边界。

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
//
//    Licensed under the Apache License, Version 2.0.
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

**强制规则：** 文档紧邻所注释的 item 或 field，并位于 attributes 前。按适用性提供 `# Type Parameters`、`# Parameters`、`# Returns`（包括每个 `Option` 状态）、`# Errors`、`# Panics`、`# Safety`。每个当前 crate 的公开核心类型必须在类型级 Rustdoc 中有完整且可通过公共 API 编译的 `# Examples`：通过公共 crate 路径导入，以推荐 constructor 或 builder 构造，调用代表性核心 API，并处理或断言结果。私有类型只需按 DOC-002 说明语义，不强求无法通过公共路径编译的示例。其他 item 在示例可显著防止误用时提供完整示例。

**无公共构造路径：** 框架提供的上下文、回调参数和错误类型，可以在完整示例函数中接收该类型的值或引用，再调用代表性公共 API 并处理结果；应说明值由哪个公共流程提供。无需新增公共构造器、扩大可见性，也不使用私有字段、伪造值或 `ignore` 示例来满足构造要求。

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
/// let client = Client::new();
/// assert!(client.is_ready());
/// ```
#[derive(Debug)]
pub struct Client;

impl Client {
    /// Creates a ready client.
    #[must_use]
    #[inline]
    pub fn new() -> Self { Self }

    /// Reports whether this client is ready.
    #[must_use]
    #[inline]
    pub fn is_ready(&self) -> bool { true }
}
```

**例外：** 公开核心类型无“API 很直观”例外；方法片段或其他位置的示例不能替代类型级示例。

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
fn test_parses_public_input() { assert!(my_crate::parse("ok").is_ok()); }
```

**例外：** 仅确实需要 private access 的断言保持 inline；混合测试按责任拆分。

## TEST-002：测试源码命名、镜像与 Cargo 发现

**适用范围：** 所有测试层级。

**强制规则：** 公开/内部行为对应的测试源码分别放在 `tests/` 与 `src/tests/`，按生产模块结构镜像命名；纯聚合文件不要求空镜像。嵌套的 `tests/foo/bar_tests.rs` 必须由 Cargo 能发现的顶层测试目标逐级通过 `mod` 声明引入；仅有文件而没有入口或模块引用不算合格。使用行为导向的 `test_*` 名称、显式 import、精确结构断言和有意义的失败消息。feature gate 放在依赖该 feature 的最窄源码范围。此规则只检查源码组织和语法，不要求运行测试或评估行为覆盖率。

**不合格示例：**

```rust
use crate::*;
#[test]
fn test1() { assert!(parse("x").is_ok()); }
```

**合格示例：**

```rust
use my_crate::parse;
use my_crate::ParseError;

#[test]
fn test_parse_rejects_empty_input() {
    assert_eq!(parse(""), Err(ParseError::Empty));
}
```

**例外：** 测试 helper 可同文件存在；共享、大型或分散 helper 移到对应测试树的 `internal/`。顶层 `tests/foo_tests.rs` 可作为 `tests/foo/mod.rs` 的入口；多文件目标也可使用 Cargo 支持的 `tests/foo/main.rs`。

## TEST-003：测试文件命名

**适用范围：** root `tests/` 和 `src/tests/`。

**强制规则：** 具体测试模块文件名为 `*_tests.rs` 或 `mod.rs`；Cargo 多文件集成测试目标的入口可命名为 `main.rs`。support、common、fixtures 目录按项目配置排除。

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
use my_crate::parse;

#[test]
fn test_parses_valid_input() { assert!(parse("ok").is_ok()); }
```

**例外：** 无。

## BENCH-001：benchmark 源码组织

**适用范围：** `benches/**/*.rs`。

**强制规则：** benchmark 入口与 helper 按 Cargo target 和模块路径组织；函数、组与输入变量使用能说明工作负载的名称。遵守通用的 import、文档和文件组织规则。此规则只检查源码风格与语法，不评价测量结果、输入代表性或运行表现。

**不合格示例：**

```rust
fn bench1() { /* ... */ }
```

**合格示例：**

```rust
group.bench_function("parse_short_header", |b| b.iter(|| parse(black_box(header))));
```

**例外：** 项目采用自定义 benchmark harness 时，遵守该 harness 的入口约定。

## FUZZ-001：fuzz 源码组织

**适用范围：** `fuzz/**/*.rs`。

**强制规则：** fuzz target 入口、输入转换与 helper 使用明确命名和清晰的模块边界；不在 target 内复制生产 parser、validator 或业务实现。遵守通用的 import、文档和文件组织规则。此规则只检查源码风格与语法，不评价 fuzz 运行效果或 invariant 是否充分。

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

**例外：** 多个 target 共用的输入解码可放入具名 helper 模块。

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

**新增 `#[inline]`：** 按函数自身的工作、调用频率和调用边界判断，不按行数、单表达式形状或 `pub`/private 可见性批量增删属性。

1. **应添加：** 纯转发的 getter/setter、字段或常量 accessor，以及自身只做少量廉价操作的极薄函数。包括简单转换、构造迭代器或 `Option` 的查询、短小的纯计算，以及调用底层操作后仅做轻量结果转换的公开薄包装。判断包装层时，区分自身工作与被调用操作的成本；底层涉及原子操作、锁或 I/O，不单独构成拒绝内联的理由。
2. **可以添加：** 有少量分支或循环的短函数，若存在明确的高频调用场景或跨 crate 调用边界，可说明具体场景后添加。
3. **不主动添加：** 函数自身主要执行格式化、分配、I/O、锁等待、重试循环或较大段业务逻辑，且没有明确的调用场景或性能证据。单表达式也不能自动视为廉价；普通冷路径由编译器自行决定。

**已有 `#[inline]`：** 不符合新增条件不等于必须删除。已有且位置合法的普通 `#[inline]` 默认保留；不能仅因被调用操作的成本较高、函数不是单表达式，或缺少新增属性所需的证据而删除。只有属性位置不合法，或目标构建配置下的性能、代码体积证据表明它有害时才删除，并记录具体证据。

**trait 方法的 `inline` 位置：** `#[inline]` 和 `#[inline(always)]` 都只能放在有函数体的方法上。没有默认实现的 trait 方法声明不得标记其中任一种属性。trait 方法有默认实现时，按默认函数体判断是否符合上述内联条件；标在该方法上的属性只作用于默认实现。具体 `impl Trait for Type` 中的方法按其自身函数体独立判断，并将符合条件的属性标在该实现方法上；不要把实现方法的属性移到 trait 声明上。

**`inline(always)` 的边界：** `#[inline]` 是常规内联提示；不要把 `#[inline(always)]` 当作默认升级。新增 `#[inline(always)]` 须有目标构建配置下以 `#[inline]` 为对照的性能分析，确认强制内联带来稳定、明确的收益，且代码体积等代价可接受，并记录证据。已有且位置合法的 `#[inline(always)]` 默认保留；缺少随代码记录的历史性能证据，不能作为降级理由。只有目标构建配置下的性能或代码体积对比证据支持调整时，才逐项降为 `#[inline]` 或移除；不得在风格审查中批量替换。位置不合法时移除属性。普通 `#[inline]` 是否保留按上述已有属性的规则判断。

**`must_use`：** 调用后丢弃结果没有实际意义的函数，以及必须被观察的返回类型（尤其自定义 error enum），应让编译器在忽略结果时给出提醒。按以下顺序判断，明确的 getter 也不例外：

1. 先检查实际返回类型，必要时展开类型别名。若返回类型本身已有 `#[must_use]`（如 `Result`、`Option` 或已标记的自定义类型），不要给函数重复添加，以免触发 `clippy::double_must_use`。此项优先于“丢弃结果没有意义”的判断，适用于新增属性和已有属性；已有的无说明重复标记应移除，不通过添加 `allow`、`expect` 或无实际新契约的消息规避诊断。
2. 返回 `&T` 或 `&mut T` 时，检查 `T` 的实际类型，必要时展开类型别名。若 `T` 已标记 `#[must_use]`，不要给函数添加无说明的 `#[must_use]`：Clippy 也会将这种引用识别为 `double_must_use`。需要明确提醒调用方检查该引用时，可以使用带具体说明的 `#[must_use = "..."]`。注意，`T` 上的属性本身不会使忽略 `&T` 或 `&mut T` 的调用触发 `rustc` 的 `unused_must_use` 警告。若 `T` 未标记，且忽略返回的引用没有意义，可给函数添加 `#[must_use]`。返回元组时，若其中包含已标记的类型且仍需给函数标记，应使用带具体说明的形式，避免 `clippy::double_must_use`。
3. 需要给 trait 方法添加 `#[must_use]` 时，放在 trait 声明上；不要为同一调用契约在 trait impl 的对应方法上重复添加。有实际副作用、丢弃返回值仍有意义的函数，不因非 unit 返回而机械标记。

**`Self` 与宏生成方法：** 返回 `Self` 的构造函数、`with_*`、builder 或链式方法，必须查阅所属类型定义；类型上已有 `#[must_use]` 时，函数不再重复标记。主文件是宏定义时，沿直接或嵌套宏调用追踪到实际使用类型及其属性，不能只读宏体，或仅凭“消耗 self，丢弃结果会丢失更新”添加属性。同一宏被多个类型复用时分别核对调用场景；不要向所有展开无条件添加 `#[must_use]`。无法确定实际返回类型时，记录缺失上下文及未解决项，不猜测或报告 `PASS`。`ATTR-001` 的证据须写明检查的类型定义和宏调用路径。

**开发验收中的重复标记验证：** `clippy::double_must_use` 由 Clippy 在类型检查和宏展开后判断，rustfmt、项目自定义 style-check 和普通 `cargo check` 不能替代。完整开发验收执行 `cargo clippy --locked --workspace --all-targets -- -D clippy::double_must_use`；仅源码风格审查不自动运行 Clippy，也不得将纯语法通过宣称为此项诊断已通过。参见 [Clippy double_must_use](https://rust-lang.github.io/rust-clippy/master/index.html#double_must_use)。

**官方依据：** Rust Reference 说明 `inline` 只能用于有函数体的函数，trait 默认方法上的属性只作用于默认函数体；三种 `inline` 形式都是提示，编译器可能忽略，且编译器会自行内联。不合适的内联决定可能使程序变慢。参见 [Rust Reference 的 inline 属性](https://doc.rust-lang.org/reference/attributes/codegen.html#the-inline-attribute)。

**不合格示例：**

```rust
pub fn name(&self) -> &str { &self.name }
```

**合格示例：**

```rust
#[must_use]
#[inline]
pub fn name(&self) -> &str { &self.name }

// Result 本身已标记 must_use，函数不再重复标记。
pub fn validate(&self) -> Result<(), ValidationError> { Ok(()) }

#[must_use = "the wrapper must be stored or invoked"]
pub struct Wrapper;

impl Wrapper {
    // Self 已由类型级属性覆盖，消耗 self 也不重复标记函数。
    pub fn with_name(self, _name: &str) -> Self { self }
}

pub fn emit_event(&mut self) -> bool { self.emitted += 1; true }
```

**内联选择示例：**

```rust
// 新增时不合格：缺少性能对比证据的 always
#[inline(always)]
pub fn label(&self) -> &str { &self.label }

// 合格：纯转发 getter 保留内联机会
#[must_use]
#[inline]
pub fn label(&self) -> &str { &self.label }

// 合格：setter 自身纯转发，底层原子存储不构成拒绝内联的理由
#[inline]
pub fn store(&self, value: Arc<T>) { self.inner.store(value) }

// 合格：公开薄包装只调用一次底层 CAS 并转换返回值
#[inline]
pub fn compare_and_exchange(&self, current: &Arc<T>, new: Arc<T>) -> Arc<T> {
    Guard::into_inner(self.inner.compare_and_swap(current, new))
}

// 合格：短小、廉价的分支函数也可 inline
#[inline]
pub fn is_marker(byte: u8) -> bool { byte == b'<' || byte == b'>' }

// 合格：函数自身执行格式化，没有明确调用场景时不主动添加 inline
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.load())
}
```

**例外：** `emit_event` 之类确有副作用的 API 不要求 `#[must_use]`。

## ATTR-003：coverage 专用 cfg

**适用范围：** 生产 `src`。

**强制规则：** 不使用 coverage 专用 `cfg` 或 `cfg_attr` 改变生产路径；用行为测试表达 coverage 需求。例外必须同时有文件内 allow 注释和项目 allowlist 的带理由条目。

**开发验收时覆盖率不足的补救顺序（不由风格审查自动执行）：** ① 在 root `tests/`（或适用的 `src/tests/`）确认或补充真实行为测试，并核实缺计数原因；② 检查属性位置和计数工具限制；已有合法 `#[inline(always)]` 按 ATTR-001 保留，只有明确的性能或代码体积对比证据支持时才调整，不能为补计数机械降级；③ 若有证据需要保留 `inline(always)`，或无法通过合理的内联级别解决计数问题，且仍无法在不违反 TEST 规则的前提下达标，才申请经审查的 coverage-cfg 双重 allow。

**不合格示例：**

```rust
#[cfg(coverage)]
fn only_for_coverage() {}
```

**合格示例：**

```rust
#[test]
fn test_covers_error_path() { assert!(run_bad_input().is_err()); }
```

**例外：** 经审查的双重 allow 记录。

## COMP-001：复杂度与内部模块

**适用范围：** 生产实现。

**强制规则：** 按领域职责拆分，而非只按行数。深层嵌套、重复策略分支、重复转换映射、解析/验证/错误构造混杂、大型 match arm、循环内 collection scan 都是审查信号。使用职责明确、可文档化的具名 helper，私有实现放入 `internal/`。修正时保持现有行为，不以代码行数作为拆分依据。

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

**适用范围：** 仓库级项目 README；单文件源码风格审查记录 N/A，不据此宣称 README 通过。

**强制规则：** 同时有 `README.md` 与 `README.zh_CN.md`，并完整遵循 [readme-structure.md](readme-structure.md)。从当前项目解析仓库 URL、owner/name、Cargo crate 与最低 Rust 版本，不复制其他项目元数据。两份 README 的功能表、限制、安全/资源限制、规范格式、示例和公开行为语义一致。

**不合格示例：** 英文 README 声明一个 feature，中文 README 缺失该 feature。

**合格示例：** 两份 README 以各自自然语言描述同一组 feature、限制和示例。

**例外：** 无。

## VERIFY-001：风格修正后的验证

**适用范围：** 已授权的修正模式；本规则由调度器执行，单文件会话标记 `N/A`。

**强制规则：** 先完成所有 worker 的 Rust 风格审阅与修正，合并到一个集成 worktree，然后执行一次全项目 `.infra/bin/align-ci.sh`。失败时调用 agent 修复相关 Rust 风格问题，有限重试；没有进展或越界时保留现场并记录未完成。alignment 成功后，对最终实际改动的 Rust 源文件使用 Rust 编译器纯解析模式检查语法。解析失败可修复本次风格引入的语法错误，必须成功复查才能进入回写流程。

**职责边界：** 不执行 Cargo 构建基线、Clippy、feature 矩阵、fixture 测试或 fuzz 构建；不重建锁文件，不修改依赖或自动安装工具链。纯语法检查不等同于编译、类型检查、测试或行为验证，报告不得混淆这些结论。源码中的预期编译失败测试场景必须保留，不为了项目构建而改变测试契约。

**回写：** 原始工作树可以已有或新增提交、暂存和未暂存改动。按 base、原始工作树当前内容和风格修正结果做三方合并；冲突 agent 必须优先保留原始变更的功能目的，再结合风格修正。不得覆盖并发产生的修改，不改变原始 HEAD 或暂存区。无法安全解决时保留隔离冲突现场及三份版本，明确告知用户需手动解决，不声称全部完成。

**只读模式：** 只审阅并报告，不执行 alignment、语法修复或回写。

**合格流程：** 所有 worker 审阅 → 合并集成 → 全项目 alignment / 有限 agent 修复 → Rust 编译器纯语法检查 → 保留原始意图的三方回写。

**例外：** 缺少必要工具时报告未验证范围并保留现场，不下载工具链或转而修改依赖。

**技能执行器的验证档位：** 手工逐文件会话可在源码风格验证之外执行项目 style-check 与 `cargo check --workspace --all-targets`（独立 fuzz crate 按 manifest 编译），须事先说明采用含编译检查的档位，并如实记录命令与结果。此为增加验证，不能取代源码语义审查；默认 CLI 调度器采用上面的纯语法档位。两者均不默认运行测试、benchmark、fuzz、doctest 或 CI。
