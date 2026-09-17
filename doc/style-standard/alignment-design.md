# rs-infra-style 对齐 Rust 风格 skill 的设计

日期：2026-09-17。状态：方向已批准，具体设计待审阅；本轮只设计。

目标仓库：`/home/starfish/working/qubit/rust-infra/rs-infra-style`。
基线：`8c9a9ce043871767900fa3a4ee5b5b4c6c0f1a7f`，`dev-starfish`，设计开始时工作区干净。
Temporary Workspace: `/tmp/superpowers-rs-infra-style-dnC3PfCo`

## 1. 目标和规范来源

将工具变成 skill 的可靠机械门禁，遵循用户明确要求：“只处理能机械化处理的内容，宁可漏报不要误报”。只有已证明适用且确定违规的子项才阻断；不确定性、启发式候选和人工审查项不改变门禁退出状态。未覆盖的规则不得显示为完整通过。

仓库最终参考标准为 `doc/style-standard/README.md` 及其链接的三个规则文件，已按用户要求落入目标仓库。实现、旧测试和 legacy 配置必须服从这些规范。规范含全部规则，门禁仅实现满足精度要求的子集。

本任务以 `reviewing-rust-code-style` 的正文和三个 reference 文件为准。脚本、旧版工具和已有例外仅提供实现参考，不能覆盖正文。一般规则优先级仍为当前任务、项目明确规则、skill；本次用户已明确要求工具与 skill 冲突时以 skill 为准。

规范目录：`/home/starfish/working/qubit/dev-support/vibe-tools/dot-agents/skills/reviewing-rust-code-style/`。

| 规范文件 | SHA-256 |
| --- | --- |
| references/rust-coding-standard.md | be29084e7b0ba70170251268206ab4d983867fd023f3e1efd0e97c939bfe77a0 |
| references/rust-file-header.md | dbec6f1d087b0f9443315f1f301e4316fb8e058b930056621c5d3268b74bbe5d |
| references/readme-structure.md | 31f8378d0ee3310659f06c6b254198c54a181596ab004f22cf9bdc38ee69ca2d |

发布时随二进制携带规范快照、来源和摘要。运行时不依赖维护者机器上的 skill 绝对路径，也不自动拉取最新规则。上游 skill 更新后，通过显式更新快照、规则映射和回归用例发布新版本。

范围包含全部 19 条规则的覆盖声明和其中可确定子项的机械检查；人工清单可以作为辅助产物，但不要求实现语义分析或启发式评分引擎。不包含批量修改下游项目、更新下游 pins、修改 skill、发布或 Git 推送。

## 2. 方案选择

| 方案 | 收益 | 局限 |
| --- | --- | --- |
| 继续增加逐行字符串检查 | 改动少 | 多行属性、注释、嵌套模块、宏和 cfg 误判持续增加 |
| 基于 syn AST 和 Cargo 元数据实现确定性规则，输出人工清单（采用） | 复用现有依赖，精确定位，检查范围和判断依据可解释 | 不提供完整类型解析、宏展开和性能证明 |
| 接入编译器完整语义分析 | 能解析更多符号与 cfg | 工具链耦合、编译成本和跨 feature 分析复杂，仍不能判断文档质量和性能收益 |

不内置模型服务，也不为本次门禁实现复杂度、性能或文档质量的启发式分析。后续 AI 审查可以消费已有文件清单和人工规则覆盖声明。

## 3. 必须先纠正的旧行为

1. `check_inline_tests` 一律拒绝源文件中的测试属性，与 TEST-001 允许必要 private/pub(super) 测试冲突；还可能拒绝合法 `#[cfg(test)] mod tests;`。skill 模式不沿用该禁令。
2. `check_source_test_pairs` 接受任意目录同名测试、源目录旁的测试或父模块测试，不能证明精确镜像；缺少 tests 目录时直接返回也不能视为满足规则。
3. `check_test_file` 在文件含测试时跳过 redirect 检查。具体测试文件内的源码重定向必须检查，不能用添加一个测试绕过。`include_str!` 等数据包含不属于源码重定向。
4. imports 目前只从生产文件入口检查；必须扩展到所有项目自有 Rust 文件，包括测试、benchmark、fuzz、example。
5. coverage 目前只需 TOML 例外，与 ATTR-003 的源码注释和项目 allowlist 双重要求冲突。
6. aggregation 目前禁止所有普通常量等非 mod/use 项，未表达 skill 允许的小型聚合；需要对含义不确定的聚合项请求审查。
7. 公有类型检查忽略 restricted/private 类型；正文要求覆盖生产类型，不限 pub。
8. WalkDir 错误被 `filter_map(Result::ok)` 丢弃、部分 AST 解析错误被忽略。skill 模式必须报告确切未检查路径。

