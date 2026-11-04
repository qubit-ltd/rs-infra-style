# 确定性分析协议

`rs-infra-style analyze --project ROOT --files-from LIST --standards SNAPSHOT --format json`

`LIST` 为 JSON 字符串数组，仅接受仓库内普通 `.rs` 文件的规范相对路径。
不接受绝对路径、`..`、反斜杠或符号链接。按路径排序并去重；`files` 回显精确清单。
规范目录由调用方准备，程序不下载规范或工具链。

## 报告与退出码

schema_version 为 1。字段包括 `analyzer_version`、`standards_digest`、
`supported_standards_digest`、`input_digest`、`files`、`diagnostics`、`coverage`、
`blocked`、`excluded`、`context_files`。摘要使用 SHA-256；规范摘要兼容审查流水线
对完整规范文件摘要映射的排序 JSON 编码。所有读取的 Rust、Cargo 清单、项目
style 配置及工具链 defaults 纳入输入摘要。调度器应额外绑定二进制摘要，防止
开发期间同一包版本的不同构建复用结果。

诊断包含规则 ID、稳定 `check_id`、`severity`（`error` 或 `info`）、路径、行列、
`message`、`evidence`、`help` 和 `related_locations`。信息级提示不能当作确定违规。
覆盖状态为 `pass`、`fail`、`needs_semantic_review`、`not_applicable`、`blocked`。
每个输入文件均列出所有规范 ID；文件级 README 与调度验证标为 N/A，不冒充
仓库级验收。子检查通过不代表所属整条规范通过。

退出码 0 为机械分析完成且无确定违规；1 为确定违规；2 为不完整分析。存在
blocked 时优先返回 2，同时保留已发现诊断。输入协议本身无效时输出 stderr，
返回 2，不伪造有效 JSON。调用方必须校验 JSON 协议和退出码一致性。

## 统一机械门禁

`.infra/bin/style-check.sh` 调用的 `check` 先执行项目 rustfmt 检查，再运行本协议
的同一机械引擎；既有 CI style 任务因此自动包含 Analyzer。默认发现 Git tracked
和非忽略 untracked Rust 文件，包括非默认 workspace 成员、独立 fuzz crate 和
fixture；显式 source/test 目录选择及非 Git 项目发现行为继续支持。

`check` 只读；`fix` 和 alignment 仅通过 rustfmt 格式化，再执行同一机械校验。
其他机械问题只报告，不自动修改源码，也不调用 agent。检查结果使用既有
`Diagnostic` 数组投影确定错误，保留兼容规则代码；本协议的 `analyze` 报告结构
不变，继续提供稳定 `check_id`、信息提示和覆盖情况。原 `STYLE009` 聚合文件
业务 item 与 `STYLE012` internal 测试模块关系检查需要语义判断，改为待审提示，
不作为确定错误。`STYLE016` 继续检查纯 `mod.rs` 聚合文件中可确定的私有
import；条件配置存在歧义时为待审提示。`explicit-imports` 例外保留适用的 import 策略豁免，项目 rustfmt
格式检查仍必需。

三个入口都只在机械检查完整且无确定违规时返回 0；确定违规返回 1，检查不完整
或工具执行失败优先返回 2。信息级语义提示不阻断，成功不代表完整语义审查通过。
`check --format json` 在格式失败或分析不完整时仍输出诊断数组，工具日志写入
stderr。新机械规则直接成为门禁，升级时发现的存量问题不会自动豁免。

## 子检查登记表

| 规范 | 稳定子检查 | 边界 |
| --- | --- | --- |
| DOC-001 | header-template | 精确七行模板、合法保留年份；只读诊断 |
| IMPORT-001 | single-object-import, import-groups, unused-imports-allow, rustfmt-import-order | AST 遍历；组内排序委托项目 rustfmt |
| ORG-001 | one-type-per-file, type-file-name, empty-type-body, builder-build-receiver | 私有和公开类型、inline 模块；条件声明不按同时生效计数 |
| DOC-002 | rustdoc-presence | 生产类型、别名、字段、变体、方法及 helper；测试上下文排除 |
| DOC-003 | documentation-attribute-order, required-documentation-sections, public-type-examples | 明确签名章节确定检查；核心类型判断保留语义审查 |
| TEST-002 | test-function-name, test-reachability | test_* 前缀确定检查；无法完整解析的模块图为审查提示 |
| TEST-003 | test-file-name | 具体测试文件命名；support 目录须确认例外 |
| TEST-004 | test-source-redirection | AST include!/path/cfg_attr；宏生成内容仍待审 |
| ATTR-001 | trait-inline-without-body, double-must-use-self | 无函数体 inline；直接同模块 Self 与本地类型明确重复标记 |
| ATTR-003 | coverage-cfg | AST cfg/cfg_attr；文件真实注释与带理由 allowlist 同时满足 |
| ORG-002 | inherent-owner | 候选类型、定义文件及声明关系；不猜测别名或跨模块名称解析 |
| ORG-003 | aggregation-items | item 分类信号，保留小型聚合例外判断 |
| METHOD-001 | method-inventory | inherent 方法位置和签名清单，不把名称启发式当作语义分类 |
| COMP-001 | complexity-evidence | 分支、循环、嵌套证据，不定义复杂度拒绝阈值 |
| ORG-001 | conditional-configuration, unexpanded-macro | 明确保留条件配置、宏生成的未解析范围 |

TEST-001、BENCH-001、FUZZ-001 等要求保留完整语义覆盖，不因未发现机械诊断而
标记通过。README-001 和 VERIFY-001 由上层协调者负责仓库级检查和验证。
Rust 源码的完整语义审查、类型解析、宏展开、doctest 可编译性和行为测试均不由
此入口替代。开发验收继续采用仓库原有 alignment 和 CI 门禁。

## 显式项目覆盖与仓库级范围

`.infra/style/analysis.toml` 支持 `format = 1` 与 `[header]` 下的 `owner`、
`minimum_year`、`reason`。三个覆盖字段必须同时提供：owner 为无占位符的非空
单行文本，minimum_year 为四位年份，reason 为非空理由。版权行以
`Copyright (c) {year} {owner}.` 或以 minimum_year 开始的范围匹配，接受后保留
原年份。其余六行仍按权威模板精确检查。配置来源和理由进入 header-template
覆盖证据；配置字节参与输入摘要。符号链接配置在读取前拒绝。

`--repository-readme` 增加独立的 `repository_diagnostics` 与
`repository_coverage`，检查 README 双语文件存在、标题和徽章结构及最终章节
顺序；未自动判定的模板正文、项目自定义入口和双语语义保留明确待审覆盖。
未指定此选项时字段为空，不改变源码审查范围。仓库级确定错误参与退出码 1，
无法安全读取或解析必要元数据参与退出码 2。选项本身、文件缺失状态、实际读取
的 README 及 Cargo 元数据均绑定输入摘要。
