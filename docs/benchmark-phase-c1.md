# C1：云端连接与个性化评测准入交付

2026-10-25。前置复核与后续分片见 [Phase C 计划](benchmark-phase-c-plan.md)。本轮停在 C1，等待验收。

## 已交付

进入“评测 → 个性化评测”，先输入自有 API Key，读取服务商真实模型目录，再选择模型并连接。支持 OpenAI、Anthropic、Google Gemini，重新验证、更换服务商/密钥、取消、解除绑定及重启读取。浏览器预览禁用真实配置；React 不直接访问云端。

“已验证”严格指密钥认证及目录可见性。目录请求不能证明余额充足或模型拥有推理权限。OpenAI/Anthropic 的生成能力保持 unknown；Gemini 仅保留声明 generateContent 的条目，仍不代表推理实测通过。没有预填模型、上下文长度或价格。

标准页展示数据库读取的固定题库目录；运行和结果界面尚未交付，页面明确说明。既有单题 Ollama 接口保持可用。连接成功只展示准备就绪，不伪装 C2 问卷或 C4 生成已经实现。

## 实际文件

以下路径相对 `apps/desktop/`。

| 文件 | 变更 |
| --- | --- |
| `src-tauri/src/cloud/mod.rs` | 新增连接状态、限时验证、取消/删除互斥、Guard、5 个 Tauri 命令 |
| `src-tauri/src/cloud/provider.rs` | 新增三种官方模型目录协议、分页、限额和保守错误分类 |
| `src-tauri/src/cloud/vault.rs` | 新增 Windows CredReadW/CredWriteW/CredDeleteW、版本化单记录 |
| `src-tauri/src/cloud/tests.rs` | 新增 Mock Provider、安全存储和取消/超时等专项 |
| `src-tauri/src/lib.rs`、`src-tauri/build.rs`、`src-tauri/capabilities/default.json` | 注册服务和具体命令权限；未开放通用网络/文件/命令接口 |
| `src-tauri/Cargo.toml`、`src-tauri/Cargo.lock` | reqwest 启用 native-tls；windows-sys 凭据 API；zeroize；提交锁定解析结果时应一并保留 |
| `src/benchmark/BenchmarkPage.tsx` | 新增标准题库目录与个性化入口 |
| `src/benchmark/CloudConnection.tsx` | 新增连接引导表单、状态与错误、取消和后端准入调用 |
| `src/benchmark/cloud.ts`、`src/benchmark/benchmark.css` | 状态类型/中文文案、沿用现有 Soft Glass Tokens 的局部样式 |
| `src/App.tsx`、`src/navigation.ts` | 评测路由及个性化子视图；不读取 query 中的 allowed 标志 |
| `src/components/AppShell.tsx`、`src/components/NavigationIcon.tsx` | 评测导航入口与线条图标 |
| `tests/cloud-connection.spec.ts`、`tests/cloud-native.spec.ts`、`playwright.config.ts` | 浏览器替身/真实桌面专项与注册 |

文档新增本文、`docs/benchmark-phase-c-plan.md`；更新 `docs/README.md`、`docs/implementation-plan.md`、`docs/benchmark-implementation-plan.md`、`docs/benchmark-provider-design.md`、`docs/milestone-log.md`。

## 接口与存储

| 命令 | 输入 / 行为 |
| --- | --- |
| `cloud_connection` | 读取脱敏状态；不会主动联网 |
| `verify_cloud_connection` | `input: {provider, apiKey?, model?}`；有新 key 时原子替换当前连接，缺省 key 使用安全存储；读取目录，选中实际返回模型才允许通过 |
| `cancel_cloud_verification` | 中断请求并等待状态写入；处理响应完成与取消的竞争，返回最终状态 |
| `remove_cloud_connection` | 先取消再删除系统凭据；保留所有 SQLite 历史 |
| `personalized_access` | 从安全存储验证真实连接、模型、状态及时间；不接受前端布尔值作为凭据 |

返回 `state / provider / model / checkedAt / allowed / models`，不返回 key 或尾号。输入拒绝未知字段、自定义 Provider、空/过长/含空白的 key。允许完整的用户自有 key，不把格式检查当真实性验证。

状态分别表示未配置、验证中、待选模型、已验证、密钥无效、模型/目录无权限、明确额度不足、限流、网络不可用、服务异常、需重新验证和已取消。普通 429 只标限流；只有明确 quota 错误标额度不足。服务正文和底层异常不返回 UI。未识别服务错误保守标异常，不根据消息猜测余额。

安全凭据记录 version=1，包含当前 Provider、密钥、模型和最后状态/时间；使用当前 Windows 用户的 Generic Credential、本机持久化。配置与密钥一起原子写入，避免双存储事务。C1 只保存一组活动连接，更换即替换。没有 SQL 迁移，schema 仍为 8；既有 8 份迁移、9 个 v1/v2 数据文件及评分器源码保持字节不变。