skill 自带 Shell 检查器也保留 legacy 开关。它可用于差分测试，但不能作为所有期望结果的唯一裁判。

## 4. 扫描和分析模型

处理流程：配置与规范版本 → 文件清单 → AST/文档解析 → 模块与符号索引 → 规则检查 → 例外验证 → 统一报告。

- 默认覆盖 Cargo workspace 全体成员；`--package` 或已有显式目录参数可缩小范围，报告必须列出所选范围及遗漏成员。
- 使用 Cargo targets 发现自定义 lib/bin/test/bench/example 路径，同时枚举成员内项目自有 Rust 文件，发现未接入模块树的文件。包含独立 fuzz crate 和 build.rs。
- 路径去重，维护 production、external-test、internal-test、benchmark、fuzz、example、other、generated、vendor 文件角色。inline-test 是 item 级作用域，不能把整个生产文件降为测试角色。
- 排除 target/.git 等构建元数据；vendor/generated 使用显式配置并记录理由。检测到生成标记但未配置归属时输出待确认项，不静默跳过。生成文件的违规仍报告，修复指向生成器；vendor 保留上游许可证。
- 不跟随目录符号链接无限递归；Cargo 明确引用的外部路径记录作用域，扫描同一真实文件仅一次，fix 不越出所选项目边界。
- `syn` 启用 visitor 能力，使用 span 行列定位。解析多行属性、受限可见性、嵌套模块、use tree、impl、字段、变体和文档属性。
- 建立模块路径、定义、inherent impl、trait impl、use/re-export、测试函数索引。无法解析的类型路径、外部宏生成 API 和 cfg 关系保留人工审查状态。
- 扫描各 cfg 分支的原始声明并记录条件；不把互斥分支中的同一类型简单算作多类型冲突。可证明的互斥条件按条件处理，不能证明则请求审查。
- 文件读取失败、已知 Rust 文件解析失败、必需元数据不可得为 BLOCKED；宏展开缺失等预期能力限制输出 REVIEW_REQUIRED，并明确相关规则没有完整覆盖。

## 5. 全部规则覆盖设计

“自动”仅指该列能够证明的子项；表中候选、定位、索引均不是阻断检查。若实现无法在宏、cfg、名称解析等条件下证明结论，允许跳过并声明边界。不是所有列出的分析能力都必须在首版实现；同一规则含人工项时，机械通过不等于规则通过。

