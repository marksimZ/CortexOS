# C2：工作区使用偏好

2026-10-27。用户确认 C1 验收并授权下一步；本轮只交付 C2，C3–C5 保持规划。

## 操作与边界

在“评测 → 个性化评测”完成已有连接验证后点击“继续”，进入当前工作区的使用偏好：

1. 工作与水平：多选领域、自定义领域、可选专业水平。
2. 任务与工具：多选任务、自由补充；按所选领域建议工具，同时支持自定义清单。切换领域保留已选工具。
3. 模型选择偏好：质量、速度、费用、本地资源、隐私、编程、长上下文各 0–5，默认均为 3，至少一项非零。

全部问题可跳过。跳过创建默认偏好的独立版本；保存后展示完整摘要、版本和时间，可修改或确认重置。重置恢复未填写状态并新增版本，旧记录保留。没有据此生成模型分数或推荐。偏好仅保存在本机，不发送给模型服务。

未保存草稿按工作区保留在本次前端进程内，切换页面/工作区后可继续；不是持久化草稿。当前编辑页关闭/刷新时使用 beforeunload 提醒，但应用进程被强制结束仍会丢失未保存输入。保存失败保留填写内容；版本冲突提示重新读取，可明确放弃本地草稿读取最新版本。

## 接口与数据

- `get_benchmark_profile(workspaceId)`：返回该工作区最新版本或 null。
- `save_benchmark_profile(workspaceId, input)`：input 含 expectedVersion、action（save/skip/reset）、profile。数据类型、枚举、长度、工具数量和权重由 Rust 校验。
- 所有公开读取/写入均调用 C1 的 `CloudState::with_access`，持有连接操作锁完成本地操作，避免与本应用解绑/更换并发。失效凭据不能继续读写画像；历史数据保留供重新连接后读取。前端状态或深链接参数不构成授权。
- 工作区版本单调递增，expectedVersion 乐观并发校验；保存使用 SQLite 事务。
- 新增 `0009_benchmark_profiles.sql`，schema 8 → 9；只有 `benchmark_profile_versions` 一张表。记录 workspace_id、version、action、profile_json、内容 SHA-256、created_at，更新和删除触发器保证版本不可变。
- 沿用现有 migrate：升级前 VACUUM INTO 备份，迁移事务失败整体回滚。旧版程序不能直接打开 schema 9；如需回退应使用兼容版本或升级前备份。没有打开用户生产数据库执行升级。
- 保留原 0001–0008 迁移、Core60 数据和已有评分器/Runner；不改聊天或知识的数据语义。

## 实际修改文件

路径相对仓库根目录：

| 文件 | 内容 |
| --- | --- |
| `apps/desktop/src-tauri/migrations/0009_benchmark_profiles.sql` | 新增画像版本表与不可变约束 |
| `apps/desktop/src-tauri/src/benchmark/profile.rs` | 新增类型、校验、工作区查询、事务保存、2 个命令 |
| `apps/desktop/src-tauri/src/benchmark/mod.rs` | 注册 profile 模块 |
| `apps/desktop/src-tauri/src/db.rs` | 追加迁移与画像测试模块 |
| `apps/desktop/src-tauri/src/db/profile_tests.rs` | 真实 SQLite 版本/隔离/重置/重开、8→9 备份/失败恢复专项 |
| `apps/desktop/src-tauri/src/db/benchmark_foundation_tests.rs` | 既有迁移检查更新 schema 9 和新增表数量预期，保留旧数据断言 |
| `apps/desktop/src-tauri/src/cloud/mod.rs`、`cloud/tests.rs` | 复用 Guard 的带锁本地操作接口与未配置/撤销检查 |
| `apps/desktop/src-tauri/src/lib.rs`、`build.rs`、`capabilities/default.json` | 仅注册画像读写命令权限 |
| `apps/desktop/src/benchmark/ProfileEditor.tsx` | 新增三步编辑、摘要、修改/跳过/重置与进程内草稿 |
| `apps/desktop/src/benchmark/BenchmarkPage.tsx`、`CloudConnection.tsx` | C1 成功后进入 C2，返回连接设置 |
| `apps/desktop/src/benchmark/benchmark.css` | 复用 Tokens，选项列表、权重行、三尺寸布局和底部操作区 |
| `apps/desktop/tests/cloud-connection.spec.ts`、`cloud-native.spec.ts` | 问卷全流程、截图、C1 兼容及真实 IPC 未授权检查 |
| `docs/benchmark-phase-c2.md`、`benchmark-phase-c-plan.md`、`implementation-plan.md`、`README.md`、`milestone-log.md` | 阶段说明、入口、当前状态与实际证据 |

没有新增依赖或修改模型调用协议。

## 验证及待办

- 3 个 C2 Rust 专项通过：版本/隔离/输入约束/不可变性/重开；8→9 备份与故意失败后重试；Guard 阻断未配置、操作中和已撤销状态。
- 8 个既有数据库专项通过：旧聊天数据与迁移校验保留、备份失败不升级、事务回滚、用途隔离、历史评测恢复等。只运行与新增迁移/存储相关的检查。
- 4 个浏览器检查通过：保留 C1 三项并新增画像流程；多领域、水平、任务、工具、权重保存、页间草稿、修改、重置、跳过。连接与画像 IPC 为明确的测试替身；真实数据库保存另由 Rust 检查覆盖。
- 2 个原生 WebView2 检查通过：画像读写无凭据不能绕过、固定题库及原页面可用；真实 SQLite 聊天取消/错误/重启兼容。使用隔离库和本地受控模型服务，没有真实云端调用。
- 类型检查、Rust offline check、Windows debug 桌面构建、格式检查通过。三种目标尺寸查看问卷实际截图，并补任务页/深色权重页截图。Vite 大块提示沿用现状。

本机证据在 `artifacts/phase-c2/`、`artifacts/phase-c2-browser-verified/`、`artifacts/phase-c2-layout-final/`、`artifacts/phase-c2-native/`。最后一次问卷专项是布局修订后的重复检查，不累计为新测试。这些结果不代表真实云端账号下的问卷完整原生端到端验收；真实账号、系统缩放、读屏以及 Docker 实测仍待后续环境验证。

停止于 C2 验收。确认后下一步为 C3：授权本地文件夹/ZIP 的只读导入、安全过滤和项目摘要；本轮没有扫描仓库、上传资料或生成题目。