首次解锁需要真实目录验证和模型选择。24 小时后要求重验证；上次中断在 verifying 状态时，重启转为需重新验证。网络失败保留 key 并关闭本次准入，不宣称密钥失效。系统凭据读取失败同样拒绝准入。Debug/release 凭据命名隔离；测试依据隔离目录另分命名空间，真实存储单测使用随机目标并删除。

前端调用后立即清空密码输入，页面离开时取消未完成验证；不写 localStorage/SQLite/快照/日志。Rust 关键临时缓冲和 Record 的密钥使用 zeroize，Win32 返回 blob 在 CredFree 前清理。JS 不可变字符串、IPC 和 HTTP 库内部副本无法承诺全部物理擦除，系统凭据也不抵御已控制当前 Windows 账号的恶意进程。

官方 HTTPS origin 固定，禁重定向；认证只在 Header，敏感 Header 标记不可调试展示。8 秒连接、单请求 20 秒、整体 30 秒；每页 1 MiB、最多 10 页/5000 项，无自动重试、无推理请求、无费用猜测。保留系统 TLS 证书验证；用户企业代理/证书链需实机验收。Anthropic 版本头固定 2023-06-01，依据其仍列出的[版本契约](https://platform.claude.com/docs/en/api/versioning)，不随模型名猜版本。

C1 没有生成、个性化任务创建或任务恢复接口，调用不存在的命令不能绕过 Guard。C4/C5 的每个新写入/恢复入口必须再次调用 Guard 并检查与请求内容绑定的费用/资料授权；不能仅依赖本轮曾验证通过。生成失败更新连接状态的接线随真正生成接口实现。

## 实际验证

| 检查 | 实际结果 |
| --- | --- |
| Rust 云端逻辑 | 7 个 Mock/解析/Guard 检查通过；含真实等待 30 秒的超时、并发验证拒绝、取消/删除、重建状态、错误分类、选择模型、脱敏及缺失存储 |
| Windows 凭据单测 | 1 个通过：随机合成凭据写入、重新打开读取、替换、删除；未使用真实 key |
| 浏览器专项 | 3 个通过：绑定/目录/Guard/清空输入/解绑/标准访问，网络失败及 4 尺寸×2 主题截图，取消后忽略晚到替身响应 |
| 原生专项 | WebView2 准入与原有聊天兼容性共 2 个通过；真实 IPC/SQLite，聊天使用受控本地 HTTP 替身 |
| 类型与构建 | TypeScript、Vite、Windows Tauri debug 无安装包构建通过 |
| 格式及范围 | cargo fmt、git diff --check；应用源码基线比对，评分器/题库/SQL 未改 |

Rust 的“重启”检查重建服务读取同一存储；Windows 测试重新创建凭据访问对象；原生新连接页面还验证了页面重载。真实账号绑定后退出进程再启动的云端端到端检查待真实环境验收，不把这些检查合称真实云端重启验收。

本机证据目录：`artifacts/phase-c1-browser-final2`（8 张实际浏览器截图）、`artifacts/phase-c1-native`（原生准入/导航、3 尺寸截图和聊天回归）、`artifacts/phase-c1-native-final`（最终构建重复 C1 原生检查 1/1）。截图是实际应用渲染；浏览器连接状态由明确的测试替身提供，不是供应商认证成功的证据。已审阅 1366 浅色和 1920 深色浏览器代表截图，以及最终 1600 原生截图。

运行过程中的失败也保留：初始 cargo check 因 reqwest query feature 未启用失败，改用现成 URL 编码；沙箱 TLS 拉依赖失败后在正常权限下载。凭据单测在沙箱登录令牌下不可用，正常权限重跑通过。首轮浏览器模型选择定位失败，补明确可访问名称及修正替身事件清理。新增原生测试误用 void 返回值导致一次类型检查失败，已修正。后续取消边界补测通过。不将这些初次失败写成通过。

## 待验收与风险

- 三家真实 API Key 的认证/权限/余额/企业网络、TLS 错误细分和供应商协议差异尚未实测；仅查询目录不保证后续推理可用。无需把 key 发给开发者，请在桌面页输入。
- C2–C5 未实施：画像、项目导入、生成/审核、费用预算、云端执行、多模型比较与结果 UI 均未伪造完成。
- 真实 Docker 隔离仍待环境；未下载模型、镜像或安装系统软件；无付费调用、发布包、Git 推送。
- 保留既有 Vite 大块提示。真实 150% 系统缩放与读屏未为新增页重做；本轮视口检查不等于系统缩放验收。

建议验收 C1 后再进入 C2；C4 前补真实账号目录验证，C5 前完成原 Runner 的串行调度/恢复与 Docker 实测。