| 规则 | 自动检查 | 人工审查边界 |
| --- | --- | --- |
| ORG-001 | 各可见性 struct/enum/trait 的单文件布局和 snake_case；普通空 struct/trait 的 // empty；builder 已识别结构的 build(self) 签名 | alias 自然所有者、private helper 职责、是否需要 builder、复杂 cfg 和宏生成类型 |
| ORG-002 | 可解析 inherent impl 的定义归属、类型本地实现子树、定义文件的显式 mod 声明与职责注释存在性；trait impl 分开 | 模块职责是否合理、歧义名称/alias/复杂 cfg；不根据同名类型直接判违规 |
| ORG-003 | 聚合文件中的明确业务类型；已解析且归属外部 crate 的公开 re-export；纯聚合私有 import | 小型聚合与业务逻辑边界、空 enum 命名空间适用性、歧义 re-export 链 |
| IMPORT-001 | AST 检查单对象、brace/glob、unused_imports allow；三组空行；2024 rustfmt 排序；可解析的外部路径未顶部导入 | 宏内引用、路径歧义、alias 命名质量；不能按文本字母排序替代 rustfmt |
| DOC-001 | 所有自有 Rust 文件的七行模板、owner/SPDX、年份格式、重复/残缺头；保留合法原年份 | 不合规年份的真实取值、vendor 上游许可和生成器定位 |
| DOC-002 | 生产类型/alias/字段/变体/变体字段/函数/方法/trait 方法的文档存在性、非空及精确总数；排除测试作用域 | 英文语义、约束、并发、所有权、性能和文档是否仅复述名称；宏或外部 include 文档不能误报缺失 |
| DOC-003 | 文档与属性顺序；明确签名所需章节的存在性；标记示例代码块及 ignore/no_run；unsafe API Safety 章节 | 核心类型识别、示例代表性、Option 各状态说明、隐藏 panic/error、示例可运行性需实际 doctest 证据 |
| TEST-001 | 测试文件/inline 作用域清单；AST 检查 src/tests 的真实连接及 cfg 附着 | 公开行为是否应外移、是否确需 private access；不能禁止全部 inline，也不能通过扩大可见性修复 |
| TEST-002 | test_* 命名；为每个非纯聚合源文件生成精确外部/内部镜像候选；验证已声明映射路径存在 | 由公开/内部/私有合同选择正确层级；覆盖质量、重复必要性、feature 发现需运行验证；不为所有源文件强制生成空测试 |
| TEST-003 | 具体测试文件 *_tests.rs/mod.rs；按配置识别 support 目录 | 目录角色歧义；support 豁免不传递至 imports/header 等无关规则 |
| TEST-004 | 具体测试文件的 include! / #[path] 源码重定向，不受同文件是否已有测试影响 | 宏生成重定向的未展开部分；数据包含不误报 |
| BENCH-001 | benchmark 入口和 feature 清单；定位计时闭包内断言等候选 | black_box 是否充分、setup 边界、负载代表性、性能与正确性分离；编译交由验证命令 |
| FUZZ-001 | target 与 feature 清单；时间/网络/sleep/文件系统和随机调用候选位置 | 输入边界、确定性、invariant 有效性、生产逻辑复制、seed 转普通回归 |
| METHOD-001 | 在确定类别内检查可见性排序；trait impl 排除；每个 generic impl 独立 | 构造器、getter、setter、builder 的语义类别；不能只凭方法名称判定 |
| ATTR-001 | 首版仅声明人工审查范围，不做阻断检查 | getter/薄函数判定、const/宏等边界、成本、副作用、频率、性能收益都交人工；不自动增删属性 |
| ATTR-003 | 多行 cfg/cfg_attr coverage 条件；源码 allow 注释与精确路径带理由记录双重匹配 | coverage 补救顺序及 inline 性能证据真实性 |
| COMP-001 | 首版仅声明人工审查范围，不实现启发式扫描 | 领域复杂度、拆分方式；行数或复杂度数值不作为新增硬性标准 |
| README-001 | 双语文件存在；Cargo 元数据；六 badge、尾部四 H2 及固定文本；尾部无额外实质内容 | 双语功能/限制/示例语义一致；不明 metadata 为 BLOCKED，不猜测 |
| VERIFY-001 | 导出所需验证顺序和命令清单，记录本次工具实际执行阶段 | 项目 CI/coverage/feature 证据由外部流程提供；一次 check/fix 不能证明全流程完成 |

Rustdoc 章节按实际适用性检查。返回类型是别名且不能解析时，不猜测 Result/Option。宏属性可能生成文档时标为待审查。README 模板严格按 reference 比较；只统一 CRLF/LF，正文不得擅自宽松化。版权头参考中的七行模板优先于编码标准内简化示例。

## 6. 报告和退出语义

新增 `AnalysisReport`，包含规范摘要、profile、scope、文件角色、诊断、规则覆盖、人工候选、例外和工具执行结果。

