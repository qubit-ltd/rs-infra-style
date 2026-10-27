# 完整开发验收

本文件规定实现功能、修复缺陷和维护检查器时的验证要求。
仅源码风格审查遵守编码规范 VERIFY-001 的范围，不因本文件自动增加测试或构建任务。

## 测试设计

测试覆盖适用的成功、错误、边界、回归、所有权和策略行为。
优先在能完整观察行为的最外层稳定边界验证；多层保留同一行为测试时，
须有不同合同或明显改善定位的理由。不得为迁移测试扩大可见性或增加 test hook。
测试发现、目录镜像和命名以 TEST-001 至 TEST-004 为准。
验证默认及适用的最小 feature 配置，不能把未被 Cargo 发现的文件计为已测试。

## benchmark 与 fuzz 设计

benchmark 使用稳定且有代表性的输入与 `black_box`，setup 放在计时循环外，
除非 setup 正是被测负载。计时路径不使用断言，不以 wall-clock 阈值当正确性门槛。
仅在计时外验证必要 setup；benchmark 不替代正确性测试。

fuzz 调用真实公开或刻意暴露的 API，不复制生产逻辑。
在分配或昂贵解析前限制输入；不依赖时间、网络、sleep、未控文件系统或非输入随机性。
断言 no-panic、round-trip、idempotence、symmetry 或独立实现一致性等有效不变量，
重要 regression seed 落入普通测试。

## 验证顺序与范围

1. 运行与当前变更相关的聚焦回归和适用最小 feature 验证。
2. 运行项目 alignment/format，立即检查最终 diff 和 `git diff --check`。
3. 运行当前 crate 的 style-check、编译、Clippy 和项目 CI；
   涉及 must_use 时确保 `clippy::double_must_use` 已检查。
4. 补充 CI 尚未覆盖且与变更相关的文档/doctest、feature、benchmark 或 fuzz 验证。
5. 仅在 CI 覆盖率不足或用户要求时额外运行 coverage；按 ATTR-003 处理例外。

CI 只负责当前 crate 自身，不加入或保留 downstream all-features 测试。
`--all-features` 仅在当前 crate 的 features 可组合且项目要求时使用，
互斥 features 按有效组合验证。不为了验证擅自重建锁文件、改依赖或安装工具链。
缺少工具、依赖或环境时记录实际阻断，区分源码语法、类型检查、测试与行为验证的结果。
