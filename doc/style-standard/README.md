# Rust 统一开发规范

本目录是 Rust 开发、源码风格审查和机械门禁的唯一事实源。
权威维护仓库为 [qubit-ltd/rs-infra-style](https://github.com/qubit-ltd/rs-infra-style/tree/main/doc/style-standard)，
技能与审查脚本每次使用 GitHub `main` 最新文档，不再从技能同步规范。

## 阅读顺序

| 文档 | 内容与适用范围 |
| --- | --- |
| [编码规范](rust-coding-standard.md) | 19 个稳定规则 ID、组织、导入、Rustdoc、测试结构、属性、复杂度、README 与风格修正验证 |
| [Rust 文件头](rust-file-header.md) | DOC-001 的完整七行模板与年份保留/修复规则 |
| [双语 README](readme-structure.md) | README-001 的元数据、徽章与末尾章节模板 |
| [开发验证](development-validation.md) | 开发验收的测试、benchmark、fuzz、Clippy、CI 边界 |
| [机械门禁边界](alignment-design.md) | 确定性诊断、人工审查、错误与通过的含义 |
| [远程规范消费协议](consumption.md) | main 查询、完整缓存、版本固定与恢复失效规则 |

## 裁决与维护

先按规则适用范围判断，再查正文和专用模板。当前任务、项目明确约定优先；
有差异的项目例外须记录来源、理由和具体范围。模板中的固定文本以模板为准；
示例仅解释当前规则，不要求其同时展示所有其他规则。

修改规则时同时检查索引、正文、模板和相关实现测试，不复用规则 ID 表示无关含义。
工具不支持的新规则应明确人工检查或未覆盖范围，不能悄悄删掉规则或报告全部通过。
机械检查器可以随版本发布规范快照；快照必须声明来源版本及限制，
不得冒充每次在线更新的 AI 审查规范。

`reviewing-rust-code-style` 和 `rust-style-review.sh` 仅保留执行流程及缓存读取代码；
缓存和 worker 输入是上游同一 commit 的派生副本，不是可独立编辑的规范源。
本地未发布修改不会被远程消费者读取；只有修改进入 GitHub `main` 后，
下一次使用或恢复才会自动采用。

整理日期：2026-10-27。合并了技能版、审查脚本版和原基础设施规范；
保留 19 个既有规则 ID，以更新后的属性、Cargo 测试发现和完整示例规则统一冲突。