- 诊断带 canonical rule_id、细分 check_id、文件/行列、item 标识、证据、修正方向；兼容字段保留 legacy code。
- 对 19 条规则分别记录 mechanical 子项和 semantic 子项。覆盖状态为 PASS、FAIL、REVIEW_REQUIRED、BLOCKED、N/A；N/A 必须附可观察理由。REVIEW_REQUIRED 是工具工作队列，不冒充 skill 台账的终态。
- 未人工验证的语义规则不输出 PASS。工具默认不接受一个布尔“已人工审核”开关来伪造完成。
- JSON 新报告使用 schema_version=2；stdout 只有结构化报告，rustfmt 等子进程输出进入 stderr 或报告字段。
- Text 默认显示确定违规、人工项数量、未扫描范围和“机械检查通过，仍有人工项”等准确结论。提供 TSV 用于 skill 台账导入，FILE/规则 FAIL/SUMMARY 保持可识别，人工清单单独输出记录。
- skill profile 退出码：0 表示机械检查通过且扫描未阻断；1 表示确定违规或 rustfmt 差异；2 表示配置、解析、读取或工具执行阻断。有阻断优先返回 2。人工项不使默认 CI 失败。
- `check` 不写目标源码或清单文件；报告默认 stdout。`--output` 只有显式指定时写文件。
- `rules --format json` 输出全部规则、各子检查和自动化边界，便于验证规范更新是否遗漏 ID。

## 7. 兼容与例外

引入 `--profile legacy|skill`。首个发布版本默认 legacy，显式 `--profile skill` 启用本设计；实际对齐验收必须使用 skill profile。规则不因旧环境变量关闭，若存在会改变语义的 legacy 开关，skill profile 报配置冲突并说明迁移方式。

保留现有 `check`/`fix` 命令和公开 Rust 函数签名、`Diagnostic` 结构与 legacy JSON v1。新增报告 API 与 CLI skill JSON v2，不往公开 struct 直接添加破坏字面量构造的字段。旧规则逻辑仅在 legacy profile 提供兼容；不继续用于 skill 合规判断。

旧 `.infra/style/exceptions.toml` format=1 在 legacy 模式沿用。skill 模式使用 format=2，按 canonical rule/check ID、精确路径、理由记录。未知规则、空理由、重复、路径越界和不受规范支持的豁免一律报配置问题。

format=2 中区分 scope 声明、规范允许的 exception 和人工 rationale。带理由不意味着允许违背规范；IMPORT-001、TEST-004 等无例外规则不能通过配置放行。旧文件中的任意豁免不得自动升级为 skill 例外；空 format=1 文件可以无损兼容，含条目则要求明确迁移。

coverage exception 在 format=2 中充当项目 allowlist，仍必须有源码 `// qubit-style: allow coverage-cfg` 注释。支持读取旧 `.qubit-style-allowlist` 的 coverage 条目以辅助迁移；同路径双来源冲突为配置错误，不静默选择。源码 blanket allow all 不支持。

生成迁移建议时只输出映射及无法迁移的条目，不自动改写下游文件。本轮不改变下游工具 pins。后续新版本是否切换默认 profile，单独决策。

## 8. fix 行为

第一版 skill fix 保持 rustfmt 加重新检查，避免同时引入新规则和复杂自动修复。`--dry-run` 显示所有将执行的 formatter 命令，包括独立 fuzz manifest，且不执行写操作。

后续安全修复独立扩展：缺失版权头可在确定无现存许可证块时补齐；合法旧年份不得刷新；残缺头原地修复、年份不确定则人工处理。自动修复不得自行移动文件、改变公共路径、补写猜测的语义文档、批量增删 inline/must_use、迁移测试或新增例外。

formatter 使用与 check 相同的受控 Rust 2024 配置。skill profile 验证 edition/style_edition、imports_granularity、group_imports、reorder_imports 的有效配置；工具链或配置冲突报告阻断。不复制或覆盖项目配置。与现有未设置环境变量行为的兼容由 legacy profile 保证。

## 9. 实现组织与交付顺序

保留 CLI 和 legacy API，将新实现放入独立模块，避免继续扩张 style_checker.rs：

