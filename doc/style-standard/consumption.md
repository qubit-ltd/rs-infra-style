# 远程规范消费协议

## 权威定位

仓库：`https://github.com/qubit-ltd/rs-infra-style.git`；分支：`main`；目录：`doc/style-standard/`。
`reviewing-rust-code-style` 与 `rust-style-review.sh` 不依赖本机 rs-infra-style checkout，
不捆绑另一份可独立维护的编码规范，不用绝对或相对路径读取维护者硬盘上的规范源。

## 每次使用

每次启动审查或恢复任务，先通过 Git 查询 `refs/heads/main` 当前 commit。
不使用 TTL 或“今天已经检查过”跳过查询；`--help` 和仅验证自定义测试规则不属于规范审查。
按 repository、commit 和目录定位缓存，校验完整文件清单及逐文件 SHA-256。
缺失、损坏、多余文件或远程 commit 改变时，立即下载该 commit 下的完整目录，
验证必需文件齐全，再原子发布缓存索引。以 commit 获取内容，避免下载过程中混合多个版本。

缓存根为 `${XDG_CACHE_HOME:-$HOME/.cache}/qubit/rust-style-standard/`，按仓库分命名空间，
由跨进程锁保护更新。失败不得发布半份文档，也不得破坏此前有效快照。
远程查询失败、main 缺失、下载失败或目录不完整时明确报错，停止依赖该规则的审查；
不能静默退回旧缓存、技能内副本或本地 checkout。

## 固定版本与恢复

一次 CLI 运行中的所有 worker 使用同一完整快照，可复制为 worktree 内的受控输入。
每份报告/会话记录来源、commit、目录和内容摘要，派生副本不得改写规则。
人工技能的各次会话命令也检查远程，发现变化后停止继续采纳旧结论，
停止并释放 worker，再 sync 到新快照并复核。规则摘要覆盖整个规范目录，
仅文件头、README 模板或验证要求改变，也不能沿用旧的完整规范通过结论。
恢复已完成任务同样先检查 main；规则变化时重新排队，不直接返回旧的完成状态。

过往快照保留以便追溯。维护规范必须修改权威仓库并进入 main；
编辑缓存或 worker 副本不能作为规则更新方式。
