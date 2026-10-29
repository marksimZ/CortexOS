# Phase C：个性化评测实施方案

2026-10-27 最新状态：用户已验收 C2 并授权完成 C3–C5。项目快照、确认发送、候选审核发布、复用 Runner 的串行评测与报告现已交付开发版；见 [C3–C5 实施与验收](benchmark-phase-c3-c5.md)。真实云端计费及 Docker 实测仍未验收。下方验收门槛保留为历史记录。

2026-10-25。用户授权 C0 后直接实施 C1，C1 完成后停止。

2026-10-27 更新：用户已确认 C1 验收，继续 C2。当前交付[工作区使用偏好](benchmark-phase-c2.md)，完成后停在 C2 验收；以下 C1 状态为历史记录，C3–C5 尚未实施。

## C0 仓库复核

| 实际能力 | 代码依据 | 验证边界 |
| --- | --- | --- |
| 中文、数学、JSON、RAG、原生工具评分 | `apps/desktop/src-tauri/src/benchmark/evaluation.rs`、`evaluation_json.rs`、`evaluation_tests.rs` | 引擎 core60-runtime-2.1，真实解析结构，非文本模拟工具 |
| Python 隔离控制器 | `benchmark/python.rs`、`python_runner.py`、`python_tests.rs` | 已实现限制与清理；本机无 Docker，真实隔离测试仍未执行 |
| 单题评测与状态保存 | `benchmark/runner.rs`、`runner_v2.rs`、`scoring_repository.rs` | 只有本地 Ollama、单模型单题，不是已完成批量评测产品 |
| 不可变数据集 | `benchmark/dataset.rs`、`dataset_v2.rs`、`datasets/` | v1/v2、内容哈希、评分版本；个性化数据集尚不能直接传入 |
| 用途隔离、迁移 | `migrations/0008_benchmark_foundation.sql`、`src/db.rs` | schema 8，迁移前备份、事务回滚；不改既有迁移 |
| 模型基础设施 | `ollama.rs`、`generation.rs`、`benchmark/preflight.rs` | 统一 Provider 仍是设计；实际执行依赖 Ollama，不能声称已有云端适配器 |
| 界面 | `App.tsx`、`components/AppShell.tsx`、`navigation.ts` | 原有对话/知识/图谱/设置；没有标准评测结果页面可直接扩展 |

复核了阶段 B 报告及本机 `artifacts/benchmark-scoring-v2/scope-check.json`：历史证据为 37 个 Rust 检查、7 个不同原生检查，14 个受控实验/11 次替身请求。它们不是本轮重跑结果，也不是 60 题真实模型成绩。Docker 和真实云端均未验收。

## C1 的最小适配

- Rust 新增 cloud 模块，固定 OpenAI、Anthropic、Gemini HTTPS 模型目录协议，复用 reqwest；不放宽 Ollama 地址策略。
- Windows Credential Manager 保存一个版本化连接记录（密钥、Provider、所选模型、最后验证状态/时间）。单记录原子替换，避免 SQLite 与凭据存储双写不一致。C1 不需要 SQL 迁移。后续多连接/审计元数据再引入只含引用的配置表。
- React 仅短暂收集输入；调用后立即清空。返回结构不含密钥、尾号或原始错误正文。凭据不可读时拒绝准入，没有明文后备。
- 查询目录成功后选择真实返回的模型，再验证并连接；不硬编码模型名称、价格、上下文或生成能力。目录权限不等于推理权限/余额，未知能力明确保留未知。
- 统一 Rust Guard 读取安全存储真实状态；网络异常保留凭据并要求重试，不标成密钥无效。24 小时后要求重新验证；退出/删除立即拒绝访问。
- 新增评测入口：标准页展示真实固定题库目录，明确运行/结果界面尚未交付；个性化子路由在首次配置和已连接状态间切换。C2 尚未实现，不伪造问卷或生成按钮。
- C1 仅实现目录读取与连接管理；没有生成/个性化任务创建/恢复接口，直接 invoke 无法越过尚不存在的功能。C4/C5 新写入和恢复入口必须调用同一 Guard，并验证发送授权。

## C2–C5（早期拆分，现已按上述授权实施）

1. C2：工作区画像和不可变版本，非必要问题可跳过，偏好多选/权重可编辑。
2. C3：授权文件夹/ZIP 只读扫描、路径与符号链接边界、敏感文件排除、限额与快照；复用来源授权原则，不能把现有单文件导入当作仓库扫描器。
3. C4：统一 Provider 的生成能力；请求计划绑定模型/片段哈希/预算/确认，有限请求与取消；生成候选经静态校验和人工批准后发布不可变题库。Schema 适配受既有评分器支持词汇限制，未知类型标 Unsupported。
4. C5：扩展现有 dataset repository 的数据集标识和准入，复用 Runner/评分存储；补原切片 4 的串行排程/恢复，再接多模型报告、导出、图谱。云端执行适配在此完成，不建立第二套评分引擎。

按相同题库/规则/覆盖率比较，单独呈现偏好权重与缺失数据；费用未知时不伪造估价或允许预算保护任务启动。首次连接不代表私有代码上传许可。

## 验收

C1 使用 Mock Provider 测试绑定、分类错误、取消、超时、重启、删除、更换、模型选择和后端 Guard；浏览器验证路由/表单/三尺寸，原生检查实际 IPC 与安全存储（只使用合成凭据）。运行类型检查与桌面构建。真实三家密钥、计费/配额及企业网络待用户环境验收。保留 Docker 实测待办，不安装软件或调用付费模型。

协议依据：[OpenAI 模型目录](https://developers.openai.com/api/reference/resources/models/methods/list)、[Anthropic 模型目录](https://platform.claude.com/docs/en/api/models/list)、[Gemini 模型目录](https://ai.google.dev/api/models)、[Windows CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew)。仅读取官方文档，未提交真实凭据。