- `analysis_report.rs`、`rule_id.rs`、`file_role.rs` 等公开报告类型，一类型一文件。
- `internal/discovery/`：Cargo 范围、文件角色、读取与错误记录。
- `internal/analysis/`：AST、模块图、文档和符号索引。
- `internal/rules/`：按组织/import/doc/test/attr/readme 分工的规则函数。
- `internal/reporting/`：text/JSON/TSV、覆盖汇总与 legacy 适配。
- `doc/style-standard/`：规范原文、来源摘要和机械门禁边界；已按用户要求创建，不依赖本机 skill 路径。
- `tests/`：公共接口回归；`tests/fixtures/`：最小正反例及 cfg/宏边界用例。

交付依赖：

1. 报告与 canonical ID、profile 兼容、扫描范围和 AST 基础。
2. 首批纠正冲突，并实现 imports、header、测试命名/重定向、coverage 和类型布局。
3. 文档清单与章节、可解析 inherent 所有权、README 结构、精确镜像分析。
4. 完整 19 规则覆盖声明，将方法语义/属性/benchmark/fuzz/复杂度明确交给人工；不要求编写这些规则的启发式检查器。
5. CLI/文档/例外迁移说明及集成验证。

步骤 2 和 3 中写入集合独立的规则可在共享接口稳定后并行；报告与集成集中验证。每步都具备可调用的增量能力，不把不完整阶段宣称为最终对齐。

## 10. 验收标准

每个确定性检查至少包含违反、通过和适用边界用例；覆盖误报与漏报，不仅验证诊断数量。

重点回归：

- 必要 private inline 测试和 crate 根 `#[cfg(test)] mod tests;` 不被机械禁止。
- 同文件已有测试再 include! 仍报告 TEST-004；include_str! 数据不报。
- 测试/bench/fuzz/example 的 brace/glob imports 被发现。
- 生产 private/restricted 类型受布局约束，测试 helper 不受一类型一文件约束。
- 多行 cfg_attr coverage 被发现；仅 TOML 或仅源码注释不能豁免。
- 相同文件名但目录错误的测试不能作为精确镜像证据。
- 文档位于 derive 后、私有字段无文档、inline 测试误计入生产统计分别有回归。
- 完整七行版权头保留既有合法年份；残缺头不能当作无头追加。
- 同 crate 链式 re-export 与外部 crate re-export 区分；无法解析时不判 PASS。
- 不同 cfg 分支同名类型、宏中源码文本、注释内 use、custom targets、独立 fuzz、default-members 子集均覆盖。
- README 错项目链接、缺 badge、错误末尾顺序、作者节后内容有独立用例。
- WalkDir/文件读取/AST/Cargo/formatter 失败可观察，不能产生全通过结果。
- JSON stdout 可解析；退出码与机械失败、阻断、人工候选组合符合约定。
- legacy CLI、Rust API、JSON v1、format=1 例外维持已声明兼容；skill profile 不继承冲突规则。
- snapshot 规则索引、rules 命令和报告恰好包含同一组 19 个 ID，不遗漏、不重复。

实现验证顺序遵守 VERIFY-001：聚焦回归与最小 feature → alignment → diff 审查 → diff --check → 项目 CI → CI 未覆盖的 doctest/features/bench/fuzz。只有 CI 覆盖不足或用户要求时额外跑 coverage。

新二进制必须从本次源码构建后测试；不能把 wrapper 调用旧缓存版本的通过作为新规则的证据。skill Shell 脚本做共同规则差分；发生分歧回到规范正文和专门回归，不追求复刻旧误报。

完成含义：19 条规则均有可追踪覆盖说明；确定性部分有测试；人工部分有完整范围与证据清单；工具通过输出明确限制。文档语义、性能收益、测试充分性等仍由 skill 完整审查确认。

## 11. 设计自审

- 已将最初“20 条”更正为实际 19 条。
- 已区分 skill 正文与附带脚本行为，未将 legacy 禁令升级为新规范。
- 已区分工具机械通过和完整人工规则通过。
- 已明确不为未知语义发出阻断性风格失败；真正无法读取/解析的范围仍阻断。
- 已明确首版 skill profile 为显式启用；legacy 兼容不等于已对齐。
- 用户追加要求已纳入：完整 Markdown 规范放入目标仓库；门禁宁可漏报、不误报。人工候选不阻断，不要求实现启发式分析引擎。
- 未执行源码修改、format、提交或下游迁移；产物为具体设计文档及仓库内权威规范文档。
