# 里程碑记录

## 2026-11-26 Phase 12：新建工作区「人群场景模板」与场景化首屏引导

- **后端场景预设原子创建与首屏推荐定制（独立 Codex/GPT 新会话 `01a215e2-3194-7410-ae9e-b713851eb718` 交付）**：在 `personal_assistant.rs` 新增 4 套人群场景分身预设（`study` 学习备考教练、`work` 职场项目搭档、`writing` 阅读写作编辑、`life` 私人生活管家，及 `blank` 空白无预设），并在 `db.rs` / `commands.rs` 扩展 `create_workspace(name, template)` 于同一 SQLite 事务内原子创建工作区并发布启用版本 1（未知模板在写入前拒绝不残留孤立工作区）。升级 `memory.rs` 的 `DailyFocus` 返回 `activeAssistantName` 并按启用场景定制 3 条空态推荐开场白。
- **前端新建工作区模板选择与空态引导（`workspace-templates.ts` + `App.tsx` + `EmptyState.tsx`）**：升级「创建工作区」对话框为 5 张可选场景卡片，选中后动态切换工作区名称示例并在创建完成时提示对应场景分身已启用；空新对话首屏展示当前启用场景模式与拖拽文档提示，并提供场景化一键追问按钮。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`（2/2 通过）、`cargo test --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib memory::tests`（6/6 通过，含 `workspace_scenario_template_activates_assistant_and_tailors_daily_focus`）及 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`（通过）。未运行真实模型推理及原生 WebView2 自动化截图回归。

## 2026-11-26 Phase 11：全窗口「拖拽文件即入库 / 即提问」

- **后端安全批量拖放导入（独立 Codex/GPT 新会话 `01a215d6-bb66-76d3-8b6c-58508a69404e` 交付）**：在 `sources.rs` / `commands.rs` 新增 `import_dropped_files` 命令（同步注册至 `lib.rs`、`build.rs`、`capabilities/default.json`），支持单次最多 20 个路径混合导入文档（PDF/PPTX/DOCX/XLSX/EPUB/TXT/MD/代码，返回生效 `nodeId`/`version`/`title`）与图片（PNG/JPEG/WebP/GIF 最多 4 张，纯 Rust Base64 编码校验）。图片与文档共用严格的本地磁盘路径校验、符号链接拒绝、`GetFinalPathNameByHandleW` 句柄防替换与敏感文件名过滤；单个非法文件/目录独立报错不阻断同批有效文件。同时优化文档标题推导自动去除 `.pdf`/`.pptx`/`.docx`/`.xlsx`/`.epub`/`.csv` 后缀。
- **前端全窗口拖放联动（`DropImportOverlay.tsx` + `App.tsx`）**：接入 Tauri v2 原生 `getCurrentWebview().onDragDropEvent`，拖入文件时浮出全屏半透明虚线提示层；在**对话页**松手自动将导入文档挂载为当前对话锁定上下文并把拖入图片贴入输入框，在**知识/图谱页**松手直接入库并刷新列表与导图（拖入图片自动跳回对话页贴入）。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`（2/2 通过）、`cargo test --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib sources::tests`（4/4 通过，覆盖文档与 PNG 批量导入、敏感文件与目录拦截、去重与重复导入幂等）及 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`（通过）。未运行真实模型推理，Windows 资源管理器物理拖拽入 WebView2 窗口待实机复核。

## 2026-11-26 Phase 10：全人群多格式文档零依赖解析（PDF / PPTX / XLSX / EPUB）

- **后端多格式提取引擎（独立 Codex/GPT 新会话 `01a215ba-d3c7-7fe2-95f3-da110c40140b` 交付）**：新增 `src-tauri/src/doc_extract.rs`，零新增 Rust crate 依赖，实现：
  - **`.pptx`（课件/汇报）**：按幻灯片数字页序提取正文与演讲者备注，输出带 `## 第 N 页` 的结构化文本；
  - **`.xlsx`（办公表格）**：解析 `workbook.xml`、`sharedStrings.xml` 与各工作表，转为标准 Markdown 表格；
  - **`.epub`（电子书）**：按 OPF `spine` 阅读顺序剥离标签输出干净章节正文；
  - **`.pdf`（课件/论文/合同/报告）**：纯 Rust 实现 RFC 1950/1951 DEFLATE 解压（覆盖存储块、固定/动态哈夫曼块，单流上限 16 MiB）与 `/ToUnicode` CMap 中英文文字提取；对纯扫描件或加密 PDF 给出明确诚实提示；
  - 接入 `sources.rs` 与原生文件选择器，二进制文档原始读取上限提升至 32 MiB，提取纯文本与 XML 解压保持防炸弹上限；单文件导入与授权文件夹同步自动生效。
- **前端资料界面同步**：更新 `SourceBrowser.tsx` 与 `KnowledgeEditor.tsx` 支持格式说明与自动去后缀标题推导。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`（2/2 通过）、`cargo test --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib doc_extract::tests`（3/3 通过）、`--lib sources::tests`（3/3 通过）及 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`（通过）。未运行真实模型推理，真实复杂排版商业 PDF/PPTX 样本与原生 WebView2 选择框待实机试用复核。

## 2026-11-25 专属助手起名引导、全站称呼联动与消除启动黑框

- **消除 Windows 启动黑色控制台窗口（独立 Codex/GPT 新会话 `01a2124c-1dd1-7092-ae66-1c0fb0b7a55c` 交付后端）**：将 `src-tauri/src/main.rs` 的子系统属性由 `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` 调整为 `#![cfg_attr(windows, windows_subsystem = "windows")]`，桌面快捷方式启动开发版与正式版 `cortexos-desktop.exe` 均不再弹出全黑命令行窗口。
- **Schema 38 助手自定义称呼与 Prompt 注入**：新增迁移 `0038_assistant_name.sql`（`global_persona.assistant_name`），在 `memory.rs` 校验长度（≤ 40 字、无控制字符）、写入 `GlobalPersona::format_prompt()` 让模型明确感知用户起的称呼、纳入 `DailyFocus.assistantName`，保存全局画像与沉淀规则时广播 `global-persona-updated` 事件，并在 `capabilities/quickbar.json` 为浮窗授予最小权限 `allow-get-global-persona`。
- **首次启动起名弹窗与全站 6 处称呼联动**：
  - `App.tsx` 在首次启动且未起名时弹出轻量「想给你的专属 AI 起个名字吗？」对话框（支持快捷候选名或「暂不起名」跳过），`GlobalPersonaPanel.tsx` 支持随时修改；
  - `QuickBar.tsx` 不再沿用当前工作区/知识板块名，改为「秒问 {助手名}」「向 {助手名} 提问」「{助手名} 正在结合当前工作区记忆思考…」，并与 `ChatMessage.tsx`（回答署名）、`ChatComposer.tsx`（输入框占位与默认分身）、`EmptyState.tsx`（欢迎语与今日聚焦副标题）、`AppShell.tsx`（品牌提示）实时同步。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`（2/2 通过）、`cargo test --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib memory::tests`（5/5 通过）、`--lib upgrades_existing_database_with_consistent_backup_and_checks_all_migrations`（1/1 通过）及 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`（通过）。未运行真实模型推理，Windows 双击无黑框启动与首次起名弹窗视觉待实机复核。

## 2026-11-25 Phase 9：Windows 原生 Alt+Space 全局呼出随手记与秒问浮窗

- **零新增依赖 Win32 全局热键与浮窗生命周期（独立 Codex 新会话 `01a2121d-d924-7623-a33f-7b295bbd0bb3` 交付后端）**：新增 `src-tauri/src/quickbar.rs` 与最小权限窗口能力配置 `capabilities/quickbar.json`，通过 Windows `user32.dll`（`RegisterHotKey` / `UnregisterHotKey` / `GetMessageW` / `PostThreadMessageW`）在独立消息泵线程监听 `Alt + Space`（支持 `<data_dir>/quickbar.json` 持久化开关与热键占用冲突提示，并在自动化测试环境自动隔离不抢占宿主热键）。关闭浮窗自动转为隐藏，退出主窗口时同步投递 `WM_QUIT` 释放热键并销毁浮窗，杜绝残留后台僵尸进程。
- **无边框置顶 `#/quickbar` 双模交互（`QuickBar.tsx` + `QuickBarSettingsPanel.tsx`）**：`main.tsx` 对 `#/quickbar` 独立轻量挂载 `QuickBar`：
  - **「⚡ 秒问 AI」**：直接向当前活跃工作区提问并流式展示回答（窗口高度自适应展开至 416px），支持一键「在主窗口继续深聊」唤醒主窗口并跳转对应会话；
  - **「📌 随手记」**（按 `Tab` 或输入 `/` 前缀秒切）：输入灵感/待办按 `Enter` 直接调用 `quickbar_save_note` 写入生效知识并同步 FTS5 索引，向主窗口广播 `quickbar-knowledge-saved` 后自动收起浮窗；「设置 → 我的助手」支持随时开关全局热键或点击「立即呼出浮窗」。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`（2/2 通过）、`cargo test --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib quickbar::tests`（1/1 通过，覆盖配置持久化与随手记入库即刻可检索）及 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`（通过）。未运行真实模型推理及跨进程物理按键/多显示器 WebView2 实机自动化测试。

## 2026-11-25 Phase 8：主动式「今日聚焦」工作台晨间卡片

- **工作区上下文零延迟聚合（独立 Codex 新会话 `01a2120c-d4b9-7ba2-ad66-e5a10f29a58a` 交付后端）**：在 `memory.rs` 新增 `read_daily_focus` 命令（同步注册至 `lib.rs`、`build.rs`、`capabilities/default.json`），纯本地聚合当前工作区近期未归档活跃会话及一句话摘要、近 48 小时新增/更新的启用资料文件（含关联知识节点 ID）、待确认记忆候选数量、最新一期已生成的定时新闻简报要点（兼容 `ready`/`completed` 状态）与即将到期的提醒，并动态生成 3 条上下文相关的推荐追问词。
- **首屏工作台升级（`EmptyState.tsx`）**：空工作区与首次启动保持清爽引导；当工作区存在历史沉淀时，新对话首屏自动切换为自适应双栏「今日聚焦」工作台卡片（「接着上次聊」一键续聊、「近期资料与待办记忆」直达卡片或收件箱、「今日简报与提醒」支持一键围绕本期简报深聊），并在监听到 `memory-candidates-updated` 时自动刷新待确认计数。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`（2/2 通过）、`cargo test --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib memory::tests`（3/3 通过，覆盖空工作区、归档排除、资料与简报聚合及跨工作区隔离）与 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`（通过）。未运行真实模型推理及原生 WebView2 自动化截图回归。

## 2026-11-25 Phase 7：划词追问/存卡悬浮气泡与 Ctrl+V 粘贴截图多模态问答

- **阅读区划词悬浮气泡（`SelectionPopover.tsx`）**：在 `ChatMessage.tsx` 与 `SourceBrowser.tsx` 接入无侵入划词快捷胶囊（选中 4–8000 字时自动浮出，点击不丢失选区）：
  - **「💬 追问这段」**：自动将选中内容（资料文件自动附带《文件名》与起止行号）按 Markdown 引用块追加至当前会话输入框并聚焦光标；
  - **「📌 存为知识卡」**：在回答中一键调用 `save_knowledge` 保存并自动定位连续原文片段以保留 `sourceMessageId` 溯源；在资料文件中自动解析选中行区间（`data-source-line`）并调用 `save_source_knowledge` 直接保留文件版本与精确起止行溯源。
- **Schema 37 多模态截图问答（独立 Codex 新会话 `01a211e9-0fa3-7ee0-bbdd-a27e5d0e91c3` 交付后端）**：新增迁移 `0037_message_images.sql`（独立表持久化每条消息最多 4 张 PNG/JPEG/WebP/GIF 图片 Base64），扩展 `models.rs`、`db.rs`、`commands.rs`、`ollama.rs`、`cloud/runtime_protocol.rs` 与 `cloud/runtime_connections.rs`，支持 OpenAI 兼容服务（`image_url` Data URI）、OpenAI 官方（`input_image`）、Anthropic（`base64 image`）、Gemini（`inlineData`）及本机 Ollama（`images`）五种协议编码与本机备用回退附件保留，图片按固定视觉预算（1024 单位/张）估算云端输入上限与预留额。前端 `ChatComposer.tsx` 支持 `Ctrl+V` 直接粘贴剪贴板截图或选图、前端 Canvas 自适应压缩（最长边 ≤ 1568px）、待发缩略图预览条，消息流支持缩略图与点击弹窗放大查看。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`（2/2 通过）、`cargo test --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib multimodal`（2/2 通过）、`--lib chat_with_pasted_image`（1/1 通过，回环 HTTP 替身校验多模态流式请求与图片落盘回读）、`--lib cloud_chat_loopback_preserves_roles_and_excludes_private_lineage`（1/1 通过）、`--lib upgrades_existing_database_with_consistent_backup_and_checks_all_migrations`（1/1 通过）及 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`（通过）。未运行真实本地/云端多模态模型推理及原生 WebView2 自动化截图回归。

## 2026-11-25 Phase 6：AI 隐式记忆提炼、冲突检测与版本演进

- **云端对话后置隐式记忆提炼（Codex 桌面独立新会话 `01a211c9-eb18-71b3-a8ad-3e39895bc351` 交付后端）**：在 `cloud/runtime_connections.rs` 新增 `extract_implicit_memories`，于云端聊天正常完成且工作区开启 `syncMemoryAndHistory` 时后台异步提炼 0–2 条长期背景、沟通规则或项目变更候选。上下文组装 `cloud_memory_context` 严格排除 `local_only` 私密节点、本地备用回退轮次并经过 `privacy::check_workspace` 隐私校验；写入新候选后发出 `memory-candidates-updated` 事件驱动前端实时刷新。
- **Schema 36 旧记忆冲突检测与一键版本演进**：新增迁移 `0036_memory_conflict_evolution.sql`（扩展 `replaces_node_id`、`replaces_value`、`conflict_reason`），同时升级本地零开销规则提取与词项重叠冲突识别。前端 `MemoryCandidatesBar.tsx` 对冲突候选展示「记忆演进」警示标签、旧记录删除线对照与「更新并保留历史」操作；确认采纳后，项目事实自动在原知识节点上递增 `effective_version` / `latest_version`、写入审批事件建立 `knowledge_version_lineage` 版本演进链并刷新 FTS5 索引（旧版本自动退居历史不再干扰召回），全局偏好与长期规则原位替换并去重。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`（2/2 通过）、`cargo test --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib memory::tests`（2/2 通过，覆盖冲突检测、跨工作区拒绝、`local_only` 隔离、版本演进链与 FTS5 索引替换）、`cargo test --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib upgrades_existing_database_with_consistent_backup_and_checks_all_migrations`（1/1 通过）及 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`（通过）。未运行真实本地/云端模型推理及原生 WebView2 自动化截图回归。

## 2026-11-25 Phase 5：2D 思维导图知识图谱、框选专题深聊与智能连线推荐

- **2D 思维导图画布重构**：将 `GraphViewport.tsx` 从 Three.js 3D 球体视图替换为零第三方运行时依赖的 2D 思维导图画布（SVG 贝塞尔曲线 + 清晰圆角节点卡片），支持「思维导图（树状层级自动排版 + `−/+` 子树折叠展开）」与「自由白板（手拖节点保存坐标）」双模式切换、滚轮无极以光标为中心缩放、可打断平滑聚焦动画；`GraphViewport` 分包体积从 `941.15 kB` 降至 `10.66 kB`（缩减约 930 kB）。
- **多选 / 框选节点驱动专题对话**：支持 `Shift+拖拽` 框选与 `Shift/Ctrl+点击` 多选节点，浮出操作栏支持一键「围绕选中节点深聊」「考考我（考点自测）」「串联生成综述」，自动创建新会话、挂载所选生效知识版本并开启图谱关联。
- **智能关联推荐（独立 Codex 新会话交付后端）**：在 `knowledge.rs` / `commands.rs` 新增 `suggest_knowledge_links` 命令（权限同步注册至 `lib.rs`、`build.rs`、`capabilities/default.json`），基于有效版本标题提及与词项重叠自动推荐最多 4 条尚未连线的同工作区节点及建议关系；前端图谱详情侧栏新增 `SuggestedLinksPanel` 支持一键建立连线。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`（2/2 通过）、`cargo test --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib suggests_related_knowledge_links_excluding_existing_edges`（1/1 通过）与 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`（通过）。未运行真实模型推理及原生 WebView2 自动化截图回归。

## 2026-11-25 个人助手审查修复

- 公开网页抓取在建立连接时校验 DNS 返回的整组地址，连接使用同一组地址。重定向沿用相同策略，网页正文抓取使用直连。
- 文件夹绑定与后台扫描分开。数据库迁移 0034 保存扫描位置和自动生成知识的归属，旧数据按原始导入记录回填。同步保留停用授权、手工摘录和人工编辑版本，每次每个文件夹最多读取 60 个文件，后续同步从上次位置继续。重新进入工作区会再同步，有改动时刷新知识列表和图谱，并显示跳过、待续和失败信息。
- 普通问答只使用生成模型，自动路由和手动选模都跳过本次用不到的向量与重排连接。周报按来源授权和生效日期筛选知识。记忆候选与摘要在同一保存点写入，提取失败会记录事件并保留已完成的聊天回答。
- 浏览器验收使用模拟服务。执行 `npm.cmd --prefix apps/desktop run build`，以及在 `apps/desktop` 运行 `npm.cmd run test:smoke -- --project=edge runtime-cloud.spec.ts runtime-profiles.spec.ts --output=../../artifacts/review-fixes-final`。更新工作区选择器、隐私设置和后台同步的验收断言后，最终 5 项通过；查看了窄屏深色对话截图。
- 后端使用 `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --offline --lib <过滤器>`，分别检查 `sources::tests::watched_folder`、`chat_router_prices_only_workspace_candidates_and_honors_task_preferences`、`public_page_addresses_exclude_special_use_ranges` 和 `upgrades_existing_database_with_consistent_backup_and_checks_all_migrations`，合计 5 项通过。离线 SQLite 样例另验证了旧库归属回填、周报资格筛选和记忆写入失败回滚。`git diff --check` 通过。全仓 Rust 格式检查仍报告既有格式差异。
- 本轮更新原生诊断用例的预期数据库版本至 34。未执行原生 WebView2 诊断导出、真实网页抓取或真实模型调用。

## 2026-11-24 专属个人 AI 演进（Phase 4 收口：网页正文抓取、持久化文件夹同步、对话栏助手切换）

- **安全网页正文抓取与一键入库**：在 `web_search.rs` 新增 `fetch_web_page` 命令，复用 `result_url` 并在自定义重定向策略的每一跳拦截内网/回环/非 http(s) 地址（SSRF 防护），限制 1 MiB 与 15 秒超时，提取标题与纯文本正文；支持直接调用 `save_knowledge` 生效并同步进入 FTS5 检索。对话联网来源卡片新增「抓取并存入知识」按钮，资料文件对话框新增公开网页链接直接抓取入库入口。
- **Schema 33 授权文件夹持久化与自动同步**：新增迁移 `0033_watched_folders.sql`，持久化记录工作区已授权的本地同步文件夹路径与最近同步时间；新增 `list_watched_folders`、`sync_watched_folder`、`remove_watched_folder`、`sync_workspace_watched_folders` 命令。用户同步文件夹时自动绑定，资料文件面板可查看已绑定目录、一键重新同步或解绑；切换进入工作区时后台静默增量同步并在有新增/更新时提示。
- **对话页快捷切换助手分身**：在对话输入栏模型选择器旁新增助手分身下拉菜单（当工作区存在已发布分身版本时显示），支持在「默认助手」与各已发布分身间一键热切换。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`、`cargo test --offline --locked --lib`（`web_search::tests` 2 项、`watched_folders_upsert_sync_and_remove` 1 项、`upgrades_existing_database_with_consistent_backup_and_checks_all_migrations` 1 项、`global_persona_and_memory_extraction_cycle` 1 项，全部通过）及 `cargo build --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`。未执行真实外网网页抓取、本地/云端模型推理或原生 WebView2 截图回归。

## 2026-11-24 专属个人 AI 演进（Step 0 拆库 + Phase 1–3）

- **评测独立拆库**：先将基线改动归档提交 `c56f10e`，经由本机 Codex 在 `marksimZ` 账号下创建公开仓库 `CortexOS-Bench`，完整校验推送 26 个分支与 2 个标签并克隆至 `D:\vibecoding\CortexOS-Bench`；原 `origin` 保持不变。
- **Phase 1（瘦身、云地贯通、资料直连）**：主导航收敛为「对话 / 知识 / 图谱 / 设置」四栏，移除前端 `src/benchmark/`、3D 执行轨迹图、7 节点演示切换器及快照/实验分支选择器。Schema 30 增加 `knowledge_local_only` 单条“仅限本地”隐私锁与工作区常驻同步策略 `syncMemoryAndHistory`（默认开启）；云端对话支持 OpenAI 兼容 SSE 流式输出、多轮历史、工作区个人助手指令与聊天栏直接切换本机/云端模型；仅在发消息所用模型与上一轮不同时在对话流插入行内切换提示条。用户手动录入知识与导入文档默认直接生效并同步 FTS5 检索。
- **Phase 2（双层记忆与进化闭环）**：Schema 31 新增 `global_persona`、`memory_candidates`、`conversation_summaries`。全局「关于我」画像与跨会话近期情景摘要在本地及开启同步的云端对话中统一注入；每轮完成后零显存开销提炼偏好、长期规则与项目事实，在对话页与知识页提供待确认记忆收件箱（一键记住 / 编辑 / 忽略），并在用户消息下方支持一键「沉淀为长期规则」。
- **Phase 3（多格式同步、周报草稿与引导式人设）**：零新增依赖复用锁定 `zip` + `quick-xml` 支持 `.docx` 及常见代码/配置文件导入，新增授权文件夹增量同步 `import_source_folder`；任务中心新增近 7 天本地进展周报草稿生成器；「设置 → 我的助手」增加引导式角色分身预设与风格快捷标签。
- **实测与未验证范围**：执行 `npm.cmd --prefix apps/desktop run build`、`node --experimental-strip-types --test apps/desktop/tests/unit/navigation.test.ts`、`cargo test --offline --locked --lib`（覆盖 Schema 31 全量迁移、记忆进化闭环、云端兼容路由与隐私校验）及 `cargo build --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`，均通过。未运行真实本地/云端模型推理及自动化 WebView2 截图回归。

## 2026-11-24 国内云端模型预设

- 工作区的“设置 → 云端模型”新增 DeepSeek 官方、阿里云百炼（北京）和硅基流动。复用已有兼容协议、按连接绑定的 Windows 凭据与模型用途验证。切换服务商清空已输入的密钥。运行方案与聊天显示连接、预设和模型，每次发送沿用云端授权范围。配置按既有格式保存，无数据库迁移。
- DeepSeek 官方提供生成验证，已知支持关闭思考的模型在固定短文本验证时采用非思考模式。日常聊天采用服务商默认设置。百炼预设提供生成和向量验证，硅基流动另提供重排序验证。目录失败保留错误，可手填模型标识。云端聊天使用非流式接口，能力通过验证后才出现在相应运行方案的模型选择中。
- 桌面目录执行 `npm.cmd run build`，前端类型检查与构建通过。根目录执行 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 和 `git diff --check`。专项协议检查 `cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib runtime_cloud_compatible_routes_against_http_fixture` 一项通过，回环服务核对三家目录与生成请求的地址路径和认证、输出解析及硅基流动向量与重排。
- Edge 模拟 IPC 在桌面目录运行 `npm.cmd run test:smoke -- --project=edge runtime-cloud.spec.ts runtime-profiles.spec.ts --output=../../artifacts/cloud-presets/edge-final`，四项通过。覆盖各预设保存、模型选择、知识库用途与显式验证、同会话逐次确认及切换运行方案。截图位于 `artifacts/cloud-presets/`。
- 原生 WebView2 在桌面目录设置 `CORTEXOS_NATIVE_ONLY=1` 后执行 `node node_modules/@playwright/test/cli.js test --project=desktop cloud-native.spec.ts -g "domestic cloud" --output=../../artifacts/cloud-presets/native`，一项通过。使用隔离数据和占位凭据，验证三家连接可并存、重启仍可从凭据管理器读取、工作区分离及清理。
- 厂商账号实网试聊与实际计费仍待具备对应 API 凭据后验证。本次没有使用真实凭据或调用模型。开发版停在首批预设验收点。

## 2026-11-24 一句话创建定时新闻简报

- 聊天识别定时资讯请求，预览查询词、频率和本地时间，确认后保存。到期读取 Bing News 公开 RSS，每期在工作区保存至多五条标题、来源摘要与原文链接。任务中心可改期、暂停、取消以及展开历史。应用运行期间执行调度，重启沿用提醒补记规则。
- Schema 29 按计划和到期时间去重，保存当期查询词、结果与错误。网络请求位于数据库事务之外；上次退出时仍在查询的记录显示中断。代码复核修正了前端查询词字段反序列化以及普通提醒历史读取可空状态的问题。
- 桌面目录执行 `npm.cmd run build`、`npm.cmd run format:check`，根目录执行 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 和 `git diff --check`。Edge 模拟 IPC 检查 450×768 下的简报、空结果、失败和无历史四种界面。
- Windows WebView2 使用隔离数据库与本机 RSS 替身，在桌面目录设置 `CORTEXOS_NATIVE_ONLY=1` 后运行 `node node_modules/@playwright/test/cli.js test --project=desktop reminders-native.spec.ts --output=../../artifacts/scheduled-briefs/native`，两项通过。关闭测试实例后调整隔离计划时间以验证到期，覆盖保存、空结果、服务失败、重启去重、中断恢复、工作区隔离与普通提醒回归。本片未运行单元测试或调用模型。真实 RSS 连通性见同日上一切片。
- 当前验收点已完成。后续工作流为从工作区已确认进展生成周报草稿。

## 2026-11-24 免密钥公开新闻与新版前端验收

- 聊天复用现行 Apple × Linear 输入区和来源列表。未配凭据时直接读取 Bing News 公开 RSS；已配置 Brave 的工作区可在两种服务间选择。配置读取失败时明确提示，可重读或手动选择公开新闻。结果和随问题发送的参考文字标明实际来源服务。Rust 从固定 HTTPS 端点读取 RSS，复用锁定的 quick-xml，解析标题、原文链接、摘要并过滤重复项。无需新增 Tauri 权限或数据库迁移。
- 用通用查询探测发现带 `mkt=zh-CN` 的端点在当前网络重定向首页，最终请求仅携带 `q` 和 `format=rss`。独立 Python HTTP 读取“人工智能”取得 HTTP 200 和 12 个 RSS 条目。
- 在桌面目录运行 `npm.cmd run build`，含类型检查；`npm.cmd run format:check` 初次标出 API 文件，执行 `node node_modules/prettier/bin/prettier.cjs --write src/api/web-search.ts` 后复验通过。根目录执行 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`、`git diff --check`。
- Edge 模拟 IPC：Vite 1427 与临时脚本 `node .public-news-browser-check.mjs` 在 1366×768、450×768 核对无密钥、配置／移除 Brave、主动切换、空结果和错误。新版 Windows WebView2：在桌面目录设置 `CORTEXOS_NATIVE_ONLY=1`、`CORTEXOS_PUBLIC_NEWS_LIVE=1` 后执行 `node node_modules/@playwright/test/cli.js test --project=desktop web-search-native.spec.ts --output=../../artifacts/workflow-public-news/native`，2 项通过。替身检查无凭据请求、实体与原文 URL 解析及 Brave 回归。真实公开查询在未配置密钥、无模型的工作区显示五条来源，已核对 1366×768 浅色与 850×640 深色截图。未运行单元测试或调用真实模型。
- 本切片停在交互式新闻读取验收点。定时简报尚待接入；来源覆盖与语言取决于上游返回，本片不抓取全文。

## 2026-11-23 聊天联网资讯查询

- 在“设置 → 我的助手 → 网页搜索”按工作区管理 Brave Search 密钥。三个新增 IPC 命令分别读取配置状态、保存／移除凭据及执行查询。生产搜索仅向固定的 HTTPS 搜索端点发送查询，返回最多五条去重的标题、原文链接和摘要。应用内不抓取结果网页全文。普通发送、提醒和联网查询各有独立入口；“依据来源回答”沿用当前助手与云端披露，聊天历史保留实际附带的来源节选。
- 在桌面目录执行 `npm.cmd run build`（含类型检查）。`npm.cmd run format:check` 初次提示新 API 文件格式，使用 `node node_modules/prettier/bin/prettier.cjs --write src/api/web-search.ts` 后复验通过。根目录 `git diff --check`、`cargo check --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml` 与 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 通过。
- Edge：Vite 1427 端口、临时脚本 `node .web-search-browser-check.mjs --chat` 和模拟 IPC 验证 1366×768 与 450×768 下的密钥配置、空结果、读取失败、勾选来源与发送失败保留草稿。代码级专项检查覆盖默认 4K 配置下五条长中文摘要的截取与超长链接省略。
- Windows WebView2：桌面目录设置 `CORTEXOS_NATIVE_ONLY=1`，执行 `node node_modules/@playwright/test/cli.js test --project=desktop web-search-native.spec.ts --output=../../artifacts/workflow-web-search/native`，1 项通过。隔离凭据与两个本地协议替身核对工作区边界、仅发送本题、近期查询参数、URL 去重、可空摘要、429 反馈、模型接收与持久历史一致。浏览器及原生截图在 `artifacts/workflow-web-search/`。
- 本片未使用真实 Brave 账户、执行模型推理或运行单元测试。实际搜索质量与在线额度待使用有效密钥核验。下一步为定时简报复用搜索；本片停在交互式查询验收点。

## 2026-11-23 个人助手版本

- 设置中的“我的助手”支持按工作区编辑草稿、发布并启用、查看和切换历史版本、恢复内置默认。Schema 28 保存版本与生效指针，四个主窗口命令用于本工作区读取、保存、发布和启用。聊天请求组装计入指令预算；本地生成保留实际系统消息与助手标识，处理记录展示该次选择。云端生成与固定检索评测沿用内置指令。助手草稿缓存沿用备份的工作区归属过滤。已确认聊天没有联网搜索和页面抓取工具，订阅资讯与搜索分期写入实施计划。
- 在桌面目录运行 `npm.cmd run build`（含类型检查）。`npm.cmd run format:check` 首次发现新 API 文件格式问题，执行 `node node_modules/prettier/bin/prettier.cjs --write src/api/assistant.ts` 后复验通过。根目录 `git diff --check`、`cargo check --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml` 和 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 通过。SQLite 内存专项覆盖版本、默认值、工作区外键与级联清理。
- Edge 模拟 IPC：桌面目录 `node node_modules/vite/bin/vite.js --host 127.0.0.1 --port 1427 --strictPort` 和临时脚本 `node .assistant-browser-check.mjs`，核对 1366×768 和 450×768 下草稿恢复、错误重试、版本切换及工作区边界。截图在 `artifacts/personal-assistant/`。
- Windows WebView2：在桌面目录设置 `CORTEXOS_NATIVE_ONLY=1` 后执行 `node node_modules/@playwright/test/cli.js test --project=desktop personal-assistant-native.spec.ts --output=../../artifacts/personal-assistant/native-final`，1 项通过。首次检查的固定 HTTP 替身缺少 GGUF 元数据，被已有模型来源校验拒绝；补齐替身元数据后，从 UI 发布 v1、保存不同的草稿，依次核对 v1、v2、回退 v1、内置默认的四次真实 HTTP 请求、历史快照与隔离。截图位于该输出目录。未执行单元测试、真实推理或外部资讯请求。
- 本片达到本地请求与版本管理验收点。输出质量、真实模型负载及资讯来源接入尚未验证。下一工作流交付点为定时生成带依据的草稿。

## 2026-11-22 工作流首版第一切片：应用内定时提醒

- 对话支持常见中文提醒指令、本机时间与可编辑草案预览；确认才写入。任务中心支持查看、改期、暂停、恢复、取消及最近 100 条提醒记录。Schema 27 增加按工作区归属的计划和运行记录；主窗口新增的三个命令仅用于列出、保存和修改提醒。进程内调度每五秒检查，单个到期时刻以事务和唯一约束去重。退出或休眠后回到应用时，五分钟以内可补发最近一期，多个旧周期合并记为错过。
- 编译与静态检查：在桌面目录执行 `npm.cmd run build`（含 TypeScript）、`npm.cmd run lint`、`npm.cmd run format:check`；根目录执行 `git diff --check`、`cargo check --offline --locked --manifest-path apps/desktop/src-tauri/Cargo.toml` 和 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`。均通过；ESLint 仍有警告，Vite 提示部分分包较大。
- 浏览器检查：在桌面目录以 `node node_modules/vite/bin/vite.js --host 127.0.0.1 --port 1427 --strictPort` 启动 Vite，临时脚本 `node .reminder-browser-check.mjs` 以模拟 IPC 在 Edge 核对 1366×768 与 450×768 的编辑、空状态、读取失败与恢复、取消确认、工作区隔离及到期提示，截图在 `artifacts/workflow-reminders/`。另以 Python SQLite 内存库核对外键及去重约束。
- Windows WebView2：桌面目录设置 `CORTEXOS_NATIVE_ONLY=1`，运行 `node node_modules/@playwright/test/cli.js test --project=desktop reminders-native.spec.ts --output=../../artifacts/workflow-reminders/native`，1 项通过。隔离数据目录与不可用模型地址下，普通问题保持离线限制，工作日提醒能预览且确认前数据库为空；原生界面实测保存、暂停、改期、恢复、取消；IPC 验证跨区隔离。实际未来的单次提醒跨重启仅交付一次，隔离库内预置的逾期样例记录为错过，再次重启两条历史不重复。已查看原生草案、任务中心及浏览器截图。未运行单元测试或调用模型。
- 本片停在人工体验验收。下一片为可发布的个人助手版本，其后接入定时生成周报草稿。未实测物理休眠、切换系统时区或夏令时跃迁，完全退出后持续唤醒属于后续后台运行范围。

## 2026-11-22 H5 Windows x64 公开预发行

- 从已验收的 `350ef8a` 建立独立构建目录。先运行 `npm.cmd run build`，再运行 `scripts/release-dry-run.ps1 -Version 0.2.1-rc.4`。最终 NSIS 安装器为 7,017,245 字节，校验 11 项安装文件和 39 个内嵌资源路径，Authenticode 状态为 `NotSigned`。
- 独立依赖副本的清单纳入 85 项 npm 和 262 项 Rust 运行依赖。随包声明和四份 MPL 源码归档通过一致性核对。候选包 SHA-256 为 `1CCA4EACFE1582D15A5A58C36638FAD423815336249076D9C33DB3979141298D`。
- Windows 沙盒用最终包通过 4 项全新安装、首次启动、重装保留和显式卸载检查。从 `0.2.0` 升级的最终包复验在沙盒共享目录中断后终止，保留未验证。上一候选包的升级证据见 [H4 安装矩阵](release/h4-install-upgrade-validation.md)。
- 经维护者批准公开此次预发行。标签指向 `350ef8a`，GitHub 的安装器、校验文件和两份第三方材料共 4 个附件已核对，公开下载安装器的 SHA-256 与本地一致。发布记录为 `artifacts/h5-release-src/artifacts/rc4-final/github-publication.json`。 [GitHub Release](https://github.com/marksimZ/CortexOS/releases/tag/v0.2.1-rc.4)

## 2026-11-21 Phase H5 产品体验验收

- 在空工作区加入本机、云端、恢复及跳过引导。标题栏加入任务中心与按需帮助。任务中心复用队列与历史记录，保留现有个性化评测准入。
- 首轮 WebView2 内部走查 13 项通过。复跑历史定位与相关流程 7 项通过。Edge 的 2 项工作区切换回归定位为设置分类丢失，修复后 5 项通过。最终构建的引导和队列回归 3 项通过。原生与浏览器均在非活动 Windows 桌面执行。
- `npm.cmd run build`、`npm.cmd run lint`、`npm.cmd run format:check`、离线锁定的 `cargo check` 和包含 custom-protocol 的桌面构建通过。最终 ESLint 错误检查通过。完整命令、证据边界、场景耗时和体积对照见 [H5 验收](productization/h5-acceptance.md)。
- Phase H 开发交付收口，停在 H5 用户验收点。正式发布继续按 H4 条件处理。

## 2026-11-21 Phase H4 隔离 Windows 安装验收

- 重启后启用沙盒。在真实 Windows 11 x64 来宾发现并修复两处问题。rc.1 误把构建机绝对前端路径识别为外部 URL，rc.2 在可执行文件位置被同名目录阻挡时仍返回成功。修复文件为 `scripts/release-dry-run.ps1`、Tauri NSIS 配置及 `windows/installer-hooks.nsh`。
- `release-dry-run.ps1 -Version 0.2.1-rc.3` 构建通过，36 项前端嵌入路径、NSIS 11 项解包、SHA-256、第三方材料与敏感模式检查通过。`updates-native.spec.ts` 对实际候选文件复跑一项，通过。
- `prepare-windows-release-sandbox.ps1 -WebView2Installer` 生成隔离输入，`windows-release-guest.mjs` 的全新安装 4 项和旧版升级 3 项通过。确认真实 v0.2.0 从 Schema 7 升为 26，保留有效知识与可读备份，错误安装路径返回 2。来宾均已关闭。
- [发布准入](release/h4-release-checklist.md)保留 No-Go。项目授权、正式 Authenticode、正式更新分发和物理多屏及 DPI 仍待确认。H4 停在验收点。

## 2026-11-21 Phase H4 发布工程与隔离安装准备

- 延用正式 NSIS 产品标识和安装范围。接入本机手动触发的稳定版、预览版更新，要求包签名与签名内版本均匹配。统一关于页及新记录使用实际应用版本。
- 新增 `scripts/release-dry-run.ps1`、`scripts/release_inventory.py` 与 Windows 沙盒验收入口。首份本地候选完成生产构建、7-Zip NSIS 归档完整性检查、11 项解包白名单及两种签名状态读取。正式分发保留 No-Go。
- `npm.cmd run typecheck`、锁定离线 Rust 检查及调试桌面构建通过。非活动桌面的 `updates-native.spec.ts` 通过 1 项，含稳定版与预览版、服务失败、完整性和版本签名校验。证据与范围见 [H4 发布工程](release/h4-release-engineering.md)。
- 经批准启用 Windows 沙盒，Windows 返回需要重启。全新安装、旧版升级、卸载和正式更新安装验收仍待来宾环境。见 [安装矩阵](release/h4-install-upgrade-validation.md) 与 [发布准入](release/h4-release-checklist.md)。
- `release-dry-run.ps1` 从 `dcad612` 完成正式标识的 `0.2.1-rc.1` 候选，6,485,232 字节，SHA-256 已记录。对该实际文件复跑 `updates-native.spec.ts`，1 项通过。候选材料覆盖 262 项 Rust 运行依赖与 85 个前端包。`prepare-windows-release-sandbox.ps1` 已生成网络关闭的 `fresh.wsb` 和 `upgrade.wsb`，等待重启后执行。

## 2026-11-21 Phase H3 可靠性与本机诊断

- 在「设置 → 关于」加入本机诊断及脱敏 JSON 导出。统一队列状态投影，关闭窗口时纳入存储任务取消与收尾。本机生成服务按常见 HTTP 错误给出处理提示。
- 前端 `npm.cmd run build`、`npm.cmd run lint`、`npm.cmd run format:check` 及离线锁定的桌面 custom-protocol 构建通过，ESLint 有 63 条已有警告。6 项 Rust 专项和 9 项原生用例通过，含隔离库的 SQLite 写锁恢复、索引取消及 120 秒固定流式。执行命令和实际结果见 [H3 验收](productization/h3-acceptance.md)。
- 获准临时启动本机 Linux Docker，用固定回环跑完 Codex、Claude Code、Aider 队列。停止、失败接手和重启恢复符合预期，新增残留容器为 0，测试后引擎恢复关闭。[性能回归](productization/h3-performance-regression.md)记录启动、检索、图谱和资源样本。
- 真实模型多轮加载、正式云端请求、执行中物理切断 Docker 引擎、多小时趋势和系统冷开机仍待补验。停在 H3 验收点。

## 2026-11-21 Phase H2 数据管理与迁移

- 在设置交付全量或工作区备份、校验、重启恢复和安全备份。从空库写入工作区数据并重建全文索引，避免其他工作区的索引词残留。保留 Schema 26、现有依赖和历史评测记录。
- 接入草稿和界面偏好恢复、Windows 凭据原位绑定、知识 JSON / Markdown 导出，以及已重建缓存的旧副本清理。存储清单补充备份及库内各类内容的字节统计。
- \`npm.cmd run build\`、lint、格式检查及离线锁定桌面构建通过。Rust 备份专项 4 项、存储专项 2 项，独立 Windows 桌面 3 项迁移与存储验收、2 项标准评测回归均通过。后者使用固定回复替身。真实正式安装升级和跨机器测试待 H4。
- 验证命令、实际结果、截图及样本大小见 [H2 验收](productization/h2-acceptance.md) 和 [验证记录](productization/h2-backup-restore-test.md)。停在 H2 验收点。

## 2026-11-21 Phase H1.4 视觉系统与最终回归

- 统一阅读字号、控件尺寸、圆角、状态和浅暗主题。修复个性化问卷底栏遮挡末项滑块，在 800×600 实测键盘调节和保存。
- `npm.cmd run build`、Prettier 范围检查、ESLint 和离线锁定 Rust custom-protocol 构建通过。独立桌面的 Edge 最终 17 项、原生业务 7 项、最终原生基线 3 项和 H0 评测专项 2 项通过。五页四尺寸两主题各留真实截图，另有强制配色检查。
- H1.3 与 H1.4 交替复跑 8 项性能检查通过。1,000 条会话打开 P50 为 62.16→62.50 毫秒，Markdown 流式入口均为 18 次。完整测量、差异及手验边界见 [H1.4 验收](frontend/h1-4-acceptance.md)。
- H1.0–H1.4 开发与自动化验收收口。独立 Windows 安装、授权与签名仍由 [H0 发布条件](release/h0-release-decision.md) 跟踪。

## 2026-11-20 Phase H1.3 前端接口与导航

- 整理 51 条对话、知识、图谱和评测 IPC 契约，核对 Rust 注册项。拆分知识与关系编辑，统一位置参数和过期读取处理，保留模型设置草稿。
- `npm.cmd run build`、离线锁定 Rust custom-protocol 构建、格式检查通过。ESLint 0 错误，有 64 条提示。4 个 Node 单元及编译期接口反例通过。独立 Windows 桌面的 Edge 4 项、原生 6 项和原生性能对照 2 项通过。见 [H1.3 验收](frontend/h1-3-acceptance.md)。
- 合成流根树提交 19 次、Markdown 流式入口 18 次。1,000 条会话打开至最新消息挂载中位 54.21 毫秒。五页浅暗各四尺寸共 40 张截图，无页面级横向溢出。

## 2026-11-20 Phase H1.2 流式渲染与长会话

- 对话流改为按消息订阅，90 毫秒合并中间更新。首段和终态即时显示。工作区、会话、设置草稿和界面错误按职责管理。长会话首批渲染 60 条，支持加载更早消息、旧消息定位及回到最新内容。
- 在 `apps/desktop` 执行 `npm.cmd run build` 和 `cargo build --manifest-path src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 通过。独立 Windows 桌面运行 `playwright test --project=h1-baseline`，H1.1 留档及最终版各 2 条通过。最终版原生滚动专项 1 条通过，其余原生回归 8 条通过。命令及测试边界见 [H1.2 验收](frontend/h1-2-acceptance.md)。
- 同一 3,000 字符固定回复，React 根树提交 119→20、Markdown 流式入口 60→18。1,000 条合成会话打开至最新消息挂载 P50 307.65→54.36 毫秒。两版各留 40 张原生截图。到达 H1.2 验收点。

## 2026-11-20 Phase H1.1 前端正确性与基础交互

- 统一错误、草稿与窗口关闭处理，补充键盘、表单校验和渲染恢复。以浏览器原生选项保留系统语义，并沿用现有颜色和圆角。
- `npm.cmd run build` 通过，含 TypeScript。离线 Rust 调试 custom-protocol 构建通过。Playwright 使用独立 Windows 桌面和隔离库：原生核心 2 条、原生知识/基线回归 5 条、评测与引文补充 3 条、最终前后对比及菜单 5 条通过。浏览器回归 13 条及专项复测 4 条通过（部分重叠）。所有模型或云端返回是固定测试输入。
- 最终复拍 40 张截图，检查了四尺寸浅暗主题及首屏和流式对照。风险、操作边界及实际测试命令见 [H1.1 验收](frontend/h1-1-acceptance.md)。本轮停在验收点。

## 2026-11-20 Phase H1.0 前端审计与基线

- 核查 30 项范围，C1–C16 按附件索引，V1–V14 根据可用范围及代码重建。现行规范为 HTML 原型。
- 执行 `playwright test --project=h1-baseline`，使用 H0 隔离调试包和测试库。两条通过，共 25.1 秒；采得五页的两主题四尺寸共 40 张截图、3000 字符流性能和 100/500/1000 条长历史数据。
- 详见 [审计](frontend/h1-audit-verification.md)、[性能](frontend/h1-performance-baseline.md)、[截图](frontend/h1-ui-baseline.md) 和 [后续切片](frontend/h1-implementation-plan.md)。

## 2026-11-20 Phase H0 发布阻断项收尾

- 补验公开 v0.2.0、G5、H0 Windows 安装器。离线完整性与载荷检查三套均通过，每套 9 个条目。H0 主程序与留存构建的三个差异字节都来自 Tauri 的 NSIS 安装类型标记。正式安装、升级和签名状态见 [H0 Windows 验证](release/h0-windows-validation.md)。

- 2026-11-21 添加旧标准评测草稿兼容校验。最终 H1 调试构建的标准评测原生固定回复两项通过，覆盖默认保留、即时释放、快照及等待取消。另从当前前端生产源映射识别 85 个实际打包的第三方 npm 包，详见 [H0 发布决定](release/h0-release-decision.md)。

- Core 60 v2 本地批计划支持每题结束即释放。计划哈希、实际请求和上下文清单绑定该设置，旧快照照原值保留。未迁移数据库。
- 根目录执行 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 通过。`apps/desktop` 执行 `npm.cmd run build` 通过，含 `tsc --noEmit`。用非活动桌面跑 `playwright test --project=desktop --grep "cancel resource wait" standard-benchmark.spec.ts --workers=1`，固定回环原生专项 1 条通过，20.3 秒。未运行全套或 Rust 单元测试。
- 非活动桌面以同一测试库交替运行 G0、G5 和 H0 的启动、空闲与 RAG 计时。另用临时诊断可执行文件拆分 Rust、IPC 与 DOM 响应。诊断源文件恢复后未进入交付构建。用 Python 只读 SQLite `EXPLAIN QUERY PLAN` 核对索引。复核两代留档正式包的 SHA-256 和 Authenticode，重新提取 Windows 目标 Cargo 许可树和已装 npm 非开发依赖信息。
- 已授权的两次真实本地 GPU 请求在非活动桌面顺序执行。Core 60 v2 `math-001` 得到 `Passed`，实际请求与快照 `keep_alive:0`；24 节点图谱短窗口单独及并行 P95 均为 7.1ms。两次均确认模型释放，未发云端请求。
- `CARGO_NET_OFFLINE=true` 下 `npm.cmd run tauri -- build --bundles nsis` 成功。H0 候选包 6,306,612 字节，SHA-256 `2A260774422FA69197413B81FC5B89EC3C993FBA1B7981F9D436D2AD59C3325B`，Authenticode `NotSigned`。正式身份 GUI、签名和第三方分发声明仍待验收。没有发布或推送。
- 原始证据位于 `artifacts/phase-h0/`。状态、已知限制和发布影响见 [阻断项](release/h0-blockers.md)、[性能](release/h0-performance-review.md)、[正式安装](release/h0-windows-validation.md)、[发布决定](release/h0-release-decision.md)。


## 2026-11-19 Phase G5 发布候选版验收

复跑三工具共享队列，修复工具间错误取消。业务代码只变更 `tasks.rs` 和 `benchmark/harness/session.rs` 的等待取消归属。新增原生三工具与隔离安装场景，复跑标准评测、过期草案、凭据、网络与迁移定向检查。实际执行 `cargo build --manifest-path src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`、`npm.cmd run tauri -- build --bundles nsis`、三个定向 `cargo test --offline --locked --lib <name>` 和六组非活动桌面专项。队列结果未发现测试标签残留容器。

经授权向真实本地模型转发两次 GPU 对话。标准评测既有五分钟保留参数触发批准范围守门，0 次推理转发。固定回环性能、真实 GPU 数据、阻断及安装手验步骤见 [Phase G5 验收报告](optimization/phase-g5-release-validation.md)。本地候选包 SHA-256 为 `C8494E4CE5148D62E5D3A8FE2451CB181EF82DE8EF1070D684527A4B1E3FBB08`，Authenticode 未签名，未对外发布。

## 2026-11-19 Phase G 项目资源队列、架构收敛与回归

项目副本的准备和提交、Codex / Claude Code / Aider 单题以及组合实验接入共享等待。旧 IPC 调用保留即时申请。取消等待匹配工作区、用途和请求编号，取得任务槽后继续走原有确认与检查。任务记录按原有流程写入。队列登记和信号量注册使用同一顺序，更新或过期的副本无法提交。

`npm.cmd run build`、`cargo check --manifest-path src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 与同配置的 `cargo build` 已执行并通过。非活动桌面以 `--project=harness-docker-real --project=desktop --grep "project validation wait survives reload|cancel resource wait"` 用固定回环联测，2 条通过，32.6 秒。真实 Docker 未启动，项目验证在已知缺失环境下走原有失败路径。

G3 共用本地和云端的禁止重定向与连接超时策略，保留本地独立的代理限制。三条隔离检查恢复语句合为一条并保留用途及错误诊断。G4 中实际运行了聊天、知识、图谱、RAG、评测、项目审核、Windows 测试凭据与缓存恢复。旧库第 6 版原生迁移通过，Rust 第 7 版升级、迁移失败回滚、工作区隔离与部分输出恢复、固定云端个性化流程、固定云端索引与重排的定向测试均通过。验收边界链接见 [Phase G 验收记录](optimization/phase-g-acceptance.md)。

后台启动 Docker 后读取 Linux Engine 29.9.2 和已有 Python 镜像。选择既有原生集成测试逐条执行：`harness-programming-validation.spec.ts` 的三组验证和审核发布、停止与强制退出恢复各 1 条通过。`harness-workspace.spec.ts` 的人工副本与独立评分、`harness-experiment.spec.ts` 的固定回复组合实验、三种工具的固定回复单题各 1 条通过。未发起实际本地或云端模型调用。

最终 `npm.cmd run tauri -- build --bundles nsis` 通过，Release 构建报告 8 分 34 秒。验收包和签名、哈希检查结果在 `artifacts/phase-g2/delivery`。窗口标题栏与弹窗操作通过，三尺寸输入和分屏回归通过。物理拖动的自动化因非活动桌面无法读取鼠标位置而未完成。

## 2026-11-19 G2.4 项目编程验证排队

「项目适配 → 测试任务 → 编程任务草案」的容器验证在确认后复用共享串行队列。每份草案只能提交一个排队或启动请求。重开页面从调度器读取状态，按工作区和请求编号取消。应用重启清空未执行的等待请求，需重新确认。获得资源后再次核对草案来源、修订和镜像标识，写入记录时原子校验最新修订。实际容器环境仍由原验证流程核对。既有执行中断记录、部分结果和容器归属清理入口继续可用。

后端修改 `src-tauri/src/tasks.rs`、`benchmark/harness/programming/validation.rs` 和两处复用取消的评测入口。前端修改 `src/benchmark/harness/{ProgrammingValidation.tsx,programmingTypes.ts}`、`src/studio.ts`。验证场景在 `tests/harness-programming-validation.spec.ts`。

在 `apps/desktop` 运行 `npm.cmd run build` 和 `cargo build --manifest-path src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`，类型检查、前端构建及桌面调试构建通过。按本轮范围使用原生集成验证，未运行单元测试。

在非活动桌面以临时工作区和固定回环响应运行 `playwright test --project=harness-docker-real --project=desktop --grep "project validation wait survives reload|cancel resource wait" harness-programming-validation.spec.ts standard-benchmark.spec.ts --workers=1`。首次项目验证因回环模型缺少预检元数据未进入占用阶段，补全后联合回归 2 条通过（29.2 秒）。标准评测占用时，项目验证刷新后可取消。重复入队和跨工作区取消均被拒绝。取消和过期草案请求留下的验证记录均为 0。最新修订获得资源后进入原环境检查，因无可用环境而记录失败，未生成容器归属。标准评测取消与再运行通过。日志为 `artifacts/phase-g2/harness-validation-queue-final.log`。

截图复核后，等待和启动状态改用中性色。重新构建后用 `--grep "project validation wait survives reload"` 复验，1 条通过（14.3 秒），确认状态为中性色、取消按钮在视口内。截图在 `artifacts/phase-g2/harness-validation-queue-ui`，日志在同名 `.log`。

只读查询发现 Docker Linux 引擎管道未启动。实际容器运行、执行中取消和应用中断后的容器清理本轮未测。真实模型调用及 CPU、RAM、VRAM 对照留给联合验证。现有项目副本、工具单题及组合实验的等待接入仍排在后续。

## 2026-11-19 G2.4 第二项：标准与个性化评测等待资源

报告页确认后的批次复用索引的串行队列；旧 IPC 客户端不传状态通道时仍即时申请。入队前核对工作区、题库类型、计划哈希和批次状态。个性化评测排队前检查云端访问，取得资源后重新读取连接并按原有规则核对绑定、计划和可恢复题目。等待只存于内存，进程重启后需要重新确认。取消等待复用现有停止命令的 `onlyWaiting` 参数；队列已交给执行器时，该操作不能停止实际运行。未修改迁移、历史结果、模型服务或桌面权限。

主要文件：`src-tauri/src/tasks.rs`、`commands.rs`、`benchmark/{standard,report_read}.rs`、`benchmark/personalized/{batch,generation}.rs`；前端 `src/benchmark/{PersonalizedReport.tsx,useBatchOverview.ts,personalized.ts}`；验证 `tests/standard-benchmark.spec.ts`（路径均相对 `apps/desktop`）。

实际命令：`npm.cmd --prefix apps/desktop run build`（含类型检查）；`cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`；同一 manifest 下分别运行 `cargo test --offline --locked --lib scheduler_tests`、`cargo test --offline --locked --lib personalized_batch_restart_only_allows_explicit_resume_of_unsent_items`，两项各 1 条通过。

非活动桌面 WebView2 使用隔离库和回环服务：`playwright test --project=desktop --grep "cancel resource wait|native personalized guard" standard-benchmark.spec.ts cloud-native.spec.ts --workers=1`。首次权限检查通过，但排队取消按钮未生效；改用现有受控停止接口后 2 条通过（26.1 秒）。同一个标准批次刷新后可取消，计划状态、逐题记录不变；取消前后占用方仍在运行，等待者新增请求 0 次。再次确认后等待占用方释放，完成 8 次固定回复请求。个性化验证覆盖未授权访问与恢复规则；没有运行真实云端或本地模型，没有测量显存、吞吐提升或完整业务回归。

截图复核发现排队时仍显示数据库的“待确认”，现由等待状态取代。最终桌面构建后用 `--grep "cancel resource wait"` 重跑，1 条通过（20.3 秒）；日志 `artifacts/phase-g2/batch-queue-ui.log`，原生截图在同名目录。

剩余：项目适配长任务的停止、排队与恢复；跨资源冲突和使用量验证；随后 G3、G4。

## G2.4 第一项：索引等待与取消

本机与云端索引设置页在确认后提交带工作区和任务编号的请求。任务槽通过单许可的 FIFO 信号量协调占用，通过原请求的通道通知等待或开始。排队状态随进程结束释放。聊天、评测及项目任务继续使用即时申请接口；云端索引获得任务槽后仍须通过连接互斥、授权及预算检查。

修改集中在 `tasks.rs`、`commands.rs`、本机和云端 RAG 构建入口、两处索引设置组件及 `rag-native.spec.ts`。在桌面目录执行 `npm.cmd run build`、`cargo test --manifest-path src-tauri/Cargo.toml --offline --locked --lib scheduler_tests` 和 `cargo build --manifest-path src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`，最终均通过。专项验证三个等待者按序获得许可、中间一项取消、跨工作区取消被拒绝，以及新任务无法插队。

非活动桌面的 `playwright test --project=desktop --grep "workspace hybrid" rag-native.spec.ts` 通过，耗时 10.7 秒。固定回环服务占用任务槽时，第二个工作区能看见等待原因，取消后的向量请求为 0；再次提交后正常完成。日志与截图位于 `artifacts/phase-g2/index-queue.log` 和同名目录。没有实测真实模型、云端费用或显存。G2.4 余下工作为评测与项目任务的等待及恢复语义、跨资源冲突和资源指标验收，再进入 G3、G4。

## 2026-11-18 G2.3 缓存确认与恢复

- `storage.rs` 将这次预览与实际界面位置、进程和目录身份绑定。`storage/recovery.rs` 写入预约及结果记录，并在 Windows 上复核、同盘移动、恢复和失败回退。`lib.rs` 在数据库单实例锁取得后、创建主界面前处理预约。`commands.rs` 及主窗口命令清单接入现有任务槽。数据库无需迁移。
- `StorageInventory.tsx` 增加选项确认、撤销及缓存操作记录。支持一次预约一项，启动时最多等上次已核对的 WebView2 进程 10 秒。新缓存会另存后再恢复旧缓存。保留同盘备份时记为释放 0 字节。本轮使用隔离缓存，没有处理正式缓存或调用模型。
- `npm.cmd run build` 与 `cargo build --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 通过。`cargo test --offline --locked --lib storage::` 最终 2 条通过，覆盖预览、真实共享占用导致的回退、两处中断后的恢复和重复启动。默认工具沙箱拒绝访问测试所需的 Windows 目录句柄，最终专项在获准的隔离环境运行。未执行全量单元测试。
- 在非活动桌面用 `artifacts/rag-e6-qa/alternate-desktop.py` 执行 `playwright test --project=desktop --global-timeout 110000 storage-native.spec.ts storage-recovery-native.spec.ts --output ../../artifacts/phase-g2/recovery-acceptance`。最终 2 条通过，总 17.1 秒。原生检查覆盖确认前不执行、撤销、非法缓存编号、重复预约、重启移入备份、保留新缓存并恢复旧缓存。样本旧缓存的逻辑大小前后均为 1570 字节。两个原生截图和结果 JSON 位于输出目录，运行日志为 `artifacts/phase-g2/recovery-acceptance.log`。
- 首轮原生验证修正了弹窗取消焦点、测试定位，以及窗口关闭后 WebView2 进程可能尚未退出的问题。正式安装环境、跨盘移动和永久删除备份未做实机验证。跨盘和永久删除不在本次操作范围。G2 当前仅剩 G2.4 任务资源协调，之后为 G3 架构收敛与 G4 完整回归。

## 2026-11-18 G2.2 回收预览

- 「设置 → 本地数据 → 预览可回收内容」读取缓存大小、管理方、位置、核对结果以及拟用备份磁盘余量。预览复用现有读取编号、停止操作和 SQLite 引用统计。Rust 从实际运行的 WebView2 查询其数据目录，匹配默认位置后才检查三个固定缓存子目录。其他界面数据、索引、备份、原始资料和历史证据保留。Windows 的两个接口依赖原本已在锁文件内，本轮将其声明为直接依赖。
- `npm.cmd run build` 通过，含 TypeScript 检查。初次 `cargo check --offline` 发现当前 Tauri 需要通过 `get_webview_window` 获取主界面，已修正。最终 `cargo build --offline --locked --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --bin cortexos-desktop` 通过。Vite 仍有图谱动态块超过 500 kB 的既有提示。
- `cargo test --offline --locked --lib storage_reclaim_candidates_fail_closed_and_leave_files_intact` 专项 1 条通过。用隔离文件验证 5 字节候选、空目录、目录名被普通文件占用、自定义位置、备份路径冲突、取消及源文件不变。整套单元测试按本轮范围未运行。浏览器固定 IPC 回归 `node artifacts/phase-g2/storage-cancel.cjs` 通过，核对主动读取、按编号取消和保留旧报告。
- 用 `artifacts/rag-e6-qa/alternate-desktop.py` 在非活动桌面执行 `playwright test --project=desktop --global-timeout 50000 storage-native.spec.ts --output ../../artifacts/phase-g2/reclaim-preview-final`。首轮定位器误用了精确行文本，修正后复跑通过。最终 1 条通过，总 9.4 秒。隔离配置中 3 类缓存均标为位置未核实，显示 12 个受保护组、1 条知识快照版本引用及备份磁盘可用空间 586751565824 字节。运行前后核对主库、WAL、升级备份和源文件内容相同，备份目录仍不存在。1366×768 浅色、1600×900 浅色、1920×1080 深色截图均已查看。原生日志 `artifacts/phase-g2/reclaim-preview-final.log`，截图及原始 JSON 位于同名目录。
- 未在正式安装的数据目录扫描缓存，原生端到端覆盖隔离自定义位置保护，默认目录扫描规则由专项文件测试覆盖。本轮只有预览，未实际腾出空间或调用模型。G2.3 确认与恢复、G2.4 任务资源协调待实施。实际回收需针对具体预览另行确认。

## 2026-11-18 G2.1 只读存储清单

- 在「设置 → 本地数据」增加按需读取。`StorageInventory.tsx` 展示实际文件大小、读取时间、管理方、引用、保护原因及部分读取状态。`storage.rs` 按应用数据位置读取文件元数据，通过独立只读 SQLite 事务统计关联。两个 IPC 只接收读取编号，复用独立任务槽取消，不占用现有模型和评测槽。数据库、原始文件和历史记录保持原有存储行为。
- `npm.cmd run build` 通过，包含 TypeScript 检查。初次 `cargo check --offline --locked` 发现 SQLite 计数要先按 i64 读取，已改为校验后转换。`cargo build --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 通过，长路径样式调整后再次构建通过。没有新增依赖或迁移。
- `node artifacts/phase-g2/storage-cancel.cjs` 通过。生产前端在 headless Edge 用固定 IPC 检查进入设置时不读、停止仅绑定本次编号以及保留上次清单。
- 非活动桌面运行 `playwright test --project=desktop --global-timeout 50000 storage-native.spec.ts`。首轮修正测试文件必须在隔离目录内的既有约束，首次长路径断言修正为定位已展开的详情。最终 1 条通过，总 8.4 秒，原生核对字节、分配量、实际知识引用、目录联接跳过、未知 SQLite 文件排除、长路径不挤断字段名，以及主库/WAL/备份/源文件在读取前后内容相同。三尺寸截图与 `inventory.json` 位于 `artifacts/phase-g2/storage-native-layout`，已实际查看浅色和深色。
- 最终隔离样本已测 1000952 字节，其中主库 4096、WAL 959992、SHM 32768、升级备份 4096。知识到资料版本引用与知识快照版本引用各 1 条。隔离 WebView2 使用自定义数据目录，按合同显示未测量。真实 Ollama、Docker 与外部文件未扫描，未执行回收或运行真实模型。
- G2.1 验收完成。剩余 G2.2 回收预览、G2.3 确认与恢复、G2.4 任务资源协调。当前无 G2.1 阻断。G2.3 的实际回收需在生成具体预览清单后另行确认。

## 2026-11-18 Phase G1.9 Windows 构建与阶段收尾

- Windows 桌面入口直接链接 `cortexos_desktop_lib` 的 Rust 库。`Cargo.toml` 将本项目库产物由 `staticlib`、`cdylib`、`rlib` 改为 `rlib`。保留原 Release 代码生成参数、功能和锁文件。
- 在同一隔离目标目录 `artifacts/phase-g1/build-compare` 使用 `measure-build.ps1 -Release` 两次运行 `cargo build --offline --locked --features tauri/custom-protocol --bin cortexos-desktop --release`，以 Cargo 编译清单逐项计量应用产物。逻辑字节 365842818→143603833，减少 222238985 字节。桌面 EXE 前后均为 20661248 字节。两次总耗时 706.51 秒和 471.73 秒，依赖缓存状态不同，只作为实测记录。旧产物仍保留，磁盘未执行回收。
- `npm.cmd run tauri -- bundle --bundles nsis --features tauri/custom-protocol --no-sign --no-binary-patching --ci` 退出 0，生成内部 NSIS 包 6197153 字节，SHA-256 `558210963a8cd654899476822b75da36d0df1a2329ef980464d4a547124c8c8d`。本次验证 Windows 构建和制包，未实测安装升级。`build-before.json`、`build-after.json`、`windows-delivery.json` 位于本轮审计目录。Release 保留三条原有 `unused_mut` 提示。
- G1.8 累积版通过 `playwright test --project=desktop --grep 'workspace hybrid|model combination runs|native Markdown' rag-native.spec.ts markdown-camera.spec.ts`，3 条通过，28.9 秒。聊天重启专项独立运行 `playwright test --project=desktop --global-timeout 45000 chat.spec.ts`，1 条通过，41.7 秒，覆盖生成、停止、失败和重启。都使用非活动桌面、固定回环回复和隔离数据，保留实际运行日志。
- G1 九项均已有前后对照与对应自检。实时本地模型、远端云模型、正式安装和完整跨领域回归保留给相应阶段。G2 存储与资源生命周期、G3 架构收敛、G4 完整回归待后续实施。

## 2026-11-18 Phase G1.4–G1.8 轮询、长会话、索引和读取

- **G1.4 轮询。** 聊天轮询改查独立的工作区和用途约束状态。通道负责当前会话流式更新，读盘负责恢复和最终状态。详情和模型组合对照改为前一次响应结束后再安排下一次，终态停止。`npm.cmd run build`、`cargo build --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 通过。`CORTEX_G1_PROBE=polling` 的固定响应探针记录聊天共 3 次轻量状态读取、完成后 0 次，详情两次请求且峰值并发为 1。原来打开详情会每秒读取完整记录，直到关闭。原生固定回环的模型组合对照通过，日志 `g1-4-native.log`。
- **G1.5 长会话。** 拆除消息卡对整个工作台状态的依赖。用稳定消息引用与操作回调避免输入框变化使已加载消息重新渲染。现有消息 DOM 全数保留，引用定位和选择文本仍使用 DOM。`CORTEX_G1_PROBE=chat-render-before/chat-render-after` 分别采 5 轮 1000 消息会话、每轮 12 次按键。Chrome DevTools 记录脚本时间 P50/P95 146.44/177.28→14.06/18.42ms，含输入自动化的墙钟时间 344.87/376.97→191.09/203.80ms。此处统计实际浏览器任务，未推断 React 提交次数。`markdown-camera.spec.ts` 的 Markdown、选取原文及引用检查原生通过，构建通过。
- **G1.6 本机索引增量构建。** 在当前工作区、同模型及指纹、同索引格式下按内容哈希复用合法向量。缺失内容按批生成，事务内复核当前资料。索引写入仅新增、更新或清除当前索引中的派生行。云端接口没有不可变模型指纹，本轮先保持其原构建约定。原生固定回环实测无变化 1→0 次向量请求，新增资料仅发送新文本，来源停用后复用其余向量。旧指纹、错误响应与替换索引仍受原验证。
- 用非活动桌面运行 `playwright test --project=desktop --grep 'effective knowledge|authorized snapshots|workspace hybrid' context.spec.ts sources.spec.ts rag-native.spec.ts`，3 条通过，总 18.6 秒。复验覆盖 G1.1 两版本快照、资料停用及重启，G1.6 无变化重建、单条变化、工作区隔离、不同指纹、维度回退及重建。测试改用既有 UI 关闭辅助，补上已有的 `local_model_usage` 事件并把依据选择器限定在详情抽屉。先前合并启动器因聊天长用例超时，未将该批当作通过。保留 `g1-6-targeted.log` 与失败日志。
- **G1.7 云端传输。** 目录查询和 JSON 提交分别使用长驻连接池，保留各自 8/10 秒连接超时及每请求超时。凭据仍逐请求绑定，重定向继续禁止。`cargo test --offline --locked --lib sequential_cloud_posts_reuse_a_connection -- --nocapture` 用相同 HTTP/1.1 回环输入采改前、改后 TCP 连接 3→1。改前目标断言按预期失败，改后通过。三个凭据头及重定向拒绝均核对，实际外网延迟和费用未调用。
- **G1.8 向量读取。** `cargo test --offline --locked --lib rag_semantic_scan_measures_large_workspace_with_fixed_vectors -- --nocapture` 改前和改后各通过一次，均包含 1 次预热和 10 次采样。隔离 SQLite 含 1000 节点、2000 片段及 1536 维固定向量。直接访问 SQLite 借用的向量，只为选定的前 20 条片段加载完整正文。查询 P50/P95 170.91/178.37→148.31/156.75ms，短请求竞争等待 170.93/178.39→148.35/156.85ms。维持相似度公式、哈希过滤、片段去重与排序，核对前 20 条精确 ID、片段、正文和损坏向量排除。调试测量不外推到 Release。沿用单连接，未增加索引或迁移。
- 原生和浏览器均使用合成资料、固定回环或固定向量。本轮未触发真实模型调用。G1.9 构建产物对照继续进行。

## 2026-11-18 Phase G1.1–G1.3 请求与首屏优化

- **G1.1 资料选择器。** 展开时读取已生效、日期有效且来源可用的知识摘要，每页最多 100 条。维持原手动选择版本和发送时复核。`npm.cmd run build` 和原生 `cargo build --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 通过。首轮 Rust 检查修正 SQLite LIMIT 类型，首轮原生检查补齐主窗口只读 IPC 权限。
- `CORTEX_G1_PROBE=context` 执行 `node artifacts/phase-g1/frontend_probe.cjs`，与 G0 的 1000 条合成资料场景相比，折叠读取从 1 次列表加 1000 次详情降为 0 次。展开 1 次返回 100 条，再用 1 次读到 200 条，选中第二页后关闭再展开仍保留选择。单次首屏展开约 43.55ms。记录为 `artifacts/phase-g1/g1-1-context.json`，并检查 1600×900 截图。
- 在非活动桌面通过 Playwright 指定 `context.spec.ts sources.spec.ts --grep 'effective knowledge|authorized snapshots'`。通过新增的有效版本、日期、跨工作区、未确认知识及停用来源断言，已核对第一条消息的实际固定回环请求快照；两条整用例最终未通过。`context.spec.ts` 的预期事件缺少 Phase F 增加的 `local_model_usage`，`sources.spec.ts` 在停用与历史快照断言之后等待测试窗口关闭超时。日志 `g1-1-native-verified.log` 保留实际失败位置。
- **G1.2 图谱分页。** 同范围、同快照已有节点作为前缀，新增页照常校验快照。快照过期时从第一页重读，切换范围时停止旧请求链。`node artifacts/phase-g1/frontend_probe.cjs` 的 1000 节点 / 5000 关系合成场景重复 5 次，节点分页 15→5 次，关系 5→5 次。加载 P50/P95 609.69/631.80→618.11/651.55ms，包含原固定等待，合成场景未观察到时延下降。旧快照另验 `[200,旧]→[0,无]→[200,新]`，空闲 500ms 新增绘制仍为 0。原始数据 `frontend-graph.json`。
- 前端构建及原生构建通过。非活动桌面运行 `playwright test --project=desktop graph-query.spec.ts graph-neighborhood.spec.ts`，2 条通过，总计 19.4 秒，覆盖筛选、邻域、跨工作区、快照失效、历史、编辑与保存布局。日志 `g1-2-native.log`。
- **G1.3 路由分块。** 主框架和聊天直接渲染，知识、图谱、设置、评测及评测子页首次进入时加载。编辑器按需加载，保留主框架状态与路由参数。`npm.cmd run build` 通过，主入口 899978→493567 字节。计算首屏所有实际请求的 JS（含共享预加载）为 899978→519154 字节，下降 42.3%。与 G0 的 899089 字节相比下降 42.3%。图谱大块仍按需加载。
- `CORTEX_G1_PROBE=routes` 执行生产包浏览器检查，复核首屏没有后台页面块，首次打开知识/标准评测约 337/335ms（各 1 次，含自动化等待），首次设置块故障后可重试，返回聊天草稿保留。`frontend-routes.json` 保存资源名与字节。Edge `app.spec.ts benchmark-result-drawer.spec.ts knowledge-scope.spec.ts` 的 13 条断言流程均记录通过。原生图谱测试对应分块前 G1.2，G1.3 的原生复核随后随累积构建执行。

## 2026-11-18 Phase G0 全项目审计

- 用户限定本轮只做 G0。读取 development.md、开发规范、现行实施计划及实际工作区；HEAD 为 8e82c42，已有 E/F 业务增量保留。新增 docs/optimization 下五份中文报告，记录架构、磁盘、性能、依赖及 G1–G4 路线；没有改业务代码、依赖、迁移或清理缓存。
- 使用 Python 3.13 审计辅助脚本 storage_probe.py、storage_detail.py、database_probe.py，输出在 artifacts/phase-g0-2026-11-18。首轮项目 52068461892 字节；细分 target 36592838606、artifacts 15245951646、node_modules 331409790 字节。已区分扫描时间、逻辑/物理口径、正式数据、开发产物和外部模型/Docker；Local profile 有 9 个读取错误，保留下界。
- SQLite 使用 mode=ro、query_only，只读 schema、计数、索引与暖查询，未执行迁移、ANALYZE、VACUUM、checkpoint 或重建。正式库 schema 7，2 个知识节点；最新验收副本 schema 26。dbstat 不可用，逐对象物理页占用未测量。
- native_baseline.cjs 复用已有 cortexos-phase-ef-final.exe，通过 artifacts/rag-e6-qa/alternate-desktop.py 在非活动 Windows 桌面执行，未切换或弹出到用户桌面。9 次启动含 1 次预热，新 profile 3 次 P50/P95 为 722.42/1130.76ms，复用 profile 5 次为 707.66/724.36ms。30 次预览 IPC 为 6.90/8.10ms；固定向量/回答回环结果单独标注。
- 用户弹窗批准的 CPU 批次设置 CORTEXOS_G0_CPU_BATCH=approved-10，仅实际执行 6 次聊天和 2 轮双组固定题对照，共 10 次 qwen3:4b-instruct 请求。指纹 0edcdef34593…，2048 上下文、最多 64 输出 Token、60 秒超时、温度 0、CPU、结束释放，不自动重试。6 次聊天和 4 题均 completed，聊天 P50/P95 为 3741.33/6541.46ms，单题对照为 9322/9471ms；批后 /api/ps 为空。没有继续调用，未下载模型或调用云端。
- resource_probe.py 每约 500ms 采进程/系统资源、每约 1 秒采整卡显存。原生空闲进程树 RSS P50/P95 为 438652928/439480320 字节。模型独立 runner 未完整采到，CPU/RAM 不作伪推断；系统 CPU、系统可用内存和全设备显存与应用进程树分开报告。
- node artifacts/phase-g0-2026-11-18/frontend_probe.cjs 使用已有 dist、headless Edge、固定 IPC、1600×900。修正首次辅助脚本的长会话选择器和计时后，以 frontend-verified.json 为准。100/500/1000 节点各 5 次，1000 节点/5000 关系加载 P50/P95 为 609.69/631.80ms（含脚本稳定等待）；均 4 次绘制调用、空闲 500ms 新增绘制为 0。1000 消息实际 16787 个 DOM；折叠资料选择器仍触发 1000 次详情读取，图谱逐步加载触发 15 次节点分页。
- 执行 npm.cmd ls 读取依赖树；最终在 apps/desktop 复核 npm.cmd ls three @react-three/drei --all --json，退出 0。apps/desktop/src-tauri 中 cargo metadata --offline --no-deps --format-version 1 与 cargo tree --offline --duplicates 均退出 0，输出保存 verified 文件。确认已有 Release LTO 等配置，以及 stats-gl 下第二个 Three.js 版本；未执行安装或去重。
- 执行 py -3.13 artifacts/phase-g0-2026-11-18/summarize.py、dependency_probe.py 汇总原始数据；frontend_resource_summary.py 从已有采样汇总浏览器进程树，未运行新的模型任务。正式与隔离库的事件/WAL 读数分别保存。Docker 只读查询显示 Client 存在、Server 为 null，system df 无引擎响应，不启动引擎；镜像/容器/构建缓存明细及实际 Harness 性能未测量。
- 本轮未跑单元套件、类型检查或重新构建：没有业务实现改动。py -3.13 artifacts/phase-g0-2026-11-18/audit_manifest.py 已核对五份报告相对链接及批次数/终态一致性，并为 282 个源文件/配置/数据集文件、程序、dist 和原始观测保存 SHA-256。git diff --check -- docs/milestone-log.md 通过，仅既有 CRLF 提示。初次 frontend.json、受限账户元数据失败和无效 rg 路径不当作通过。真实 Embedding、真实云端、系统冷启动、最新 Release 和完整模型进程归属仍列未测项。
- G0 已完成，G1–G4 尚未实施。推荐验收后从 G1.1 资料选择器按需读取开始，随后图谱分页、路由分块和终态轮询；每切片分别报告前后数据、功能兼容、剩余与阻断。当前暂停等待 G0 验收。

## 2026-11-17 F7 真实 Ollama 对照与评分复验

- 按用户当批授权先通过 `/api/tags` 和 `/api/ps` 核对 qwen3:4b-instruct 指纹与空加载状态。在 `apps/desktop/src-tauri` 设置本批专用环境变量并运行 `cargo test --offline --lib f7_authorized_local_smoke -- --ignored --nocapture --test-threads=1`。1 项通过，32.12 秒。两次模型请求各自完成，完全使用隔离库和 CPU，未执行自动重跑。批后 `/api/ps` 显示无已加载模型。
- 词语查找命中 v2 并答出 90 天，未查找侧给出无本题依据的通用说明。端到端分别 10.549 秒和 21.138 秒，模型报告输入／输出 402／14 和 73／182 Token。原始报告使用 v1，漏判正确答案的等价表述。保留原始 JSON 和日志，原始报告 SHA-256 为 `55926774eadc4314e0cfe439800aabc66b594f5e845a2ec51f8b7bc34c4a61b6`。
- 新的评分版本支持日志保留的明确等价词组，核对引用所在语句、实际版本和原文行范围。首次扩充测试发现引用位于反驳语句中间时仍可错误获分，已按整条语句复核。最终 `cargo test --offline --lib rag_runtime -- --nocapture` 5 项通过。普通 `--lib rag_evaluation` 6 项通过，另 2 项显式留给各自的专项命令。
- 运行 `cargo test --offline --lib f7_offline_rescore -- --ignored --nocapture`，1 项通过。独立导出用 v2 复算原报告，词语查找的答案和引用召回率均为 1，关闭查找均为 0，模型调用为零。实际数据、独立复算和后续边界见 [Phase F 报告](phase-f-delivery.md)。
- `npm --prefix apps/desktop run build` 通过，保留 Vite 大包体提示。在 `apps/desktop/src-tauri` 执行 `cargo rustc --offline --features tauri/custom-protocol --bin cortexos-desktop -- --emit=link=<最终独立程序绝对路径>`，生成 `artifacts/cortexos-phase-ef-final.exe`。用非活动桌面启动器执行 `playwright test --project=desktop rag-native.spec.ts --output ../../artifacts/phase-ef-final-native`，2 项通过，总计 18.5 秒。最终原生检查使用固定回环回复，覆盖保存 v2 报告、原生导出、混合检索及向量异常回退。开发与验证结束，进入用户验收。

## 2026-11-17 F7 原生导出检查

- 扩展 `rag-native.spec.ts`，通过「导出完整报告」打开 Windows 保存窗口，并以原生保存对话框测试辅助脚本自动写入隔离产物目录。首次运行的 UI Automation 按钮 Invoke 失败，辅助脚本改为优先识别确定的保存按钮，必要时向本次测试的窗口提交原生命令。
- 在 `apps/desktop` 使用非活动桌面启动器执行 `node node_modules/@playwright/test/cli.js test --project=desktop rag-native.spec.ts --grep 'model combination' --output ../../artifacts/phase-f7-native-export-final`，1 项通过，总计 9.6 秒。实际 JSON 文件与已保存报告逐字段一致。核对题库、两组策略、请求指纹、执行事件及用量，且跨工作区导出被拒绝。验证用固定回环回复，无 Ollama 推理或外部计费。

## 2026-11-17 Phase E／F 原生复验

- 使用 Windows 登录权限，在非活动桌面执行回环测试。受限账户的 CDP 超时已绕开。原生调试已连通，首轮定位到测试通过 IPC 创建工作区时前端没有刷新。改为使用应用的创建工作区入口。
- F7 在 WebView2 通过计划确认、两个固定回复、查询历史、跨工作区拒绝及 3D 节点详情。用 Python SQLite 对专用隔离库执行只读完整性检查，结果为 `ok`、零项外键问题，普通聊天、执行记录和云端请求均为零。截图及日志保存在 `artifacts/phase-f7-native-final` 和 `artifacts/phase-f7-native-final.log`。
- 去除 F7 执行图详情中的重复检索结果。执行 `npm --prefix apps/desktop run build` 及原生独立构建，输出 `artifacts/cortexos-phase-ef-acceptance.exe`。无头 Edge 检索评测 2 项通过，产物在 `artifacts/phase-ef-browser-final`。
- 在 `apps/desktop` 使用 `alternate-desktop.py` 执行 `node node_modules/@playwright/test/cli.js test --project=desktop rag-native.spec.ts --output ../../artifacts/phase-ef-native-verified`，2 项通过，总计 10.1 秒。原生混合查找专项核对关键词重排、来源摘录、原文与回答定位、3D 轨迹、维度错误回退和索引替换。修正旧测试中已变化的重排文案和将整段摘录称为片段命中的断言。复核 1366×768 深色证据页和 F7 原生报告。真实模型调用仍待当批授权。

## 2026-11-17 F7 模型组合对照

- 在「评测 → 检索评测 → 模型组合对照」冻结内置题库、事实版本、查找策略、现有运行方案、模型指纹、本机设备设置和费用上限。用户确认后用隔离库执行两组，支持停止、查看历史和 JSON 导出。迁移 26 保留已完成的单题和用量，重启将进行中的实验标作中断。
- 回答要点与引用分别评分，支持核对实际发出的资料、版本和行范围。单题可打开现有 3D 执行画布。云端环节复用发送权限与预算，只使用合成题目和资料。
- 在 `apps/desktop/src-tauri` 运行 `cargo test --offline --lib rag_runtime -- --nocapture`，5 项通过，覆盖真实回环请求、固定答复与引用、两个方案共用预算、旧版本资料、向量至重排至生成及迁移 26 的失败回滚和恢复。费用仅为固定费率的回环结果。
- 在 `apps/desktop` 运行 `npx --no-install playwright test --project=edge rag-evaluation.spec.ts runtime-cloud.spec.ts runtime-profiles.spec.ts local-runtime.spec.ts knowledge-scope.spec.ts --output ../../artifacts/phase-ef-browser`，7 项通过。在最后界面调整后运行 `npx --no-install playwright test --project=edge rag-evaluation.spec.ts --output ../../artifacts/phase-f7-final-browser`，2 项通过。界面测试使用固定 IPC 响应，检查 1366×768、1600×900 和 1920×1080。
- `npm --prefix apps/desktop run build` 和 `cargo rustc --offline --features tauri/custom-protocol --bin cortexos-desktop -- --emit=link=<独立程序绝对路径>` 通过。Vite 仍报告已有的大包体提示。独立原生程序位于 `artifacts/cortexos-phase-ef.exe`。
- `cargo test --offline --lib rag_evaluation -- --nocapture` 6 项通过、1 项因真实模型授权单独保留。非活动桌面运行 `playwright test --project=desktop rag-native.spec.ts --grep 'model combination'` 使用固定回环，CDP 30 秒超时，尚未进入界面断言。日志 `artifacts/phase-f7-native.log` 可用于复核。只读 Ollama 列表显示两个本机模型，当前无模型加载。
- F0–F7 实现与自动化检查已交付。原生 WebView2 和真实模型完成情况见 [Phase F 报告](phase-f-delivery.md)。

## 2026-11-17 F6 资源与用量

- 设置新增「资源与用量」，按工作区与最近 24 小时或 7 天汇总提供方、模型和能力。显示真实请求与完成次数、输入输出 Token、向量数量、平均及总用时、检索均时、费用估算、预留额和未知费用。预算按单次任务和 24 小时显示。
- 迁移 25 记录本地向量请求和服务返回的用量。本机生成耗时与 CPU 区间平均值随实际调用记录。设备读数复用本机状态采样，明确为系统和整张显卡的读数。NVIDIA 驱动工具缺少读数时显示未知。
- 实际运行 `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib runtime_usage_report`，1 项通过，核对云端及本地用量、失败与预留额、工作区隔离。`--lib model_runtime_local_capabilities` 和 `--lib chat_fallback` 各 1 项通过。Rust 检查和前端构建通过。
- 在 `apps/desktop` 运行 `npx --no-install playwright test --project=edge runtime-cloud.spec.ts runtime-profiles.spec.ts --output ../../artifacts/phase-f6-resources`，3 项通过，包含工作区切换、费用未知和无 GPU 驱动读数。设备读数界面用固定响应验证，未进行真实模型调用或 WebView2 复核。
- F6 已交付，剩余 F7 对照、评分、报告导出及整体验收。

## 2026-11-17 F4 任务路由与 F5 隐私检查

- 迁移 23 保存工作区按普通问答、知识问答设置的首选与费用对照方案。较低费用模式比较同一请求容量下已登记费率的估算结果。费用缺失保留未知。自动切换只使用配置的本机备用回答模型，在已有回答内容或用户取消时保留当前结果。主请求快照保持不可变，备用请求另存快照。
- 迁移 24 增加工作区邮箱和连续 11 位大陆手机号过滤。云端发送及重试前复核有效版本、来源状态和目标连接对应的授权。参考资料在提示中明确作为待分析的数据，聊天执行路径只生成文字。记录发送类型、连接、提供方和模型，凭据留在 Windows 凭据管理器。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib chat_router` 1 项通过。备用模型专项初次发现主快照不可修改，改为独立保存备用快照后，`--lib chat_fallback` 1 项通过。固定回环模拟主服务 503，验证实际使用本机备用、两份快照及执行事件。`--lib cloud_privacy` 2 项通过，覆盖发送前拦截、工作区隔离和授权撤销后的限流重试终止。云端回答、预算及向量重排各回归 1 项通过，Rust 检查及前端构建通过。
- 在 `apps/desktop` 运行 `npx --no-install playwright test --project=edge runtime-profiles.spec.ts runtime-cloud.spec.ts --output ../../artifacts/phase-f4-f5-routing-privacy`，2 项通过。截图覆盖路由、隐私、索引确认和范围确认界面。真实模型及原生 WebView2 尚待复核。剩余 F6 和 F7。

## 2026-11-17 F4 云端请求限制与取消

- 迁移 22 保存工作区费用上限和每次云端尝试的用量。设置可指定单次任务、最近 24 小时、每连接每分钟请求数和限流重试次数。请求发出前原子预留估算费用，明确限流才按配置重试。失败、取消和中断保留预留额与结果不明的状态。
- 费用限制覆盖统一运行的云端回答、索引、问题向量、重排和能力验证。费率来自手工登记，实际用量来自服务响应。价格未知时按未知显示。开启预算后依照已知费用估算和未结算预留额判断是否继续。执行记录关联发出请求的用量条目。
- 对话进入查询和重排时即可停止，取消后保留输入草稿。仍可取消生成和索引构建。
- 实际运行 `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib metered_cloud`，1 项通过，固定回环分别响应 429 和成功，另外检查未定价、单次任务、滚动预算、限流和中断保留。 `--lib cloud_rag_index_and_rerank`、`--lib runtime_profiles_cloud_chat` 各 1 项通过。Rust 检查、前端构建和差异检查通过。
- 在 `apps/desktop` 运行 `npx --no-install playwright test --project=edge runtime-profiles.spec.ts runtime-cloud.spec.ts --output ../../artifacts/phase-f4-runtime-limits`，2 项通过。验证费用输入换算、准备时停止、草稿保留及索引确认，并查看费用限制和索引确认截图。无真实模型调用，原生 WebView2 尚待复核。
- F4 剩余按任务选择方案、成本与质量偏好和可审计的备用模型路由。之后继续 F5–F7。

## 2026-11-17 F3 云端向量与重排

- 迁移 21 增加按工作区、连接和模型区分的云端索引。构建前确认选定的授权知识版本，分批检查并发送标题与正文片段。构建完成后以原子事务替换相应模型的索引。云端查询只发送本次问题，向量比较在本机进行。撤回授权、关闭来源或失效版本立即退出云端语义检索。
- 运行方案已接入三环节的本地和云端选择。混合或云端方案发送前逐项展示模型和数据范围。云端重排只重排用户本次确认范围内的已授权候选，保留其余候选的本地位置。实际请求、部分结果、不可用及保留本地排序的原因在执行详情中可见。
- 实际运行 `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib cloud_rag_index_and_rerank`，1 项通过。验证回环收到的索引文本、问题向量、重排候选、返回维度与用量，以及跨工作区拒绝和撤销后过滤。`--lib runtime_profiles` 2 项通过，`--lib query_route_retains_private` 1 项通过。
- 在 `apps/desktop` 运行 `npx --no-install playwright test --project=edge runtime-profiles.spec.ts runtime-cloud.spec.ts --output ../../artifacts/phase-f3-cloud-rag`，2 项通过。验证设置中的云端索引确认、取消、成功状态和对话授权。Rust 检查和前端构建通过。记录无头 Edge 截图，真实服务与原生 WebView2 待复核。
- F0–F3 已接通，剩余 F4–F7。下一片为模型路由与容错。

## 2026-11-17 F3 工作区方案与回答

- 迁移 19 后可从设置保存、切换和删除工作区运行方案。启用方案后，实际对话使用选定的回答模型，向量索引按本机模型匹配，词语重排可独立启用。本地默认设置继续照常工作。
- 云端回答逐次确认。迁移 20 保存每个云端连接获准处理的知识版本；发送时再次核对工作区、连接、版本生效状态、文件停用状态和当次选中的授权版本。确认界面可另行发送手选知识。历史对话留在本机。云端请求保存实际请求体与发送范围、收到响应和真实用量。密钥不进入请求快照。基本敏感内容检查识别常见令牌和私钥。
- 实际运行 `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib runtime_profiles`，2 项通过。固定回复的回环 HTTP 检查核对实际请求与快照一致、作用域拒绝、手选资料、确认为本次发送的授权资料及响应持久化。`--lib cloud_privacy_filters`、`--lib query_route_retains_private` 各 1 项通过，`cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml` 和 `npm --prefix apps/desktop run build` 通过。
- 在 `apps/desktop` 运行 `npx --no-install playwright test --project=edge runtime-profiles.spec.ts runtime-cloud.spec.ts local-runtime.spec.ts --output ../../artifacts/phase-f3-model-profiles`，3 项通过。浏览器以固定 IPC 响应验收方案切换、取消保留草稿、授权默认关闭及确认发送，并保存 1366×768 方案与 1920×1080 深色确认截图。原生 WebView2 和真实模型需单独实测。
- F3 剩余云端向量建索引、语义查询和重排执行。当前开发可继续，之后进入 F4–F7。

## 2026-11-17 F2 云端模型能力

- 新增设置中的「云端模型」。支持在每个工作区保存多个 OpenAI、Anthropic、Gemini 和 OpenAI 兼容连接，读取目录、独立验证指定模型的能力、登记 Token 上限和费率。能力验证仅发送固定测试文本，兼容服务须明确提供 `/rerank` 才显示重排验证。
- 迁移 18 在 SQLite 保存公开连接配置和检查结果。Windows 凭据按数据库、工作区和连接派生存储位置，并校验服务商和地址绑定。移除连接时删除密钥。保留 C1 个性化评测的凭据和验证行为。
- 实际运行 `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib runtime_cloud`，3 项通过。核对协议解析与向量顺序、固定样本的 Windows 凭据读写及跨工作区和篡改地址拒绝、迁移 18 失败回滚和备份。受限会话首次无法写入系统凭据，使用 Windows 登录会话重跑通过，随机凭据已清理。`--lib personalized_cloud_wire` 单项通过。 `cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`、`npm --prefix apps/desktop run build`、`git -c core.safecrlf=false diff --check` 通过，Vite 保留既有包体提示。
- 在 `apps/desktop` 运行 `npx --no-install playwright test --project=edge runtime-cloud.spec.ts --output ../../artifacts/phase-f2-cloud-runtime`，1 项通过。模拟 IPC 验证保存、目录刷新、验证确认与取消、登记费率、工作区切换和移除。记录浅色、深色在 1366×768 和 1920×1080 下的截图。本轮未进行真实模型调用或原生 WebView2 界面复核。
- F0–F2 已交付，剩余 F3–F7。F3 接入工作区运行方案，实际模型调用验收待明确授权，当前开发可继续。

## 2026-11-17 F1 本机模型运行与资源

- 新增 `local_runtime.rs`、`LocalRuntimeSettings.tsx`。设置页可读生成、向量及容量声明和 Ollama 已加载模型的显存数据。可选仅用 CPU 和 0–300 秒保留时间，默认 0 秒。普通对话的实际快照使用这些设置，已冻结的评测配置沿用原预算。卸载取得任务锁，验证安装及加载指纹，并复查是否已释放。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` 分别筛选 `local_runtime_preferences`、`model_runtime_local_capabilities`、`query_route_retains_private`、`rag_evaluation_runs_real`、`standard_batch_freezes`，共 5 项通过。固定 HTTP 响应验证只有已加载模型发送释放请求，复查通过。`cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`、`npm --prefix apps/desktop run build` 与 `git diff --check` 通过，Vite 的包体提示保留。
- 在 `apps/desktop` 使用本地 Vite 并运行 `node node_modules/@playwright/test/cli.js test --project=edge local-runtime.spec.ts --workers=1 --retries=0 --output=../../artifacts/phase-f1-local-runtime`，1 项通过。模拟 IPC 确认设置保存、状态显示、卸载取消与二次确认。检查 1366×768 和 1920×1080 的浅深截图。未启动可见窗口，真实 Ollama 与 WebView2 待验收。
- 下一片 F2 云端模型能力，其后 F3–F7，目前无开发阻断。

## 2026-11-17 F0 模型能力接口

- `model_runtime.rs` 提供独立的向量、重排和生成合同。RAG 查询和构建走 Ollama 向量适配器，已有词语覆盖重排走同一重排接口。对话和本地评测使用归一后的分块结果，现有云端评测经凭据守卫使用完整结果。保存的请求与旧用量字段沿用现有格式。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` 分别筛选 `model_runtime_local_capabilities`、`personalized_cloud_wire`、`personalized_mock_generation_publication`、`personalized_removing_connection`、`rag_evaluation_runs_real`、`parser_handles_split_utf8`，共 6 项通过。`cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml` 通过。HTTP 走隔离回环固定响应，未调用真实模型。
- 下一片 F1 本机目录与资源管理，F2–F7 待实施。当前无开发阻断。

## 2026-11-17 E7 固定检索评测

- 评测新增「检索评测」入口。`datasets/cortex-rag-v1/suite.json` 固定 11 份文件、知识与来源版本、基准日、图谱关系、9 道题及回复。`benchmark/rag_evaluation/` 在内存数据库调用现有检索和资料装配流程，支持关闭查找、FTS5、固定向量混合、混合重排、关系和按日期六项策略。两侧采用 8192 上下文设置，报告保留题库、固定知识和评分版本、候选、最终资料、关系及实际算法。固定回复回放的原文支持口径与真实生成判分独立列示。
- `0017_rag_evaluations.sql` 增加受工作区约束的不可变报告。每轮完整计算后单次写入，旧报告可独立读取并校验内容指纹。测试包含失败迁移回滚、可读备份、旧工作区保留以及阶段 E3–E6 路径。
- 首轮图谱候选受共同容量拦截，将两侧统一固定为 8192 后复验。实际跨文件题的资料召回为词语策略 1/3、关系策略 3/3，含一条两跳路径。指定旧日期命中 v1，当前版本引用回放因版本不符得 0。重跑除真实耗时外逐题相同。新增的无依据回复验证拒答、引用和原文支持规则能判失败。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` 分别筛选 `rag_evaluation`、`knowledge_scope_v15_and_v16`、`query_route_retains_private_lookup`、`knowledge_graph_retrieval`、`rag_chunks_recall`，共 6 项通过。`cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`、`npm --prefix apps/desktop run build`、`git diff --check` 通过，保留 Vite 包体提示。
- 在 `apps/desktop` 使用本地 Vite 并运行 `node node_modules/@playwright/test/cli.js test --project=edge rag-evaluation.spec.ts --workers=1 --retries=0 --output=../../artifacts/rag-e7-final`。模拟 IPC 验证策略互斥、运行、历史回读、证据与评分展开，检查三种尺寸浅深主题。首次定位断言同时匹配隐藏及可见条目，收窄范围并复验。截图使用界面测试数据。
- 本轮无真实模型调用和可见桌面窗口。真实生成对照及模型判分尚未接入，原生 WebView2 仍待验证。下一切片为 F0 统一模型能力接口，当前没有 F0 开发阻断。

## 2026-11-16 E6 查找过程检查

- `retrieval.rs`、`rag.rs`、`db.rs` 将真实查找步骤和分段用时写入既有记录。候选保留排名、重排得分、来源文件版本和行号，包括被预算淘汰的候选。匹配片段显示实际命中行，整条知识来源标为摘录范围。旧记录按原字段显示。数据库迁移号为 16。
- 新增 `RetrievalInspection.tsx`，供回答依据、处理记录和执行图共用。可以打开指定版本原文并通过有效引用定位对应回答。界面区分已准备资料与模型服务收到请求的情况。每条资料显示实际文字字节数，整次输入与输出 Token 使用模型服务回报。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` 使用 `rag_chunks_recall`、`query_route_retains_private_lookup`、`knowledge_graph_retrieval`、`effective_context_snapshot_scope` 各筛选 1 项，4 项通过。覆盖预算排除仍保留原文行、混合查询、关系路径与旧快照。`cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`、`npm --prefix apps/desktop run build` 和 `git diff --check` 通过。Vite 保留包体提示。
- 在 `apps/desktop` 用后台 Vite `node node_modules/vite/bin/vite.js --host 127.0.0.1 --port 1420 --strictPort` 启动。`node ../../artifacts/rag-e6-qa/browser-review.cjs` 无头专项通过，模拟 IPC 核对预算排除、打开旧文件版本、实际用量的显示、执行图与引用定位、未发送请求。检查 1366×768、1600×900、1920×1080 浅深两套并保存截图，复核浅色证据页和深色执行图。`node node_modules/@playwright/test/cli.js test --project=edge knowledge-scope.spec.ts --workers=1 --retries=0 --output=../../artifacts/rag-e6-browser` 1 项通过。
- 用 `cargo rustc --offline --features tauri/custom-protocol --manifest-path apps/desktop/src-tauri/Cargo.toml --bin cortexos-desktop -- --emit=link=<隔离程序绝对路径>` 生成原生验证程序。在 `apps/desktop` 用 `alternate-desktop.py` 启动 `rag-native.spec.ts`，确认程序窗口在非活动桌面，CDP 连接超时。使用 `TAURI_CONFIG` 隐藏、不抢焦点、不入任务栏的独立程序再次运行 `node node_modules/@playwright/test/cli.js test --project=desktop rag-native.spec.ts --workers=1 --retries=0 --output=../../artifacts/rag-e6-native-hidden`。CDP 已连通，未能取得 WebView2 页面。两次原生检查均未完成界面验收，保留错误记录。
- 本轮验证使用固定响应与示例数据。原生 WebView2 与真实模型联验待办。下一步是 E7 固定知识快照和评测对照，目前无 E7 开发阻断。

## 2026-11-16 E5 查询路由

- `retrieval.rs` 统一选择跳过、FTS5、混合、混合后重排和指定版本范围，并按明确的关系短语或用户设置叠加图谱查找。`commands.rs` 在生成向量前读取路由，`db.rs` 在使用资料时复核设置并保存计划、实际使用的方式和回退原因。纯问候及无可查文字跳过自动查找；含工作区问题的问候继续查找。关闭自动查找后手动资料仍可选用。数据库迁移号为 16。
- `RetrievalSummary.tsx` 显示跳过原因、语义查找预览和回退说明。图谱选项注明每次沿关系查找。首轮截图发现切换知识范围后旧分支提示仍在，已在范围变化时清除并复验。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` 分别筛选 `query_route_retains_private_lookup`、`knowledge_graph_retrieval`、`rag_chunks_recall`、`retrieval::tests::`，依次通过 1、1、1、4 项，去重共 6 项。使用临时数据库和固定向量验证私有资料、空输入、禁用与手动选择、版本范围、索引变化、向量失败、重排和图谱路径。`cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`、`npm --prefix apps/desktop run build` 和 `git diff --check` 通过，Vite 保留包体提示。
- 在 `apps/desktop` 后台启动 `node node_modules/vite/bin/vite.js --host 127.0.0.1 --port 1420 --strictPort`，执行 `node node_modules/@playwright/test/cli.js test --project=edge knowledge-scope.spec.ts --workers=1 --retries=0 --output=../../artifacts/rag-e5-reviewed`，1 项通过，总计 4.6 秒。模拟 IPC 验证知识范围、图谱选项、预览策略和跳过说明，复核 `route-preview-1366.png`、`route-skipped-1366.png`，按会话结束临时服务。
- 本轮后台完成，原生 WebView2 和真实向量、生成联验待办。下一步是 E6 查找过程检查，其后是 E7 固定评测。

## 2026-11-16 E4 图谱检索与知识簇

- 新增 `knowledge_graph_retrieval.rs` 和 `KnowledgeGraphOptions.tsx`。对话资料区可开启一至两跳的关系查找。预览、回答依据和执行记录显示实际关系方向、修订、文件版本、行号和入选状态。反驳关系可补充对方资料，并在查找路径中作为终点。知识簇概览列出相连知识组的关系计数和代表原文，超过上限时提示。按事实日期查找时筛选对应知识版本，路径标注关系来自当前图谱。
- 新增 `0016_knowledge_snapshot_graph.sql`，事务内扩展快照关系，保留已冻结的反驳记录。旧快照按保存时间回填关系，实验分支继承基准关系。停用的文件在未来查找中排除，历史执行仍显示当时的路径。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` 分别筛选 `knowledge_graph_retrieval`、`knowledge_scope`、`rag_`、`retrieval::tests`，依次通过 1、3、6、4 项，去重共 13 项。覆盖反向和跨文件路径、两跳上限、跨区排除、快照与历史执行、停用来源、旧库回填及失败回滚。`cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`、`npm --prefix apps/desktop run build` 与 `git diff --check` 通过。Vite 保留包体提示。
- 在 `apps/desktop` 用 `node node_modules/@playwright/test/cli.js test --project=edge knowledge-scope.spec.ts --workers=1 --retries=0 --output=../../artifacts/rag-e4-reviewed` 运行无头浏览器专项，1 项通过，总计 4.2 秒。通过模拟 IPC 验证开关传递、实验分支、知识簇概览和跨文件路径。查看 `clusters-1366.png` 与 `graph-path-1366.png`。
- 本轮在后台验证，未启动可见窗口或调用模型。原生 WebView2 与真实模型联验待办。下一步进入 E5 查询路由，之后是 E6 查找过程检查和 E7 固定评测。

## 2026-11-15 E3 第二片实验分支、反驳关系与替代版本

- 新增 `0015_knowledge_experiments.sql`、`knowledge_conflicts.rs` 和 `KnowledgeBranchForm.tsx`。在对话的知识范围内从快照创建分支，替换基准内的指定版本并记录原版本及保存时的审批状态。草稿只在被选择的分支中进入上下文。
- 保存快照时固定反驳关系修订，实验沿用基准关系。迁移按早于快照保存时间的最后一次关系修订回填旧快照。自动查找可补入预算内的反驳资料，保留两方实际入选状态，执行记录注明当前图谱或固定快照。替代版本按同节点审批顺序确定。新版失效后按日查找不会重新启用已接替的旧版。当前范围的后续请求排除曾用到过期、被接替或已停用知识的历史轮次。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` 分别筛选 `knowledge_scope`、`rag_` 与 `retrieval::tests`，3、6、3 项通过。覆盖跨区拒绝、草稿隔离、冻结边版本、预算不足、来源停用、旧库回填、模拟迁移失败回滚与可继续迁移。Rust `cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`、`npm --prefix apps/desktop run build`、`git diff --check` 通过。前端仍有 Vite 包体提示。
- 在 `apps/desktop` 用 `node node_modules/@playwright/test/cli.js test --project=edge knowledge-scope.spec.ts --workers=1 --retries=0 --output=../../artifacts/rag-e3-branches-reviewed` 执行无头浏览器专项，1 项通过，3.5 秒。模拟 IPC 验证选择基准、保存并使用草稿分支与浏览反驳记录。查看 1366×768 截图，修复了“回到最新内容”遮挡表单的问题并复验。
- 本轮未启动可见窗口或调用模型。E3 功能开发完成，原生 WebView2 和真实模型仍待验证。下一步为 E4 图谱检索。

## 2026-11-15 E3 第一片固定知识快照和事实日期

- 新增 `0014_knowledge_scope.sql`、`knowledge_scope.rs`、`KnowledgeScopePicker.tsx`。资料选择区可保存和选择固定快照，也可按事实日期查找。知识编辑器将日期与草稿版本一起保存，审批后参照日期用于当前与按日查找。日期是本地日历日，结束当天已失效。指定日期排除日期未记录的版本，并保留跳过计数。新增两个精确的命令用于列出及保存当前工作区的快照。
- 预览与实际准备共用范围解析和来源停用检查。上下文快照记录范围、版本及过滤信息。当前查找沿用混合索引，指定日期和快照使用本地词项匹配。显式范围的历史只复用相同版本集合下的完整对话。快照成员版本在保存后固定，来源停用后从后续请求排除，旧请求可读。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib knowledge_scope` 1 项通过。同一问题分别得到东楼与西楼的资料，验证日期边界、未知日期、仅改日期、跨区拒绝、来源停用、不同范围历史隔离和已保存请求不变。相同命令筛选 `rag_` 与 `retrieval::tests`，分别 6 项与 3 项通过，含 v14 注入失败、备份可读及旧向量兼容。
- `npm --prefix apps/desktop run build` 通过，保留 Vite 包体提示。首次无头浏览器截图发现切到快照后仍显示当前手动列表的空态，已修复，并让保存快照后的表单自动折叠。最终在 `apps/desktop` 执行 `node node_modules/@playwright/test/cli.js test --project=edge knowledge-scope.spec.ts --workers=1 --retries=0 --output=../../artifacts/rag-e3-headless-final`，1 项通过，约 5.9 秒。使用模拟 IPC 验证选快照、改日期和预览请求，保存三种窗口尺寸的实际浏览器截图。
- `cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml` 与 `git diff --check` 通过。本轮无可见窗口或模型调用。原生 WebView2、真实模型、实验分支、显式冲突和替代关系仍待后续。

## 2026-11-15 E2 文件片段直接检索

- 新增 `0013_rag_source_chunks.sql`。迁移在事务内重建 FTS5、保留已有向量和快照。修改 `retrieval.rs`、`source_chunks.rs`、`rag.rs` 和 `db.rs`，让两路召回使用同一片段 ID。命中经融合进入预算，引用保留来源版本和行列。更新 `RagIndexSettings.tsx` 的重建范围说明及相关测试。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib rag_` 最终 6 项通过。包括模拟向量的纯语义命中、词法与语义命中同一片段、父片展开、跨区隔离、有效版本切换、停用以及迁移 12 和 13 回滚。初次集成断言列出的原文行号没计入测试文本中的额外换行，已改从原文计算并验证父片展开。
- 同一 Rust 命令分别用 `source_chunk_` 和 `retrieval::tests` 筛选，各 3 项通过。旧 v5 测试最初删掉当前库中的一条迁移记录，库版本不连续。改为建立真正的 v5 夹具，并在事务内插入相互引用的知识版本后通过。Rust `cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`、`npm --prefix apps/desktop run build` 和 `git diff --check` 通过。Vite 保留包体提示。
- 本轮使用无界面验证。完整版 E2 的 WebView2 流程与真机向量质量、资源用量待复核。验证没有启动验收窗口或调用模型。E3 已明确快照与生效日期的实施顺序，尚未接入代码。

## 2026-11-14 Phase E 现状审计、混合查找与文件片段

Phase D 已推送到 `origin/codex/release-0.2.0`，以远端查询核对提交 `8e82c42f616ae41b4a9ff46fdc2ef21c8a733bcd`。本地 Phase E 增量包括 [现状审计](rag-runtime-audit.md) 和 [统一实施计划](rag-runtime-plan.md)。

- E1 新增 `0012_rag_vector_index.sql`、`rag.rs`、`RagIndexSettings.tsx` 和 `rag-native.spec.ts`。工作区通过本地 Ollama 构建隔离向量索引，与既有 BM25 结果融合。生成入口保存实际排序、入选状态和降级原因。新增一个应用任务类别和五条精确的桌面命令。
- E2 新增 `source_chunks.rs`，接入来源目录、已批准知识的发送内容与回答依据。先在批准范围内匹配小片段，有余量再扩大到同节范围。原文位置和匹配信息留在本地快照，缩小模型请求。片段直接参与索引及独立代码文件导入仍待后续。
- `npm --prefix apps/desktop run build` 通过，保留 Vite 包体提示。使用 `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` 分别筛选 `rag_migration_preserves_v11_and_rolls_back` 和 `source_chunk_`，1 项迁移回滚和 3 项切片通过。迁移失败时可读取旧聊天和预升级备份，修复后 schema 为 12。切片测试初轮暴露样本的超长行跨越父片边界，分别验证长行坐标和同节父片后通过。
- 通过 `cargo rustc --offline --features tauri/custom-protocol --manifest-path apps/desktop/src-tauri/Cargo.toml --bin cortexos-desktop -- --emit=link=D:/vibecoding/CortexOS/artifacts/cortexos-rag-e2.exe` 构建隔离原生程序。原生首轮沙箱内的 CDP 超时，允许 WebView2 独立运行后进入测试。E1 修正异步开关回跳与选项对齐。E2 初轮发现在默认预算内，文件原文和审计元数据会重复占用输入，改为模型只收到编号、标题和选定片段，快照保留完整原文坐标。最后从 `apps/desktop` 执行 `npx playwright test --project=desktop rag-native.spec.ts sources.spec.ts --grep 'workspace hybrid retrieval|authorized snapshots' --workers=1 --retries=0 --output=../../artifacts/rag-e2-delivery`，2 项通过，用时 14.3 秒。覆盖维度失配、版本切换、仅语义命中、章节定位、停用与历史快照、有限预算回落到子片、实际请求排除未批准文字。
- 对 Phase D 测试库及 E2 两个隔离测试库用 Python SQLite 只读运行 `quick_check` 和 `foreign_key_check`，均为 `ok`、0 项外键错误。真实服务 `127.0.0.1:11434` 的只读目录查询连接失败，本机模型缓存只看到两个 Qwen 对话标签。没有真实向量生成、本地推理或云调用，未跑全量测试。

## 2026-11-14 Phase D 剩余功能与实际补验

新增 `benchmark/harness/programming/project.rs`、`tasks/scenarios.rs` 和前端 `ReadonlyRuns.tsx`，复用原审核、任务库、Docker、CLI 调度和图谱。修改编程草案、实际验证、固定文件副本、Codex/Claude/Aider 会话、运行报告与 Windows 专项。两条精确 Tauri 命令只运行和读取已发布的项目只读题。schema 仍为 11。范围、限制和逐项证据见 [Phase D 补齐交付](harness-fit-phase-d-completion.md)。

- `cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml` 通过。`npm --prefix apps/desktop run build` 初轮发现 TypeScript 的 `Object.hasOwn` 与目标库不符，改用现有目标支持的检查后通过，保留 Vite 包体提示。Python 源文件 AST 语法检查通过。相关 Rust 文件格式化后，`cargo rustc --offline --features tauri/custom-protocol --manifest-path apps/desktop/src-tauri/Cargo.toml --bin cortexos-desktop -- --emit=link=D:/vibecoding/CortexOS/artifacts/cortexos-harness-phase-d.exe` 成功，1 分 12 秒。
- 在 `apps/desktop`，设 `CORTEXOS_HARNESS_EXPERIMENT=1` 和独立验收程序路径，使用 `npx playwright test --project=harness-experiment --workers=1 --retries=0` 分别检查 `project tasks and native tool requests`、`Codex and Claude edit`、`project regression, SQLite compatibility`、`reopen and export the stored`，依次单项通过约 1.4 分钟、1.3 分钟、1.7 分钟、9.4 秒。首轮助手误点已展开内容后中止，下一轮 Docker Linux 不可用，恢复既有 Linux 服务后通过。固定回复专项和重开导出均无模型推理。
- 取得两项独立授权后先按 `CORTEXOS_D4_APPROVAL=two-model-two-task-aider` 执行 `harness-experiment` 中 `one authorized two-model two-task Aider comparison`，四会话各实际请求一次，执行结果与容器清理已落盘。末尾界面断言因刷新仍选到旧实验失败，脚本改为按实验 ID 选择，已单独复验并保存原生 JSON/CSV，无重跑。随后设 `CORTEXOS_BENCHMARK_PYTHON_REAL=1`、`CORTEXOS_D0_APPROVAL=qwen3-4b-ten-2048`，`npx playwright test --project=benchmark-python-real --workers=1 --retries=0`，1 项通过，5.8 分钟。D0 实际为 9 截断、1 个 Python 语法错误，历史报告指纹保留。
- SQLite 只读审计确认五个验收库完整性、外键和归属无异常。四个由既有 Harness 验收库复制的库保留原任务、事件和非评测业务数据。D0 使用新库。按新增任务归属查询 Docker，无遗留容器。未运行全量单元测试。
- 最终构建重跑 `reopen and export the stored` 专项，通过。独立校验导出内容 SHA-256、CSV 的四条会话及四次请求，结果与存储一致。运行 `node artifacts/phase-d-acceptance/open.cjs`，打开隔离验收副本的同一实验，Windows 窗口可见，PID 26032。系统未授予前置焦点，任务栏可切换。两项操作均未启动模型调用。

## 2026-11-14 Phase D 首版用户验收通过

用户确认当前已交付版本通过验收。已更新实施计划、交付清单及文档索引。原计划尚待完成的扩展任务、真实对照和报告指标继续列在 [交付清单](harness-fit-d4-d5.md)。本轮仅更新文档，未运行测试或模型。

## 2026-11-14 Harness Fit Lab D4–D5 交付与 Phase D 验收

接入串行组合、预算与归属校验、停止和中断恢复、比较报告、3D 任务回放与原生导出。主要文件为后端 `benchmark/harness/session/experiment.rs` 与 `session/experiment/report.rs`、前端 `benchmark/harness/HarnessExperiments.tsx` 和 `HarnessFitLab.tsx`、原生专项 `apps/desktop/tests/harness-experiment.spec.ts` 与保存对话框助手。复用 schema 11、Ollama Provider、任务副本和独立评分器。参见 [D0–D5 范围与证据](harness-fit-d4-d5.md)。

- `npm --prefix apps/desktop run build` 通过类型检查和生产构建，保留包体提示。`cargo rustc --offline --features tauri/custom-protocol --manifest-path apps/desktop/src-tauri/Cargo.toml --bin cortexos-desktop -- --emit=link=D:/vibecoding/CortexOS/artifacts/cortexos-harness-experiment.exe` 生成独立 Windows 验收程序，31.73 秒。
- 从 `apps/desktop` 设置 `CORTEXOS_HARNESS_EXPERIMENT=1`，执行 `npx playwright test --project=harness-experiment --grep 'fixed replies' --output=../../artifacts/harness-experiment-controlled-final`，1 项通过，约 1.2 分钟。两种模型配置各两次，每题 7/7，覆盖停止、强制重启及越权拒绝。实际截图含三种尺寸的浅深主题。设置 `CORTEXOS_D4_EXPORT_SOURCE` 指向已完成批次的隔离库，同项目同过滤条件在 `artifacts/harness-experiment-report-verified/` 独立复读，1 项通过，9.6 秒。
- 用户确认三工具各一题后，首次只读预检因 Docker Linux 引擎缺失退出，尚未启动推理。现有引擎恢复后，只读核对模型指纹及无已加载模型，设置 `CORTEXOS_HARNESS_EXPERIMENT=1`、`CORTEXOS_D4_APPROVAL=three-adjust-tasks`，从 `apps/desktop` 执行 `npx playwright test --project=harness-experiment --grep 'one authorized local comparison' --workers=1 --retries=0 --output=../../artifacts/harness-experiment-real`。1 项通过，总计 2.3 分钟。批次 `570c69d2-326c-4949-8a76-68ea196d0a47` 三题各通过 7/7，共 8 次真实推理请求，无重试。运行后只读 `/api/ps` 为空。
- 对固定回复及真实批次分别执行 `py -3.13 -X utf8 artifacts/harness-experiment-audit/verify.py` 并传对应结果目录名。实际核查 SQLite 完整性、外键、既有 26 条任务及事件、其他业务表和计划哈希均通过，两批的执行及评分容器均无遗留。
- 真实结果原生窗口已打开。启动辅助首次因按 `region` 查找无语义容器超时，重新连接同一窗口后改用标签定位，实际保存 JSON 与 CSV，核对三条会话、8 次请求及 SHA-256。`node artifacts/harness-experiment-actual-acceptance/review-trace.cjs` 实际打开 Aider 候选节点，查到 `return max(n, 0)`，截取原生 3D 画布并返回报告。Windows 已显示窗口，系统未授予强制前置。证据在 `artifacts/harness-experiment-actual-acceptance/`。

已完成 Phase D 首版的组合与报告闭环，当前交回人工验收。本批每个工具只有一个样本，未给出领先组合。D0 独立补验的历史截断项继续保留待复核，D2 的跨文件、仓库级和其他扩展任务见交付清单。当前 D4–D5 没有运行环境阻断，未运行全量单元测试或第二批真实模型。

## 2026-11-13 Aider 真实单题及离线回放修复

收到一题的明确授权。只读核对指纹与空加载列表后，在 `apps/desktop` 用 `npx playwright test --project=harness-aider-session --grep 'one explicitly authorized' --retries=0 --workers=1 --output=../../artifacts/harness-aider-session-real` 执行一次。Playwright 1 项通过，25.1 秒，总计 30.6 秒。真实会话发出 1 请求，输入 840、输出 21 Token，Ollama 总耗时 8.27 秒。模型回复用了目标完整相对路径，原校验仅接受 basename，任务状态 failed，评分未开始。记录、事件哈希和清理核对通过，既有 26 条任务和事件未变。运行后 `/api/ps` 返回空列表。

修改 `aider_session.py` 和专项，接受同一目标的准确路径别名并规范化给 Aider。默认桌面二进制被旧验收窗口占用，普通构建返回 Windows os error 5。使用 `cargo rustc --offline --features tauri/custom-protocol --manifest-path apps/desktop/src-tauri/Cargo.toml --bin cortexos-desktop -- --emit=link=<artifacts/cortexos-aider-replay.exe绝对路径>` 成功生成独立修复版。用 `CORTEXOS_AIDER_TEST_EXE` 和 `CORTEXOS_AIDER_REPLAY_REPLY` 指向独立程序及真实记录，执行 `npx playwright test --project=harness-aider-session --grep 'fixed reply' --retries=0 --workers=1 --output=../../artifacts/harness-aider-session-replay`，1 项通过，50.3 秒，总计 53.6 秒。实际候选改为 `return max(n, 0)`，独立评分 7/7，越界和恢复复验通过。回放真实推理数为 0，原会话不覆写。D4、D5 仍待后续。


## 2026-11-13 D3-3d.2 Aider 单题执行接入

接通固定镜像内的真实 Aider、回环和 stdio、受控 Provider、任务副本及独立评分；增加三个精确 Tauri 命令，复用单题 UI 和会话存储。新增回复文件名与单函数签名校验，收集时继续核对文件权限和哈希。schema 仍为 11。详情见[Aider 交付](harness-fit-d3-aider-transport.md)。

- `cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml` 通过。
- `npm --prefix apps/desktop run build` 通过，保留包体大小提示。
- `cargo build --offline --features tauri/custom-protocol --manifest-path apps/desktop/src-tauri/Cargo.toml` 通过。
- 从 `apps/desktop` 设 `CORTEXOS_HARNESS_AIDER_SESSION=1`，运行 `npx playwright test --project=harness-aider-session --grep 'fixed reply' --output=../../artifacts/harness-aider-session-final`，1 项通过，49.5 秒，总计 51.7 秒。首轮 Aider 误用 Claude relay 导致 FileNotFoundError，失败单独保留；修正后实测一题 7/7、越界文件名和额外顶层函数拒绝、停止恢复，三尺寸浅深截图六张。
- `py -3.13 -X utf8 artifacts/harness-aider-session-audit/verify.py` 通过，数据库完整性正常，旧任务及事件 26 条不变，新增 5 条记录的计划与事件哈希通过。按 Aider 会话用途查询 Docker 容器返回空。

共享会话改动后，`CORTEXOS_HARNESS_CLAUDE_SESSION=1` 下运行 `npx playwright test --project=harness-claude-session --grep 'controlled transport' --output=../../artifacts/harness-aider-claude-regression`，1 项通过，37.3 秒，总计 42.6 秒。

`CORTEXOS_HARNESS_SESSION=1` 下运行 `npx playwright test --project=harness-codex-session --grep 'controlled transport' --output=../../artifacts/harness-aider-codex-regression`，1 项通过，37.2 秒，总计 39.3 秒。按五条新增任务的归属标签逐条确认执行与评分容器无残留。打开独立 QA 验收窗口，PID 9684，窗口可见，系统未授予强制前置。

已发出真实模型单题确认弹窗，待明确回复。固定回复专项实际模型调用数为 0。


## 2026-11-10：Aider Provider 转发函数接入

用户要求开始接入。修改aider_channel.rs，新增verify_aider_provider.py，复用Ollama目录/chat方法，新增取消与有界HTTP响应，未改旧session文件或历史计划。`cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`通过，5项未接入代码警告保留。实际 `py -3.13 -X utf8 scripts/harness/verify_aider_provider.py`：首轮失败定位旧false通知误判取消，独立保留first-cancel-failure.log；改为只在true/通道关闭时取消后，重跑1项通过、100过滤，服务端核对唯一假chat请求的固定预算参数。没有本地模型推理、下载或云端；没有完整单测、UI改动或桌面更新。证据及未完成项见[Aider进度](harness-fit-d3-aider-transport.md)。剩余容器stdio/worker、受限副本/独立评分、存储/UI、原生回归及另批真实单题，d.2尚未完成，无新增外部阻断。

## 2026-11-10：D3-3d.2 后端协议边界

用户要求继续，新增aider_channel.rs及harness/mod.rs注册，具体限制与未接入项见[Aider进度](harness-fit-d3-aider-transport.md)。保留旧session.rs、bridge、descriptor及文件IO内容，无数据库/UI/依赖修改。实际 `rustfmt --edition 2024 apps/desktop/src-tauri/src/benchmark/harness/aider_channel.rs` 完成。首次cargo test带--exact导致0项匹配，未计通过；移除后执行 `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml aider_boundary_budget_and_response`，1项通过、99项过滤，编译通过，保留4项尚未接入的dead_code警告。此专项是模型/执行信任边界所必需，不运行完整单测。没有模型调用、下载、前端构建或原生界面检查。

已完成本步协议/预算边界函数；剩余Provider调度与真正取消/超时、受限副本和独立评分、存储/UI和原生回归，最后实际模型单题另弹窗。尚无完整Aider会话入口，d.2未完成；没有新增安装或模型授权阻断，当前待办是实现接线。未提交推送。

## 2026-11-10：D3-3d.2 协议观察与固定回复编辑

用户验收d.1后继续。复核development、开发规范、实施计划和既有session桥；沿用ponytail复用原则。新增 `scripts/harness/probe_aider_transport.py` 与 [进度说明](harness-fit-d3-aider-transport.md)。实际运行 `py -3.13 -X utf8 scripts/harness/probe_aider_transport.py`：配置错误退出2保留，修正后观察固定400请求；再次改为固定200回复后真实编辑与明确断言通过。外层docker命令退出0不代替内部程序结果，均分别记录。新增明确镜像指纹/本地端点/路径/请求数和大小拒绝后，执行最终脚本复核。无模型、无下载、不运行候选代码，无单元测试或前端修改。d.2生产接入尚未完成，剩余详见进度说明；未提交推送。

## 2026-11-10：D3-3d.1 Aider 无模型检查交付

实际范围与文件见 [交付说明](harness-fit-d3-aider.md)。沿用 ponytail 最小复用；没有新数据库迁移或新UI框架。工具准备已获准，用户已关闭旧窗口；全程无本地推理、云端模型、模型下载或个人登录。未提交推送，保留原D1–D3未提交变更。

实际执行（仓库根，注明者除外）：

- 隐藏启动已有 Docker Desktop；只读发现上次准备容器退出137、OOMKilled=false，仅清理指定任务容器，未推断退出原因。
- `py -3.13 scripts/harness/prepare_aider.py`：首次仍在 Docker cp 阶段失败，完整输出保留。修复为容器内固定代码导出有界归档后重跑成功：108个wheel、125519775字节、全部官方文件hash一致；最终Docker build使用network=none、固定BASE digest、pull=false，pip check无依赖冲突。最终镜像及锁指纹见交付说明。没有放宽实际运行限额。
- `npm --prefix apps/desktop run build`：类型检查、Vite构建通过，保留大包提示；源码顺序调整后重新构建通过。
- `cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`：通过（11月9日）。`cargo build --offline --features tauri/custom-protocol --manifest-path apps/desktop/src-tauri/Cargo.toml`：通过；固定镜像/锁及版本判定修改后分别重建通过，末轮31.66秒。
- 在 `apps/desktop`，追加现有Docker CLI路径并设置 `CORTEXOS_HARNESS_AIDER_RUNTIME=1`，执行 `npx playwright test --project=harness-aider-runtime --output=../../artifacts/harness-aider-native`：1通过，30.7秒/总34.0秒。流程检查通过，但版本观察误判为blocked：实际 `python -I -m aider --version` 输出 `__main__.py 0.86.2`。修正明确匹配此实际入口输出，不改写已保存结果。
- 同目录，额外设置 `CORTEXOS_AIDER_PRIOR_DATA` 为首轮独立QA库目录，执行 `npx playwright test --project=harness-aider-runtime --output=../../artifacts/harness-aider-native-final`：1通过，18.0秒/总20.3秒。版本/10项参数/七项外层检查通过，错误任务、镜像、确认、额外策略、跨用途/工作区请求拒绝，停止、强杀恢复清理通过。旧Claude与首轮Aider记录逐项相等。没有运行完整单测集。
- `py -3.13 -X utf8 artifacts/harness-aider-audit/verify.py`：integrity ok、外键空，23条旧任务及其事件逐列相同；其余业务表与schema/迁移一致，3条新检查计划及事件hash正确。设置/新增QA工作区属于预期变化，生产库未访问。
- 两次 `docker ... ps -a` 分别按本片用途label和准备容器名前缀检查，输出为空。两份Python脚本AST检查通过。
- 原生截图6张：1366×768、1600×900、1920×1080浅深主题，无横向溢出；查看1366浅色、1920深色。不是系统150%缩放、读屏或手动拖动验证；不另跑浏览器专项。
- `node artifacts/harness-aider-acceptance/launch.cjs`：打开独立QA结果，读取记录和历史数不变，无新模型/容器；`powershell.exe -NoProfile -ExecutionPolicy Bypass -File artifacts/harness-aider-acceptance/show.ps1 -AppProcessId 24804`：visible=true，foregroundGranted=false。窗口已可见，系统未授予强制前置。

D3-3d.1 完成，停在人工验收。剩余：d.2受控Aider模型通道、固定回复/单题与独立评分，D4组合对照，D5报告/图谱。实际门槛为工具内层能力和模型通道尚未验证，canRun=false，任务Not Run；无新模型批次授权。准备脚本/窗口启动辅助曾遇到编码问题，UTF-8读取后修复；没有把辅助失败列为通过。


## 2026-11-10：D3-3d.1 Aider 准备恢复（进行中）

用户已批准 Python 3.12、Aider 0.86.2 与基础依赖准备，并关闭验收窗口。新增固定用途和共享运行前检查 UI；未增加数据库迁移。已执行 `cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`、`npm --prefix apps/desktop run build`，均通过；前端保留大包提示。实际 CLI、原生专项、桌面构建尚待完成，不提前标记通过。

准备脚本初次遇到 Docker 未启动、只读根下临时目录缺失、Windows 日志解码和停止容器 tmpfs 无法复制问题，已分别定位，日志保留在 `artifacts/harness-aider-runtime/last-command.log`。改为容器运行期间下载和复制后清理；未放宽运行检查限额。跨日恢复发现旧准备容器已退出 137，OOMKilled=false，无法据此推断退出原因；只清理该已识别的任务容器。隐藏启动已安装 Docker Desktop，继续原授权工具准备。无模型、个人登录或项目代码执行。


## 2026-11-09：进入Aider工具准备，许可前核对

用户在D3-3c交付后要求继续。复核原D3顺序、development、开发规范、实施计划和既有runtime/session；沿用ponytail最小复用。查阅官方脚本、参数、Docker说明和PyPI元数据，确认Aider0.86.2要求>=3.10,<3.13，主wheel大小377009及SHA-256。宿主初次PyPI请求TLS认证失败，正常升级权限只读请求成功，未关闭TLS校验。形成[Aider方案](harness-fit-d3-aider-plan.md)：先独立Python3.12工具环境/无模型检查，原3.13评分不改；后接受控单题。当前未下载镜像/包，未运行Aider或任何模型，无应用源码修改；不跑无关单测。剩余本片工具准备/无模型验收、d.2真实通道/单题、D4组合、D5报告图谱。

## 2026-11-09：D3-3c真实单题收尾与验收窗口

用户批准的2026-11-05一次真实Claude/Qwen会话已完成：4请求、7/7独立Python断言通过、实际文件修改及失败工具命令保留、未写回原项目。专项1条通过约1.1分钟；具体命令、Token/时间口径和限制见[交付说明](harness-fit-d3-claude-session.md)。上次窗口启动因自动审批额度失败未执行，本轮正常审批恢复后仅审计历史和打开窗口，无新模型调用。只读数据库对照19条旧任务及事件、其他表一致，hash通过。验收启动脚本成功，PID19724，visible=true/foregroundGranted=false；截图已查看、选中结果与快照一致、历史数量未变。无业务代码修改，不重复单测；未提交推送。停在D3-3c验收，剩余Aider、组合对照、报告/图谱。

## 2026-11-05：D3-3c实现与无模型验证，真实单题待许可

用户已关闭验收窗口并要求继续。沿用ponytail最小复用和现有UI规范，完成[Claude本地单题通道](harness-fit-d3-claude-session.md)所列文件、用途隔离、真实CLI固定回复专项和Codex回归。执行命令、首次失败及修复、截图和数据审计见该说明；实际两项协议边界通过、前端/内嵌桌面构建通过、Claude原生42.8秒与Codex回归39.7秒通过。没有跑完整单测，没有任何真实推理/云端/模型下载。19条旧任务及全部事件不变，5条新受控会话hash核对，其他领域表内容不变，两类会话容器无残留。

Docker原先未启动，隐藏启动既有Docker Desktop后Linux引擎29.9.2可读。Ollama原先拒绝连接，隐藏启动既有serve PID4920，仅回环、关闭cloud、单并行；只读tags/ps核对实际模型指纹且loaded列表为空。本批模型许可弹窗已发出，等待答复，不将历史许可复用。当前未启动最终验收窗口，未进入Aider/D4/D5，未提交推送。


## 2026-11-03：D3-3b Claude依赖、独立兼容策略与实际Bash隔离

用户在D3-3a后要求“往下做”，按[本片方案](harness-fit-d3-claude-sandbox-plan.md)继续；实际范围、文件清单及限制见[交付说明](harness-fit-d3-claude-sandbox.md)。复核AGENTS、development、开发/实施规范和现有调用链，沿用ponytail的最小复用和ui-ux-pro-max的标签/确认/真实反馈原则，没有新设计系统或页面重排。git根仍D:/vibecoding/CortexOS，保留原D1–D3未提交改动，不提交/推送，不读取Gittoken。用户先后明确批准两项独立范围：官方Debian依赖镜像；最多三组固定内核规则对照且证据通过后仅用于Claude。没有本地推理、向量、云端、个人登录或项目任务。

### 准备、只读诊断与真实失败

实际查阅Claude CLI/headless/sandbox文档、官方SRT源码、Debian bubblewrap/socat、对应bubblewrap0.12.0源码及Linux user_namespaces/no_new_privs。只读mmap检查已缓存的固定CLI，发现max-turns参数定义；这不作为实际解析或预算执行证据。首次`py -3.13 -X utf8 artifacts/harness-claude-sandbox/inspect.py`因Docker引擎管道不存在退出1，未创建容器。只读确认已有Docker Desktop路径后，`Start-Process -FilePath <现有程序> -WindowStyle Hidden`启动，无安装/系统设置。再执行同命令退出0、自有容器清理。

真实系统为Debian13/trixie。两组固定参数（控制组、加max-turns 1组）均在`--cortexos-invalid-option`退出1、stdout空，print/help退出0；证明参数被解析，未启动模型会话。原始inspection.json保留。不能把这个检查当实际模型回合预算已执行。

用户批准依赖下载后，从根执行：

```powershell
$env:PATH = "$env:LOCALAPPDATA\Programs\DockerDesktop\resources\bin;$env:PATH"
py -3.13 -X utf8 scripts/harness/prepare_claude_sandbox.py
py -3.13 -X utf8 artifacts/harness-claude-sandbox/inspect.py --dependencies
```

构建前核对原Claude标签准确Image ID，使用HTTPS Debian官方源、已有archive keyring及APT签名/包hash校验。实际下载约10.1MB索引、536kB包；bubblewrap0.12.0-1~deb13u1、socat1.8.0.3-1+deb13u1、libwrap0 7.6.q-36新增，0升级/删除、无procps推荐包。debconf按无TTY降为Noninteractive，不当作安装失败。镜像2258d386fb20be9cc16f1d6867e1e34cc6d3c2e13f33ca07af7fbdc370dba89f构建退出0，原镜像55de5386...不变。仅APT准备阶段联网，不称本次依赖构建为离线；运行检查全部network=none。包hash/版本/来源实际读取保留。依赖可用但原默认bwrap仍因namespace权限退出1，自有容器已清理。

### 三配置组与能力字段审查

`py -3.13 -X utf8 artifacts/harness-claude-sandbox/prepare-review.py`只准备独立评审文件，不应用配置。基线为已核验的官方Moby快照，明确不是安装引擎过滤器导出。用户另行确认具体unshare/clone/mount/umount2/pivot_root/chroot及最多三组可信无模型对照后执行：

```powershell
py -3.13 -X utf8 scripts/harness/check_claude_seccomp.py
```

三组（显式官方基线、userns、bubblewrap）均记录准确归属并完成清理。基线及userns下实际Claude Bash结果仍为bwrap错误；即使CLI最终退出0也没有标通过。bubblewrap候选可运行真实Bash工具，目录/网络等保护通过，但初轮将内层CapBnd与实际可用能力混作全0条件，因此检查未通过，comparison.json完整保留。

同一第三组规则内补充只读字段和反向检查，没有增加第四种配置或放宽权限，分别执行：

```powershell
py -3.13 -X utf8 scripts/harness/check_claude_seccomp.py --candidate-details
py -3.13 -X utf8 scripts/harness/check_claude_seccomp.py --candidate-boundary
```

两次各有一个自有诊断容器，独立comparison-details.json/comparison-boundary.json保存，不覆盖初轮或声称总共只有3个容器。明细确认外层CapEff/CapBnd=0，内层UID65534、CapEff/Prm/Amb=0，CapBnd=000001ffffffffff。按Linux文档区分能力上限与已有能力，新固定协议另加nestedUserNamespaceDenied：unshare(NEWUSER)与clone(NEWUSER)实测均EPERM。外层全0、内层无可用能力、NNP=1及13项反向保护全部通过；原未通过的结果保留，不将非零CapBnd写成0。初轮部分memory max回收计数非0，所有实际OOM/oom_kill均0，256MiB限额未扩大。

固定夹具仅在容器内部回环返回最多两条预设Messages消息，唯一命令为可信隔离检查；没有连接Ollama/外部模型或个人认证。不安装额外SRT/模型来冒充当前Claude程序。实际CLI使用严格沙盒配置和固定命令的明确允许规则，没有bypassPermissions、acceptEdits、未隔离重试或弱化嵌套。夹具模型名、Token及CLI估算费用仅为调试原文，不是真实模型计费或成绩。

### 实施与实际可执行检查

复用原worker、计划/事件/用途/归属及临时文件机制，Claude固定策略ID/描述/清理文件名单独限定；Codex公开ID仍不接受Claude，反向亦然。新计划claude-runtime-check-2、probe2保存源码hash，旧check1/probe1仍按原定义读取。基础/依赖镜像与默认/兼容策略选择及确认沿用现有控件，当前记录单独标实际策略及旧版。不新增Tauri命令/权限、数据库迁移、依赖、锁文件或CSS，不修改知识/聊天/3D逻辑。

- 仅对修改的Rust文件执行`rustfmt --edition 2024 --config skip_children=true ...`；`cargo check --offline`通过55.14秒。`npm.cmd run typecheck`通过。
- 必要边界`cargo test claude_gate_ --lib --offline -- --nocapture`：2通过/96过滤，编译31.86秒、测试0.02秒，覆盖缺失/伪造证明不能通过、模型调用协议拒绝、两个公开策略互拒及临时文件互不删除。既有`cargo test codex_policy_is_fixed --lib --offline -- --nocapture`：1通过/97过滤，编译9.24秒、测试0.03秒。没有跑无关全套单元。
- `npm.cmd run tauri -- build --debug --no-bundle`通过，Vite15.84秒、Rust41.19秒；index-BoV_YWSl.js、CSS仍index-CCwtNeOJ.css，保留已有chunk大小警告。npm新版本提示没有触发升级。

从apps/desktop执行：

```powershell
$env:PATH = "$env:LOCALAPPDATA\Programs\DockerDesktop\resources\bin;$env:PATH"
$env:CORTEXOS_HARNESS_CLAUDE_RUNTIME = '1'
$env:CORTEXOS_HARNESS_CLAUDE_PREPARED = '1'
$env:CORTEXOS_CLAUDE_COMPATIBLE = '1'
npx.cmd playwright test --project=harness-claude-runtime --output=../../artifacts/harness-claude-sandbox-native
```

**1通过，场景32.3秒/总34.8秒**。SQLite backup原D3-3a QA库，实际WebView2/CLI/固定回复，旧Claude和Codex完整结果不变、错误确认/hash/策略/用途/工作区拒绝、完成、停止、暂停自己容器后强杀自己QA进程、重开interrupted及手动清理均通过。新完成5c478974...、停止0335ac13...、中断后清理96091ada...。最终sandboxReady=true仅为固定无模型检查，canRun=false，模型通道未验证，项目任务Not Run。保存1366×768/1600×900/1920×1080浅深6图及主题/无横向溢出断言，人工查看1366浅色、1920深色；没有新Windows缩放/读屏实测。

`py -3.13 -X utf8 artifacts/harness-claude-sandbox-native/verify.py`退出0：integrity ok、FK空、schema11/迁移checksum相等，11条旧Claude/Codex检查/会话及事件逐列不变，新旧6条Claude计划/事件hash及新检查源码hash相符，自有容器和临时策略文件均无残留；messages/knowledge_nodes/knowledge_versions/execution_runs/context_snapshots/benchmark_runs均0且不变。database-verification.json保存。没有模型加载/推理或生产数据写入。

### 验收窗口与阶段剩余

`node --check artifacts/harness-claude-sandbox-acceptance/launch.cjs`通过；执行该launch.cjs打开新版进程17176、CDP63305，独立QA库，定位项目适配→4.测试任务→Claude运行前检查。切换旧版记录“旧版检查”可见，再回新通过记录，selected完整对象及history数量保持。只读操作，不新建检查、容器或模型。legacy.png/ready.png保存，ready已查看。`powershell.exe -NoProfile -File artifacts/harness-claude-sandbox-acceptance/show.ps1 -AppProcessId 17176`确认visible=true、foregroundGranted=false；窗口保留，任务栏可切换，不声称抢到前台。

本片计划/交付、实施、数据/UI/测试与文档索引同步。`git diff --check`通过，仅已有CRLF归一提示；标准库AST检查4份Python源文件语法通过，本片两份文档3处相对链接有效。当前固定环境的D3-3b门槛已交付，停止验收；真实模型通道及任务仍阻断于未实现的D3-3c。剩余：D3-3c受控本地Messages/实际预算/唯一单题（模型逐批弹窗）、D3-3d Aider、D4组合对照、D5报告/图谱。完整仓库/跨文件、更多模型和长期压力继续后置。

## 2026-11-02：D3-3a 获准工具准备，真实无模型检查交付

用户明确答复“允许下载并完成无模型检查（推荐）”，覆盖官方Linux Claude Code 2.1.309、项目缓存、已有Python镜像离线构建及无模型版本/参数/隔离观察。本轮没有系统安装、个人认证、推理、向量生成、项目任务或付费API，不扩展Docker权限。按[本片计划](harness-fit-d3-claude-plan.md)完成[交付与阻断说明](harness-fit-d3-claude.md)，停在D3-3a验收。下文同日“工具准备待确认”条目保留为取得本次答复前的历史记录。

### 准备命令与失败记录

`git rev-parse --show-toplevel`再确认D:/vibecoding/CortexOS。保留D1–D3已有未提交改动，未提交、推送或读取Gittoken。

首次PowerShell缓存路径保护循环误用`Split-Path -LiteralPath ... -Parent`，重复参数集错误；停止自己会话59235（Ctrl-C、退出1），当时尚未下载。改用现有Python `guard()`检查目录及固定目标，无链接/联接后建缓存。获准HTTPS下载使用`Invoke-WebRequest`固定URL/OutFile，二进制Timeout900秒；另取同版本manifest.json、manifest.json.sig和官方keys/claude-code.asc，文件仅在artifacts/harness-claude-runtime/。

首次`prepare_claude.py --build`因Git MSYS GPG不识别Windows绝对反斜杠路径而在import退出2；未跳过签名或构建。脚本改为受保护缓存cwd下的固定相对文件名、独立verification-keyring、原`--no-options`及禁止自动取钥参数，兼容现有GPG，不读取个人公钥环。修正后从仓库根执行：

```powershell
$env:PATH = "$env:LOCALAPPDATA\Programs\DockerDesktop\resources\bin;$env:PATH"
py -3.13 -X utf8 scripts/harness/prepare_claude.py --gpg E:\Git\usr\bin\gpg.exe --build
```

实际退出0。二进制293469552字节，SHA-256 `4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658`；清单签名和固定主公钥指纹`31DDDE24DDFAB679F42D7BD2BAA929FF1A7ECACE`通过。基础Python ID核对不变；network=none、pull=false、Dockerfile无RUN安装，构建专用镜像`sha256:55de5386970372e49b2d221e15d870e36902c1e5d3e37eab9118fe4e9919f32e`。BuildKit开始有一次http2 pipe closed警告，后续构建完整退出0，不写成构建失败。未准备bwrap/socat或套用Codex策略。

### 真实程序、停止恢复与数据证据

入口实现阶段已经通过类型检查、离线Rust检查、桌面构建、单个必要claude_gate_边界、空镜像原生专项和旧Codex无模型回归，见下节。本次只修改准备脚本的GPG路径及交付文档，未重复无关单元或模型验证。

从apps/desktop执行：

```powershell
$env:PATH = "$env:LOCALAPPDATA\Programs\DockerDesktop\resources\bin;$env:PATH"
$env:CORTEXOS_HARNESS_CLAUDE_RUNTIME = '1'
$env:CORTEXOS_HARNESS_CLAUDE_PREPARED = '1'
npx.cmd playwright test --project=harness-claude-runtime --output=../../artifacts/harness-claude-runtime-prepared
```

**1通过，场景16.2秒/总19.2秒**。真实WebView2和独立QA库（从上次实跑库SQLite backup），实际Claude程序固定探针；确认弹窗、任务/hash/用途/工作区拒绝、错误跨Codex API拒绝、停止、暂停自己容器后强杀自己QA进程、重开interrupted/未清理、手动清理closed均通过。旧完成记录与Codex实跑结果保持。实际三条Claude记录为260a45f2...完成、c7dacdaa...停止、14fc129a...中断后清理，modelCalls=0、taskExecution=Not Run。专项流程通过不代表所有环境条件通过：

- 文件指纹通过，版本命令退出0且输出2.1.309 (Claude Code)。
- 帮助命令退出0，22311字符；普通帮助中未列出`--max-turns`，自动化参数维持blocked。没有据此断言隐藏参数完全不支持，也未删除必要预算条件来换取通过。
- bwrap/socat均null；默认策略下unshare(NEWUSER)返回errno1。没有下载依赖或放宽权限。
- 实际UID65534、CapEff/CapBnd均0、NoNewPrivs1、Seccomp2、无Docker Socket；实际配置无网络/宿主挂载，资源限额保留，memory.events的OOM计数0。没有真正运行Claude内层Bash沙盒，模型通道尚未实现，sandboxReady/canRun始终false。

专项截取1366×768、1600×900、1920×1080 CSS视口各浅深6图并核对主题属性、无横向溢出；实际查看1366浅色与1920深色，没有新系统缩放/读屏实测。证据artifacts/harness-claude-runtime-prepared/内runtime.json及截图。

`py -3.13 -X utf8 artifacts/harness-claude-runtime-prepared/verify.py`首次普通沙盒因启动Docker的WinError5退出1；经执行审批后原命令只读复核退出0：integrity ok、FK空、schema11及迁移checksum不变，三条计划/事件hash正确、相关容器无残留；源库8条Codex检查/会话及事件逐列字节相等，messages/knowledge_nodes/knowledge_versions/execution_runs/context_snapshots/benchmark_runs均0且不变。两份Python AST语法检查通过。只写独立证据database-verification.json，不修改数据库。

### 验收窗口与剩余

`node --check artifacts/harness-claude-runtime-acceptance/launch.cjs`通过。执行`node artifacts/harness-claude-runtime-acceptance/launch.cjs`打开调试版，进程15516、CDP65512，使用本次独立QA库，定位评测→项目适配→4.测试任务→Claude Code运行前检查，选择已有完成记录。再次核对selected完整对象和history数量不变，未发起新检查、代码或模型调用。ready.png已实际查看。

`powershell.exe -NoProfile -File artifacts/harness-claude-runtime-acceptance/show.ps1 -AppProcessId 15516`：visible=true、foregroundGranted=false，窗口保留供验收，不声称已抢到前台，可从任务栏切换。

文档同步实际状态、数据/UI/测试规范和阶段剩余；`git diff --check`通过，仅已有CRLF归一提示。Python标准库只读核对本片计划/交付的3处相对文档链接均有效。当前D3-3a交付后停止。实际阻断为普通帮助缺预算参数证据、bwrap/socat缺失、默认命名空间拒绝；后续D3-3b先解决安全门槛（下载/策略变化另确认）、D3-3c受控Messages/预算/唯一真实任务（每批模型另弹窗）、D3-3d Aider，之后D4组合对照与D5报告/图谱。完整仓库/跨文件、额外模型和云端后置。

## 2026-11-02：D3-3a Claude入口与共用检查流程，工具准备待确认

用户在D3-2c交付后要求“做”。复核新版AGENTS、development.md、开发/实施规范及D3顺序，按[本轮计划](harness-fit-d3-claude-plan.md)先做Claude，再单独接Aider。实际范围/文件/未完成项见[harness-fit-d3-claude.md](harness-fit-d3-claude.md)。本片**尚未完成真实Claude验证**，不标为D3-3a已验收；工具下载弹窗仍待用户答复，不推定其同意。

### 核实与实施

`git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS；保留此前D1–D3未提交工作，没有提交或推送，未读取Gittoken。`rg`少量PowerShell路径通配写法未命中，随后定向读取文件，不把失败当检查通过。本轮继续ponytail的最小复用及ui-ux-pro-max的可见反馈、表单标签/恢复建议，沿用现有样式。

实际查阅Claude官方headless、CLI、setup、sandboxing及Ollama官方Claude接入文档。确认Messages协议不同于Codex Responses；Linux sandbox依赖bwrap/socat，默认缺依赖可能回退，后续须failIfUnavailable并禁止非沙盒重试。文档的64k+建议没有应用到本机4k参数，不运行可能自动下载模型的ollama launch，不读取个人Claude登录或Provider密钥。

官方stable页面web抓取不可访问；获准只读HTTPS的Invoke-RestMethod取得stable=2.1.309及manifest，Linux x64293469552字节/hash4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658。npm registry latest元数据是2.1.316，未用latest替换stable。普通Get-Command未发现宿主Claude，未扫描个人目录。已有Git位于E:/Git，普通权限未读到gpg，进一步只读确认E:/Git/usr/bin/gpg.exe与gpgv.exe存在。没有执行gpg导入/验签或下载公钥。

形成精确下载/缓存/镜像方案后，通过交互弹窗请求仅准备官方Linux工具约293MB、固定Python镜像离线构建、无模型检查的许可。当前没有收到答复，因此**未下载Claude二进制/签名/公钥，未构建Claude镜像、未运行CLI**。prepare_claude.py和Dockerfile已实现固定大小/hash、专用公钥环/固定签名指纹、原有基础镜像核对与离线构建；这些执行路径尚未实测，不声称验签完成。

实现四个精确Claude业务接口、直接已发布函数版本准入和harness_claude_runtime用途。原runtime worker、事件/历史/hash/终态、关闭清理以内部Codex/Claude两个固定枚举复用；旧Codex公开接口、v1/v2读取及原始时间保持，Claude只允许默认Docker策略。schema仍11，无迁移/依赖/锁文件变化，不写聊天、知识或模型成绩。固定Claude探针只包含version/help/环境观察，内层沙盒与模型通道始终未验证，不把依赖/参数当安全执行证明。新增UI沿用任务折叠段/SelectMenu/FitStatus/AppDialog；镜像未准备时禁用检查，错误和下一步可见。

### 实际执行的检查

- `cargo check --offline`通过，59.97秒；`npm.cmd run typecheck`通过。
- `cargo test claude_gate_ --lib --offline -- --nocapture`：**1通过、96过滤**，测试0秒/编译46.21秒。仅新信任边界：错误程序指纹、混入模型调用拒绝，依赖/命名空间存在不能把真正Claude sandbox或模型通道标为已通过；构造数据不当真实环境证据。没有跑无关全套单元。
- `npm.cmd run tauri -- build --debug --no-bundle`通过，Vite15.60秒、Rust47.00秒；index-u9XVt_L9.js，CSS仍index-CCwtNeOJ.css。保留已有大chunk提示，不升级依赖或发安装包。
- Docker初次info无法连接管道；按既有许可隐藏启动已安装Docker Desktop，后续只读info确认linux，无重新安装或系统设置变化。
- 设置`CORTEXOS_HARNESS_CLAUDE_RUNTIME=1`执行`npx.cmd playwright test --project=harness-claude-runtime --output=../../artifacts/harness-claude-runtime-empty`：**1通过，4.5秒/总6.4秒**。没有设置PREPARED，实际仅验证真实WebView2/独立库的空镜像/禁用、输入/hash/用途/工作区拒绝、旧Codex实跑记录原样及三尺寸浅深6图/无横向溢出。没有Claude程序/停止恢复或模型调用，不能把测试名中的prepared恢复分支算作已运行。
- 共用worker/记录改变，定向执行既有无模型回归：设置`CORTEXOS_HARNESS_CODEX_RUNTIME_REAL=1 CORTEXOS_CODEX_COMPATIBLE=1`，`npx.cmd playwright test --project=harness-codex-runtime --output=../../artifacts/harness-claude-codex-regression`：**1通过，16.9秒/总18.8秒**。真实旧Codex/Docker的专用策略/十项探针、v1/v2历史、停止、强杀恢复/手动清理均通过，未将策略套给Claude，无模型调用。
- 标准库Python只读核对上述两份独立QA库：integrity ok、FK空、schema11及迁移checksum与原库一致；普通messages/knowledge_nodes/knowledge_versions/execution_runs/context_snapshots/benchmark_runs均0不变，Claude检查记录0。首份库的旧真实Codex会话105d3b8e...及事件逐列字节相等；检查计划hash正确。证据artifacts/harness-claude-runtime-empty/database-verification.json。

实际截图已查看1366浅色与1920深色，其他两尺寸分别留图；属于WebView2 CSS视口，不是新系统缩放/读屏验证。没有打开最终验收窗口，当前本片仍待工具准备和真实CLI检查。未做付费API、模型推理、向量生成、项目代码任务或其他模型下载。

### 当前剩余与门槛

已完成代码、空态及旧流程验证。实际阻断是**工具准备许可待回复，Claude程序/镜像尚未准备**；尚不能把可能的依赖、权限或资源限制说成实际发现。确认后依次下载/验签/构建、显式PREPARED分支验证真实程序/停止恢复，再交付D3-3a验收。后续D3-3b内层安全条件、D3-3c受控Messages/预算/单题（模型另弹窗）、D3-3d Aider，再D4组合和D5报告/图谱，保持分片。

## 2026-11-01：D3-2c 受控模型通道与首个真实单题

用户授权继续，并要求每步交付列出当前阶段剩余步骤；已同步AGENTS。保存关闭旧窗口后继续，保留此前D1–D3未提交改动，未提交或推送。复核development.md、开发/实施规范、D3计划、实际Provider/TaskSlot/Docker/任务及评分调用链，沿用ponytail和现有UI组件。交付及文件清单见[harness-fit-d3-session.md](harness-fit-d3-session.md)，本片方案见[harness-fit-d3-session-plan.md](harness-fit-d3-session-plan.md)。

### 协议与隔离实现

新增harness_codex_session用途、三个精确业务接口、计划/事件hash、模型/检查/任务归属准入、停止/关闭/中断清理。复用原表、TaskSlot、Docker控制器、Ollama、选定函数包、范围校验与独立逐断言评分；schema仍11，无依赖/锁文件或旧迁移修改，不写聊天、长期记忆、知识库、原项目或普通BenchmarkRun。前端使用SelectMenu/FitStatus/AppDialog及折叠段，原始检查标历史快照，未就绪仍能读执行历史。

已实际查阅Codex官方Provider/config-reference和Ollama官方OpenAI兼容文档。Ollama兼容接口不能直接限制上下文，选择有界Responses文本/函数→原生chat适配，不假设Codex窗口参数能约束显存。容器保持无网络，无宿主挂载/Socket/端口映射；仅容器回环HTTP，经Docker stdin/stdout送Rust，固定已有Provider。不读取个人认证。每任务最多4次请求、编程阶段180秒、单次120秒；原生num_ctx4096/num_predict512/temp0/keep_alive0。缓冲响应后发SSE，不宣称逐Token首字延迟。

真实无模型探针确认CLI0.166.0的非交互exec自动拒绝提权，保留workspace-write内层保护；禁用联网搜索、多代理与目标自动续跑后获得实际函数工具清单。文本配置2只向模型提供exec_command/write_stdin，记录relay/IO/bridge/workflow源码hash和真实副本路径。普通文本不会转成工具调用，错误参数/名称和额外权限被拒绝。

### 实际失败与修正

- 首次`python scripts/harness/probe_codex_transport.py`遇默认Python2.7缺pathlib，未执行容器；改用现有`py -3.13`。Docker首次管道离线，按已有许可隐藏启动已安装Docker Desktop，无重装/下载/系统修改。Ollama此前未运行；隐藏启动已安装serve，进程16188，仅127.0.0.1、关闭云端，GET元数据时loadedModels为空，未加载/预热。
- `cargo check --offline`首次报告cancel可变/不可变借用冲突，移出初始状态读取后通过5.63秒。第一次原生受控检查7.5秒失败于测试create_workspace参数，修正name。
- 后一次用普通`cargo build --offline`误生成开发URL版本，原生启动等待tauri.localhost而停在127.0.0.1:1420。结束仅本轮QA进程21808，改回Tauri完整构建；该轮2.4分钟失败保留。后续均等待构建完成再启动。
- 受控v3实际CLI启动退出-25，尚无模型请求。同原限制无模型探针证明state_5.sqlite-wal触及1MiB文件限制，SIGXFSZ；仅Codex会话fsize调至原HOME4MiB额度内，CPU/内存/PID/目录总额与默认Python限制不变。复验日志为2,154,792字节、CLI可发到固定错误响应器，不接模型。
- 受控v4可走协议但文件未修改，独立测试4/7失败，正确保留失败。实际工具输出为bwrap不能建立/workspace/.git。预建四个空root/0555保护目录，仅Codex采用固定目录策略；不放开额外文件/子目录。移除不必要的常驻stdin读线程，避免失败退出时Python buffered-reader锁错误。
- 受控v5全部行为通过37.2秒/总39.4秒，但直接写错主题属性，所谓dark截图仍浅色，不据此声称深色验证。最终改用真实“外观”控件并断言data-color-scheme，再检三尺寸。

### 实际命令与结果

- `git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS；git status保留旧工作，Gittoken未读取/输出。
- `py -3.13 scripts/harness/probe_codex_transport.py`：固定错误响应器、无模型请求，实际协议和资源失败/修复观察位于artifacts/harness-codex-transport。早期默认Python和PowerShell路径glob/权限读取失败不记作成功。
- `npm.cmd run typecheck`通过；后续每次Tauri构建也含类型检查和Vite。`cargo check --offline`修正后通过。必要`cargo test codex_bridge_ --lib --offline -- --nocapture`：1通过、95过滤；最终0.01秒/编译29.46秒。没有跑无关全套单元。
- `npm.cmd run tauri -- build --debug --no-bundle`最终通过，Rust39.31秒、Vite0.778秒；index-1Zo_NmyA.js、原CSS index-CCwtNeOJ.css。已有大chunk警告保留，无升级依赖或安装包发布。
- `CORTEXOS_HARNESS_SESSION=1 CORTEXOS_HARNESS_SESSION_LEGACY=1`后执行`npx.cmd playwright test --project=harness-codex-session --grep 'controlled transport' --output=../../artifacts/harness-codex-session-controlled-final`：**1通过46.0秒，总48.3秒**。真实Codex/Docker/WebView2，受控Provider，真实模型调用0；命令修改+7断言、四请求截停、未知工具名拒绝、取消、强杀恢复/手动清理、旧真实失败原样、三尺寸浅深6图及无溢出均通过。此前v1–v4失败及v5主题问题单独保留，不覆盖为通过。

### 两次分别授权的真实本地单题

实际模型列表确认Ollama0.40.9，qwen3:4b-instruct digest0edcdef34593eac1aa2be9c7d06c432dcf81945adca5eca2f27662c18f168ba0，另有base标签但没有调用。每批都先按AGENTS弹窗说明模型、唯一adjust(n)任务和4请求/4096/512/120/180秒上限；两批各获用户明确答复。无付费云端、下载或自动重试。

1. `CORTEXOS_HARNESS_SESSION_REAL=qwen3:4b-instruct`，真实专项输出artifacts/harness-codex-session-real：Playwright预算/清理检查1通过33.7秒/总35.7秒，**题目没有通过**。会话7bf018fc-f2d3-46e6-9485-6710a244d9e1实际4请求，模型选错view_image和路径，再误用无效会话；最后达到512上限，failed/model_output_limit，文件未改、测试Not Run。输入8017、输出743 Token；保留原文/工具输出/文件与失败。副本清理，GET/api/ps为空。
2. 将副本准确路径与通用文本工具范围写清，不改题目/答案/断言；另行弹窗取得新一批确认。再执行同一真实专项，另设LEGACY=1从首批独立库backup、输出artifacts/harness-codex-session-real-text2：**1通过32.0秒，总34.1秒，真实题目独立7/7 Passed**。会话105d3b8e-7bd1-4247-b7aa-ceebea136071，3请求（读文件/写文件/完成），输入4974、输出357 Token，候选return max(0,n)。7个独立评分容器全部移除。上一批失败原样保留，不拿受控替身成绩冒充模型成绩，也不把单题当排行榜。

### 交付核对与验收窗口

`py -3.13 -X utf8 scripts/harness/verify_session_delivery.py`只读核对三份隔离QA库：integrity ok、FK空、schema11及迁移checksum一致，所有本用途plan/event hash正确。首批原始job/event字节在新库与受控库相等；messages/knowledge_nodes/knowledge_versions/execution_runs/context_snapshots/benchmark_runs均0且不变。本次归属容器、临时策略文件均不存在，GET/api/ps为空；没有额外推理或清理其他容器。

最终仅整理前端历史提示和未就绪时仍可读的入口后完成构建，不再次调用模型。`node artifacts/harness-codex-session-acceptance/launch.cjs`打开新版：进程18044、CDP60580、独立真实QA库。实际WebView2再次核对未就绪历史仍可读且禁止新任务、正确条件可启用、确认返回不新增job/请求、旧失败“未修改”和新通过可切换，当前新通过记录与JSON完全相等。confirmation/previous-failure/ready三图保留。`show.ps1 -AppProcessId 18044`确认visible=true、foregroundGranted=false；不强抢前台，可从任务栏切换。窗口只展示已有结果，不启动新容器/代码/模型。

`git diff --check`通过，仅既有CRLF归一提示。没有完整180秒等待、耗尽磁盘、实际Windows缩放/读屏或更多任务/模型验证。Docker CLI需在应用进程PATH可用；验收只给当前进程补已有程序目录，不修改系统PATH。当前停在D3-2c验收；Phase D剩余D3-3逐个接Claude Code/Aider、D4组合对照、D5报告/图谱；完整仓库/跨文件仍后置。本轮不自动进入下一片。

## 2026-10-31：D3-2b内层沙盒兼容与十项反向验证

用户在D3-2a后要求继续。复核development.md、开发/实施规范、D3计划、固定CLI/副本控制器和原探针；先形成[兼容审查方案](harness-fit-d3-sandbox-plan.md)及准确候选差异，再弹窗取得用户明确批准：最多三组无模型策略对照，反向检查通过后仅用于Codex专用检查容器。实际交付见[harness-fit-d3-sandbox.md](harness-fit-d3-sandbox.md)。没有本地模型、Embedding、Agent会话或项目代码调用；D3-2整体尚未完成。

### 诊断、来源和有界对照

`git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS，保留此前未提交改动。Docker管道当前未启动；按既有许可Start-Process已安装Docker Desktop并WindowStyle Hidden（PID4528），未安装/重启或更改系统设置。只读info/version确认Server29.9.2、SecurityOptions为内置seccomp及cgroupns，固定镜像仍71d63...。

可信namespace_probe在原受限容器运行，UID65534、CapEff/CapBnd均0、NoNewPrivs1、Seccomp2/filters2，user.max_user_namespaces31182；unshare(0)/unshare(NEWUSER)均EPERM。没有更改sysctl。实际查阅Docker seccomp官方文档和Moby规则，以及OpenAI官方Sandbox/Agent approvals说明；不执行其中宽权限建议。

PowerShell/urllib读取GitHub遇TLS EOF，普通curl报撤销服务离线、raw域名请求超时；均没有据此伪造来源。系统curl仅对公开文档使用--ssl-revoke-best-effort（保留HTTPS证书校验，无全局设置），通过GitHub官方内容API取得固定提交4b8214fc4971e06172938a1f1554ccff6647ee80、blob6e5e47...及原JSON。原文件SHA-256为6416b47770785a41ac59073cdc77d9fe98517df2799dc83ef207e622de3053f6，Apache-2.0许可一并保留。不声称该官方快照是安装引擎内置过滤器的精确导出。

`py -3 -X utf8 scripts/harness/prepare_codex_seccomp.py`仅生成候选及review.json，无执行或应用动作。用户确认后`py -3 -X utf8 scripts/harness/check_codex_seccomp.py`执行三种固定策略组，9.32秒、全部clean=true，comparison.json保留准确归属、配置与原始输出：官方基线复现阻断；仅受限unshare让0/NEWUSER调用均成功，但bwrap仍失败；完整六类调用候选让Codex越过原阻断，但旧探针随后因execvp linux失败而退出101，未记为通过。没有增加capability、setns、关闭seccomp、全权限或旧Landlock回退。

`codex --help`及`codex help sandbox`实际显示0.166.0参数格式，证明原探针多传linux，先前被更早的限制错误遮住。去掉该参数后，越界写入确实被拒绝，但原助手只捕获PermissionError，实际EROFS30导致探针失败。修正为只接受EPERM/EACCES/EROFS，并扩充十项反向观察；同一已批准第三组策略复验，不增加新策略种类。bubblewrap-safety.json实际exitCode0，十项均true、UID65534：允许目录写入、目录外及系统写入拒绝、内层访问外层已证明可达监听端口拒绝、mount/BPF拒绝、零capabilities、no-new-privileges、seccomp启用、无Docker Socket。目录拒绝errno30，mount/BPF errno1。旧失败证据保留，不回改D3-2a。

### 实际修改与兼容

- 新scripts/harness/{namespace_probe.py,prepare_codex_seccomp.py,check_codex_seccomp.py}，固定可信诊断/规则生成/三组对照；不安装或调用模型。
- 新src-tauri/catalogs/codex-seccomp/{profile-v1.json,README.md,LICENSE}与benchmark/python/codex_policy.rs，固定策略ID codex-bwrap-1、hash46a3ec985105a85a8d6bb5d21974a5dd5093f176411d766f8acf51bf5f2d9f13。只追加受限unshare/clone及mount/umount2/pivot_root/chroot，保留上游默认拒绝。Docker默认仍为默认选择，原Python评分和人工副本create_args调用链不改。
- 修改codex.rs/codex_probe.py、python.rs、harness/runtime.rs和records.rs：新protocol2/probeVersion；必须十项证据全部满足。固定临时策略文件按已持久化owner UUID命名、create_new写入，容器创建后清理；退出/恢复按名字、类型、重定向属性/hash核对，只清理本任务文件。实际SecurityOpt必须与嵌入规则语义完全相符，保存实际保护配置。新计划codex-runtime-check-2绑定策略/hash/来源，v1只读不重算。schema仍11，没有依赖/锁文件/迁移/新Tauri能力。
- CodexRuntime.tsx只加固定策略选择、明确的风险确认、当前记录策略和通过但任务未运行的状态，沿用现有控件/列表，无新CSS或全局布局调整。扩充原生专项兼容模式及v1真实历史；同步说明、实施计划/索引、设计/检查/数据库契约。

### 实际检查和构建

先代码逻辑审阅，仅运行必要安全边界，不跑无关全量单元。`npm.cmd run typecheck`通过；`cargo check --lib --offline`通过51.31秒，收尾后4.46秒通过。rustfmt只格式化本片Rust模块及子文件。

`cargo test codex_ --lib --offline -- --nocapture`：2通过、93未运行，首次0.01秒（编译34.93秒）；补充临时文件恢复清理后同两项复验0.03秒（编译13.14秒）。覆盖缺失/错误反向证据、命名空间分类、未知/禁用/篡改策略拒绝及临时文件创建/幂等清理；不把合成观察当真实Docker证据。

`npm.cmd run tauri -- build --debug --no-bundle`通过，Rust46.46秒、Vite14.27秒；assets index-CCwtNeOJ.css/index-CWcwHUr2.js。既有大块/npm版本提示保留，未升级依赖。`py -3 -X utf8 -`用ast.parse解析四个可信Python助手，4通过，没有宿主执行项目代码。

### 实际WebView2、恢复、历史及截图

测试进程仅补既有Docker CLI到PATH。CORTEXOS_HARNESS_CODEX_RUNTIME_REAL=1、CORTEXOS_CODEX_COMPATIBLE=1，执行`npx.cmd playwright test --project=harness-codex-runtime harness-codex-runtime.spec.ts --output=../../artifacts/harness-codex-policy-native`：1通过，28.4秒（场景25.1秒）。从原D3-2a QA库SQLite backup创建新库，未操作生产库。

实际覆盖：v1完成记录逐字段保持、新策略明确选择/确认、真实十项保护及实际SecurityOpt/hash、错误hash/未确认/工作区/用途/检查归属/未知策略拒绝、清理幂等、取消、重开不重跑；只暂停本专项容器并强杀自有QA进程，重开interrupted、阻止新检查、UI清理后closed，旧完成/停止记录不变。completed=5aa5dd48-3fa5-420c-89b5-1545e084ad4b，cancelled=2cfe36f6-454f-4496-9872-e17a93a0bfea，恢复closed=f9ab4a64-01a4-4b99-9cad-34b43911595d，均clean=true。

证据目录artifacts/harness-codex-policy-native/harness-codex-runtime-Linu-f820c-oped-cleanup-without-models-harness-codex-runtime。runtime.json和六张runtime-{light,dark}-{1366,1600,1920}.png保留；已实际查看浅1366、深1920，三尺寸无横向溢出。只读数据库检查messages/knowledge_nodes/execution_runs/benchmark_runs均0、schema11、FK检查空、integrity=ok；三个新owner的策略临时文件均不存在，另存database-verification.json。普通Python容器策略未修改，因此未重复旧模型评测或无关UI全套。

### 窗口、风险及下一步

`node --check artifacts/harness-codex-policy-acceptance/launch.cjs`通过；执行脚本打开最终PID10052、CDP50383，使用上述独立QA数据定位实际已通过记录，仅只读展示，没有新容器/代码/模型。`pwsh.exe -NoProfile -File .../show.ps1 -AppProcessId 10052`返回visible=true、foregroundGranted=false，可从任务栏进入；ready.png/launch.json保留。最终docker ps -a --filter name=cortexos-bench- --format {{.Names}}无输出。没有提交、推送、发布或更改用户模型/原项目。

专用策略增加内核接口暴露面，反向探针不是完整安全证明；原默认策略仍可选。内层沙盒兼容门槛现已通过，但受控模型通道、请求/时间/输出预算、真实任务JSONL/补丁/独立评分及Model×Harness成绩仍未完成。真实Windows缩放/读屏/IME未由截图代替。本轮停在D3-2b等待验收；下一片D3-2c接通道与唯一任务，实际加载/调用本地模型前仍单独弹窗确认。

## 2026-10-31：D3-2a Codex镜像、隔离检查与实际阻断

用户在D3-1后要求继续；复核development.md、开发/实施规范、D3计划、上片副本/控制器和Phase D原文，按[本片方案](harness-fit-d3-codex-plan.md)推进。用户另行弹窗批准下载官方Linux CLI到项目缓存并构建专用镜像；未申请模型调用授权，亦未发起推理、Embedding、Agent任务或云端调用。[交付、修改文件和剩余门槛](harness-fit-d3-codex-runtime.md)。D3-2整体尚未完成。

### 实现与来源

新scripts/harness/{prepare_codex.py,codex.Dockerfile}固定验证和提取四个官方普通文件，不运行安装脚本。新benchmark/python/{codex.rs,codex_probe.py}复用原bounded Docker命令、create_args、配置校验和归属清理；保持原无网络/非root/readonly root/CPU1/256MiB/PID64/cap-drop ALL/no-new-privileges/默认seccomp/无宿主挂载及Socket，只增加2MiB测试目录与4MiB空home tmpfs。全部程序/附属文件hash核对后，仅执行version、exec help及固定目录权限探针。

新benchmark/harness/runtime.rs和runtime/records.rs以harness_codex_runtime复用jobs/cases，创建前记录不可变计划及端点/名字/镜像/owner；实际观察、错误和清理只追加，状态/恢复单独处理。修改harness/mod.rs、python.rs、db.rs、lib.rs、build.rs、capabilities/default.json，仅四个精确业务命令和本用途退出/恢复；schema仍11，无新表/迁移/Runner/依赖/锁文件。新CodexRuntime.tsx接入ExecutionPreflight，复用现有SelectMenu/AppDialog/FitStatus及列表，无新CSS或页面重排。新原生专项与opt-in项目，同步实施/数据库/设计/检查规范和索引。没有提交、推送或改正式发布物，之前所有改动保留。

沿用ponytail及ui-ux-pro-max；实际UX查询`py -3 D:/CodexData/skills/ui-ux-pro-max/scripts/search.py 'error recovery progress cancellation' --domain ux`，应用原有恢复、进度和可访问反馈。实际读取OpenAI官方非交互、CLI参数、配置、permissions及sandboxing页面，链接在本片文档；系统设置建议不自动构成操作授权。

### 工具准备、首次失败与实际环境

`git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS。只读GET /api/tags和/api/version确认现有两Qwen4B及Ollama0.40.9，没有加载模型。首次`npm.cmd view @openai/codex version`因受限全局npm-cache mkdir EPERM失败；普通Invoke-RestMethod读registry因TLS认证失败。获准联网的registry只读请求成功，固定正式版0.166.0与平台包元数据，未安装npm依赖或更改cache。

官方codex-0.166.0-linux-x64.tgz下载到artifacts/harness-codex-runtime，172325449字节；SHA-512与官方版本元数据一致，完整值和四文件SHA-256在runtime.json及交付文档。未独立验证npm签名或构建来源证明，不把hash核对当验签。

`py -3 -X utf8 scripts/harness/prepare_codex.py --build`首次因Docker Linux管道未启动失败；按既有授权Start-Process已安装Docker Desktop并WindowStyle Hidden（PID27760），未重装/重启。实际Server29.9.2、Desktop4.97.0、Linux amd64、WSL内核6.18.54.1。下一次FROM原始imageID被BuildKit解释为名为sha256的远程仓库，registry HEAD TLS超时而失败，没有下载新基础镜像。修为构建前核对本地python:3.13-slim标签再使用该标签；实际BuildKit绑定原digest2d447...，--network=none --pull=false通过。空home调整后最终镜像构建通过：sha256:71d63ab98eec120322e718fc0faf631cf1008bb3e938ee0554d830cecfbf480b。Dockerfile仅COPY/ENV/LABEL/USER，无RUN安装。

初始独立docker run的沙盒help探针因CODEX_HOME未创建失败；可信临时助手创建home后version通过，但/tmp下助手别名警告且bwrap命名空间失败，JSON保留artifacts/harness-codex-runtime/probe.json。正式实现采用/home/cortexos空tmpfs并核对全部四文件，仍实际返回`No permissions to create a new namespace`，固定测试命令stdout为空，目录内允许/目录外拒绝写入均未验证。没有放宽seccomp、增加特权、全权限或旧Landlock回退；未进一步精确归因于某一内核/LSM/seccomp层。

### 类型检查、构建及必要检查

- `npm.cmd run typecheck`通过，2.22秒；桌面构建内再次通过。
- `cargo check --lib --offline`首次发现Connection/MutexGuard借用不匹配，改为局部持有/释放连接后通过，6.51秒。
- `rustfmt --edition 2024 src/benchmark/harness/runtime.rs src/benchmark/python/codex.rs`仅本片模块及子文件，无全库格式化。
- `cargo test codex_runtime_checks_reject_missing_boundary_proof_and_namespace_failure --lib --offline -- --nocapture`：1通过、93未运行、0.00秒，编译44.62秒。只检验新判定边界及错误协议，合成输入不当Docker证据；未跑无关全量单元。
- `npm.cmd run tauri -- build --debug --no-bundle`通过，Rust54.05秒（含等待前述Cargo锁），Vite11.81秒；index-CCwtNeOJ.css/index-Dv0Y0o5I.js。原大块/npm版本提示保留，未升级依赖。
- 最终`py -3 -X utf8 -`以ast.parse解析可信prepare_codex.py/codex_probe.py，通过；未在宿主执行项目代码。准备脚本路径守卫收尾后未重复构建镜像，已测运行内容和CLI未变。

### 原生WebView2、Docker与旧功能

测试进程PATH补入既有Docker resources/bin；显式CORTEXOS_HARNESS_CODEX_RUNTIME_REAL=1。用SQLite backup复制此前独立D3-0 QA库到新目录，不操作生产库。

`npx.cmd playwright test --project=harness-codex-runtime harness-codex-runtime.spec.ts --output=../../artifacts/harness-codex-runtime-native`：1通过，15.4秒（场景12.5秒）。真实程序/四文件指纹、版本0.166.0、自动化参数已核对；内层命名空间失败准确保留sandboxReady=false、canRun=false、taskExecution=Not Run、modelCalls=0。未确认/错误hash/跨工作区操作前拒绝，跨用途/其他任务检查读取拒绝；清理幂等，旧快照不改；停止后cancelled/clean，重开不自动探测；三尺寸×浅深六张实际截图及无横向溢出通过。

为覆盖新增恢复SQL，追加暂停仅自有QA探针容器→强杀自有QA进程→重开→UI手动清理。复验同一专项到`../../artifacts/harness-codex-runtime-recovery`：1通过，19.0秒（场景17.0秒），不累计为新用例数量。completed=2595f192-fabe-4265-b389-096751eb1024，cancelled=fdc02240-6c23-4185-bdb0-eec79395fa45；f7d39a38-20a4-4ec8-9d7f-5a7cac1d88da重开为interrupted，阻止新检查，UI清理后closed/clean=true，无probe完成记录；原完成/停止记录逐项不变。

因触及共用退出清理，仅复验原D3-1相关场景：CORTEXOS_HARNESS_WORKSPACE_REAL=1，`npx.cmd playwright test --project=harness-workspace-real harness-workspace.spec.ts --grep 'rejects foreign files' --output=../../artifacts/harness-codex-workspace-regression`：1通过，25.2秒（场景23.1秒）。原额外文件拒绝、人工循环代码停止、正常关闭清理及强杀恢复/手动清理保持。没有重复其他无关套件或模型评测。

最终证据为artifacts/harness-codex-runtime-recovery/harness-codex-runtime-Linu-a2957-oped-cleanup-without-models-harness-codex-runtime/runtime.json及runtime-{light,dark}-{1366,1600,1920}.png，实际查看浅1366/深1920。只读QA库：messages/knowledge_nodes/execution_runs/benchmark_runs均0，schema_migrations max11、foreign_key_check空、integrity_check=ok；本用途completed/cancelled/closed各1。另存database-verification.json。

### 验收窗口与未完成项

`node --check artifacts/harness-codex-runtime-acceptance/launch.cjs`通过；执行脚本打开最终PID24084、CDP58683，使用本轮独立QA数据定位Codex隔离检查并显示真实未就绪记录。只读展示，没有新容器、代码或模型调用。ready.png和launch.json留存；`pwsh.exe -NoProfile -File artifacts/harness-codex-runtime-acceptance/show.ps1 -AppProcessId 24084`返回visible=true、foregroundGranted=false，用户可从任务栏进入，不声称Windows已允许抢前台。最终只读docker ps -a --filter name=cortexos-bench- --format {{.Names}}无输出，没有全局删除操作。

内层沙盒兼容、受控模型通道/预算、真实任务JSONL、自动补丁/独立评分和Model×Harness成绩均未完成。下一片先审查兼容方案，安全配置如需变化先形成可审阅最小差异和反向检查、另行确认；模型调用仍须独立弹窗。真实系统150%/IME/读屏未由本轮截图替代。本轮停在D3-2a环境门槛验收，不进入下一Harness或D4/D5。

## 2026-10-30：D3-1任务范围副本、人工交接与独立测试

用户授权继续D3-1，并通过交互弹窗确认已保存关闭旧验收窗口。复核AGENTS、development.md、开发/实施规范、D3分片及已有检查、不可变任务、Python控制器和退出处理，先写[本片方案](harness-fit-d3-workspace-plan.md)，交付说明见[harness-fit-d3-workspace.md](harness-fit-d3-workspace.md)。沿用ponytail与ui-ux-pro-max，查询`py -3 D:/CodexData/skills/ui-ux-pro-max/scripts/search.py 'code review destructive confirmation' --domain ux`用于确认和结果反馈，不引入视觉体系。实际打开Docker官方tmpfs/exec文档核对固定文件传输和容器内权限，不把文档配置当实测。

### 实际修改

- 新`apps/desktop/src-tauri/src/benchmark/harness/workspace.rs`及`workspace/{records,package}.rs`：按工作区、已保存检查、发布任务/hash/审核及来源准备任务范围包；复用TaskSlot和已有表，人工提交/收集/独立评分、停止清理与历史。包仅含题目、签名/类型、路径/原范围及函数原文；参考/错误实现、examples/预期答案留在评分侧。按原范围生成完整文件提案并记录前后hash，原范围外字节保留，不写原项目。
- 新`benchmark/python/workspace.rs`及`workspace_io.py`，原python.rs仅接入子模块。复用原bounded命令、create_args、configuration_valid、环境、score_owned及按归属清理；仅增加2MiB /workspace tmpfs，固定600秒无Agent常驻。可信初始化器只写文字并设置权限，实际项目代码仅在后续独立评分容器执行。题目/目录root只读、目标函数文件原地可写；盘点拒绝链接、硬链接、额外/缺失文件、hash/权限不符，无通用执行入口或宿主挂载。
- 修改harness/mod.rs、src/lib.rs、build.rs、capabilities/default.json接入4个精确命令；db.rs仅恢复本用途opening/ready/grading为interrupted。正常退出等待活动任务并尽力清理本应用记录的副本；commands.rs的close_cleanup原子标记防止重复关闭抢在清理前退出，失败保留待清理。没有新迁移，schema_migrations最大版本仍11。
- 新`src/benchmark/harness/WorkspaceHandoff.tsx`，ExecutionPreflight.tsx接入，harness.css局部样式：确认、准备/停止、历史、人工代码、修改对照、实际评分、关闭清理与其他检查下的未关闭副本入口。未提交编辑保留进程内草稿，提交保存与进入grading在同一事务；已保存代码/错误/取消记录保留。完成后代码按需展开，测试结果优先；弹窗固定关闭，沿用现有Tokens/控件，无导航/知识/图谱重构。
- 扩充原validation/tests.rs信任边界与package必要检查；新`tests/harness-workspace.spec.ts`和opt-in配置。tests/native.ts仅增加通过实际窗口关闭按钮退出的检查助手，原关闭助手保持。同步方案/交付、实施/数据库/设计/检查规范及索引。复用generation_jobs/generated_cases，以harness_workspace隔离；无新Runner、依赖或锁文件改动，没有提交/推送/更新正式发布物。

### 代码检查及实际构建

`git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS。先逻辑审阅，仅执行新增信任/事务/范围检查，不跑无关全套单元。

- `npm.cmd run typecheck`三次随实现变更通过，最终构建也包含类型检查。
- `cargo check --lib --offline`首轮发现records子模块两方法可见性不足（伴随类型推断错误），修为harness内部可见后通过，5.89秒；移除未使用collect入口并完成退出标记后复查通过，4.24秒，无警告。没有改行为以绕过编译检查。
- `cargo test workspace_bundle_and_file_inventory_preserve_scope_without_answer_fields --lib --offline -- --nocapture`：1通过、92未运行，执行0.00秒；添加CRLF/中文范围保护后同一专项复验通过（编译1.60秒），不累计为新用例。覆盖答案字段排除、额外文件/错误hash/权限拒绝及原范围外字节保留。
- `cargo test programming_controls_bind_evidence_reject_stale_approval_and_preserve_versions --lib --offline -- --nocapture`：1通过、92未运行，0.41秒（编译37.51秒）。扩充副本用途/工作区、检查hash、单个活动副本、未确认/错hash/无修改/重复提交、未清理终态拒绝、强制提交INSERT失败的事务回滚及重开interrupted；合成观察明确testOnly，不冒充Docker记录。
- `py -3 -X utf8 -`通过ast.parse检查可信workspace_io.py语法，仅解析本项目助手，不运行提交代码。
- `npm.cmd run tauri -- build --debug --no-bundle`三次通过：首次Rust1分47秒；增加阶段错误记录后29.18秒；最终UI收尾34.10秒。最终Vite718毫秒，assets index-CCwtNeOJ.css/index-BHdcQiNj.js；既有大块提示及npm更新提示保留，未升级依赖。

### 真实原生/Docker首轮失败与复验

显式CORTEXOS_HARNESS_WORKSPACE_REAL=1，测试进程PATH补入已安装Docker resources/bin；SQLite backup从D3-0实际QA库创建新的独立测试库，没有打开生产库。镜像保持原已下载的Python3.13固定Image ID，无模型、Codex或其他CLI任务、宿主仓库脚本和新镜像下载。

首次`npx.cmd playwright test --project=harness-workspace-real --project=desktop harness-workspace.spec.ts harness-tasks.spec.ts --output=../../artifacts/harness-workspace-native`：**2失败、1通过，3.1分钟**。正常交接场景第一次准备在5秒有界操作中wall_timeout，记录失败并清理，未进入测试；当时未细分阶段，无法据此确定是创建/启动/初始化哪一步，后续补上workspace_create/start/init/copy分段原因，保持原时限，无自动重试或扩大评分预算。第二场景实际越界拒绝、停止及清理已执行，随后Windows PowerShell关闭助手返回4294901760，退出验证未通过；单独只读退出0探针成功，未确定更细的系统助手原因。改用应用自己的关闭按钮并等待进程退出，直接验证用户路径。旧D2a发布专项通过，19.9秒。初轮证据保留，不记为通过。

复验仅受影响的两场景：`npx.cmd playwright test --project=harness-workspace-real harness-workspace.spec.ts --output=../../artifacts/harness-workspace-fixed` **2通过，1.0分钟**（31.0秒、28.6秒），旧D2a不重复。配置和评分时限保持，首轮准备超时在本次未复现。

实际观察包括：

- Linux临时副本非root UID65534、network none、readonly root、CPU1/256MiB/PID64、无宿主bind/Socket，只有task.json和project/src/value.py；原生真实权限尝试确认题目及新文件写入被拒绝，允许包无答案字段。
- 正确人工代码实际独立测试**7/7通过**，错误实现**4/7通过、最终Failed**；两次操作均正常结束且清理，历史成绩保留。副本先清理再创建评分容器，每个断言独立；没有Model×Harness成绩。
- 刻意由受控root测试探针加入额外文件后，收集真实拒绝workspace_unexpected_file，未生成候选/评分；循环修改停止后Cancelled，提交代码保留、待清理列表为空。
- 通过真实“关闭窗口”按钮退出时闲置副本清理且状态closed；重新打开不恢复副本。只强杀自有测试进程后再打开，ready恢复为interrupted，禁止新建直到手动清理；清理只处理记录的owner/name/image/endpoint，四次历史及先前错误/取消结果不变。
- 已发布v2读取与操作前完全相同，四类业务表messages/knowledge_nodes/execution_runs/benchmark_runs均0；schema_migrations max=11、foreign_key_check为0。未改聊天、知识、标准评测或图谱数据。

正常/错误交接证据：`artifacts/harness-workspace-fixed/harness-workspace-task-cop-acf97-n-fresh-isolated-containers-harness-workspace-real/workspace.json`，实际通过job12af5da5-e4a7-4574-b330-972bc480670b、错误job2329b213-6621-4f68-bfed-6209311b40a7。恢复证据在同根`harness-workspace-task-cop-1e758--and-forced-restart-history-harness-workspace-real/recovery.json`。实际三种视口浅深6张原生截图及无横向溢出检查通过；已查看浅1366与深1920。

### 最终界面、验收窗口与停止点

截图后仅将完成的代码框改为按需展开，并用普通话显示操作超限/失败；不重复Docker评分和已通过的停止恢复。最终构建后的原生只读检查验证两条历史对应不同代码、折叠/展开、修改弹窗关闭和三尺寸浅深6图；这与前述真实执行专项分别记录。`node --check artifacts/harness-workspace-acceptance/launch.cjs`通过，运行该脚本复用正常交接QA库，未新建容器、探针或模型请求。最终6图workspace-final-{light,dark}-{1366,1600,1920}.png和ui-verification.json、ready.png在artifacts/harness-workspace-acceptance，已实际查看浅1366与深1920。

末次docker ps按cortexos-bench-前缀观察无容器，专项也已按每个任务的实际端点/owner label确认无残留。验收进程PID2772、CDP52481，定位“任务副本与独立测试”及7/7通过记录；`pwsh.exe -NoProfile -File artifacts/harness-workspace-acceptance/show.ps1 -AppProcessId 2772`实际visible=true、foregroundGranted=false，用户可从任务栏切入。最终git diff --check通过，仅已有CRLF提示。

保留限制：当前只有已审核单文件函数的范围副本和人工交接，不是完整仓库或真实编程工具；600秒仅核对固定配置，未等待/测量完整600秒。正常退出清理为有界尽力，Docker不可用/强杀仍需保留归属并后续清理。实际系统缩放/读屏/输入法及跨硬件未在本片复验；固定用例不证明全部语义或全面副作用安全。D3-2真实运行镜像、模型通道/授权、预算、CLI事件另行确认。本片完成后停止验收，不进入D3-2/D4/D5。

用户在收尾补充反馈此前意外退出为显存不足，要求以后本地模型操作先弹窗。已同步AGENTS.md和development-standards.md：开发/验证实际推理、向量生成、评测或可能加载本地模型前说明模型/范围并等待本次明确确认，继续开发及历史许可不替代；只读健康/列表/元数据可继续。本片实际没有本地模型调用。

## 2026-10-30：D3-0 Codex检测与执行前检查

用户在D2b-1后要求“继续”，据此推进下一个增量，不一次实现D3全阶段。复核development.md、开发规范、实施计划、用户Phase D原附件、D2计划/实际交付、表与控制器。按[harness-fit-d3-plan.md](harness-fit-d3-plan.md)交付[执行前检查](harness-fit-d3-preflight.md)，完成后停止验收；D3-1独立项目副本/修改结果交接、D3-2真实CLI任务、D3-3下一工具和D4/D5继续后置。更广D2任务类型仍未完成，没有借本轮把它们标为完成。

沿用ponytail与ui-ux-pro-max，UX查询`py -3 D:/CodexData/skills/ui-ux-pro-max/scripts/search.py 'prerequisite checklist blocked status' --domain ux`返回真实提交反馈和单一上下文状态播报建议，用于检查/读取与结果列表，不新建设计体系。OpenAI Docs已实际查阅[非交互模式](https://learn.chatgpt.com/docs/non-interactive-mode)、[CLI命令](https://learn.chatgpt.com/docs/developer-commands?surface=cli)和安全页；资料支持仅用于分片与安全设计，不冒充本机JSONL/模型任务调用。没有安装CLI、新执行镜像或下载模型，也未探测个人登录/凭据。

### 实际文件与数据

- 新`apps/desktop/src-tauri/src/benchmark/harness/preflight.rs`、`preflight/codex.rs`。核对已发布Python任务的工作区/分析/version/hash/审核/固定来源，显式执行原生Codex固定--version探针，复用python::harness_environment读取本地Docker/发布镜像。程序输入由后端固定，无任意路径/参数入口；宿主版本与未来Agent运行环境分开。
- 修改harness/mod.rs接入；programming/validation.rs仅放宽programming_taskset在crate内可见性用于复用读取；发布/评分逻辑保持。src/lib.rs、build.rs及capabilities/default.json新增读取/执行前检查两个精确命令，无通用终端权限。
- 新`src/benchmark/harness/ExecutionPreflight.tsx`，修改TaskWorkbench.tsx和harness.css嵌入默认折叠区。任务/版本选择、实际检查、历史、状态列表、错误重读、原始依据按需展开；切换任务先清除旧结果，历史同步任务选择器。主列表把宿主/CLI/Agent等转为本机/程序/编程工具，原记录和原始详情保持原样，不更改旧hash或推荐。
- 检查复用generation_jobs，以purpose=harness_execution_preflight保存完成的plan/hash；canRun=false、taskExecution=Not Run。历史最多30条、已发布版本最多100项，工作区/分析/用途查询隔离。schema仍11，没有迁移、依赖或锁文件改动，不创建BenchmarkRun、执行事件、聊天、知识或成绩。环境/认证/模型/预算未实际验证的项目分别保留未就绪或未检查。
- 新`tests/harness-preflight.spec.ts`和Playwright opt-in项目；在已有validation/tests.rs追加必要的快照信任边界断言。同步D3方案/交付、实施/数据库/设计/检查文档与索引。此前D1/D2未提交变更完整保留，没有提交、推送或发布。

### 实际命令与失败恢复

`git rev-parse --show-toplevel`确认独立D:/vibecoding/CortexOS；读取AGENTS和项目相关文件，不读取Gittoken.txt或个人认证文件。两次`cargo check --lib --offline`（含一次沙箱外重试）均因本机Cargo registry缓存缺失找不到既有chrono而失败；只读检查确认cache中尚无index/src。没有据此改版本。`cargo check --lib --locked`按Cargo.lock恢复原有包，实际通过，1分23秒；没有新增/升级依赖，锁文件不变。该联网仅恢复项目既有构建依赖。

- `npm.cmd run typecheck`通过（两次实施检查，随后构建也含类型检查）。
- `cargo test programming_controls_bind_evidence_reject_stale_approval_and_preserve_versions --lib --offline -- --nocapture`：1通过、91未运行，0.20秒（编译40.96秒）。复用原已发布fixture并新增跨工作区/错误hash/缺失版本拒绝、旧用途读取拒绝、计划不可变、环境变化后旧检查不变、即使合成CLI/Docker可用仍禁止运行。合成观察明确testOnly，不当真实环境证据。
- `cargo test preflight_version_protocol_rejects_extra_output_and_relative_search --lib --offline -- --nocapture`：1通过、91未运行，0.00秒（编译2.12秒），覆盖合法正式/预发行版本、额外输出/非版本字符串、相对路径/UNC拒绝。不跑无关全套单元。
- `npm.cmd run tauri -- build --debug --no-bundle`首次通过，Rust42.39秒。真实检查发现本机Codex文件368970544字节，超过最初256MiB校验上限，未执行版本探针而正确保留cli_too_large；改为512MiB并增加实际累计读取量检查后重建通过，Rust34.00秒。探针仍固定参数、空配置环境、5秒及每流4096字节输出限制，CLI失败不保存任意原始输出。
- 最后只改显示词汇时replaceAll被项目现有TypeScript库目标拒绝，构建在前端检查阶段失败；改用全局replace正则，不改变编译目标或新增polyfill。最终完整构建通过，Rust27.55秒、Vite739毫秒；最终assets为index-CPRT-CXa.css/index-7FEmJPQa.js。原有>500kB块提示及npm更新提示保留，未为此升级依赖。

### 真实原生环境与界面

显式CORTEXOS_HARNESS_PREFLIGHT=1，当前检查进程PATH补入已安装Docker resources/bin。测试通过Python SQLite backup把D2b-1最终实际QA数据库复制到新的独立目录，未读取生产库；保留原真实任务/审核/对照，不伪造新任务或重新跑评分。

首轮`npx.cmd playwright test --project=harness-preflight --project=desktop harness-preflight.spec.ts harness-tasks.spec.ts --output=../../artifacts/harness-preflight-native`：旧D2a任务专项通过（8.9秒），新增检查在断言CLI应识别时失败（4.1秒），合计1失败/1通过。实际已记录cli_too_large，Docker同时记录docker_environment_error。只读docker version进一步确认本地Linux管道不存在，Docker Desktop/backend均未运行；没有把环境失败当成功或抹掉该检查记录。

按既有安装/使用授权，仅`Start-Process .../Docker Desktop.exe -WindowStyle Hidden`启动已安装程序，PID6368；没有重装、下载镜像或重启系统。实际docker version确认Linux Engine29.9.2、Desktop4.97.0、amd64。调整程序文件上限后，只复验受影响的新增场景：`npx.cmd playwright test --project=harness-preflight harness-preflight.spec.ts --output=../../artifacts/harness-preflight-fixed` **1通过，35.9秒**（场景33.9秒），未无故重复已通过的旧任务专项。

本机实际Codex版本为0.166.0-alpha.5.1，SHA2563097c909fb6d5cd63510d44aceb1d42df9f6e85722a3e447e477def1ecf60e16，来自固定--version探针，临时目录已清理。Docker读取的发布时Image ID仍sha256:2d44738ebf4ce5f63b9d9ea871e9c7cfdc0426f7b0c97cac915fd88ded1d86bb；只读元数据，不启动评分容器。

真实场景覆盖空工作区/无任务、选择两版实际发布任务、错误hash/跨工作区在探针前拒绝、两条独立检查、切版本隐藏旧结果、选历史同步版本、旧检查/发布v1不变、重开仍两条且不自动探测、三尺寸浅深截图及无横向溢出。独立库最终messages、knowledge_nodes、execution_runs和benchmark_runs均0，检查结果保持taskExecution=Not Run。证据为`artifacts/harness-preflight-fixed/harness-preflight-Codex-pr-ea7ea-ions-without-starting-tasks-harness-preflight/preflight.json`及6张preflight-{light,dark}-{1366,1600,1920}.png；失败证据保留在harness-preflight-native。

### 最终窗口、截图和保留限制

词汇显示调整后没有重复执行全部探针或单元；最终构建使用已有两条实际记录做原生读取及六尺寸/主题截图检查。`node --check artifacts/harness-preflight-acceptance/launch.cjs`通过；执行脚本启动新版PID22824、CDP61826，定位“4.测试任务 → Codex执行前检查”，两条历史可查看；没有新版本探针、模型或代码执行。最终6张preflight-final-{light,dark}-{1366,1600,1920}.png及ready.png在artifacts/harness-preflight-acceptance，已实际查看浅1366、深1920。显示翻译不改后台快照，最后切回浅色和原生窗口尺寸。

`pwsh.exe -NoProfile -File artifacts/harness-preflight-acceptance/show.ps1 -AppProcessId 22824`实际visible=true、foregroundGranted=false；用户可从任务栏进入，不声称Windows已允许抢前台。最终git diff --check通过，只有已有CRLF提示。

当前仅确认本机原生程序版本及Docker元数据；Windows PATH外、脚本启动形式、其他OS/认证/模型兼容和真实系统缩放/读屏未完成本轮验收。Codex预发行版本被识别不等于真实任务或Linux镜像兼容通过。隔离编程工具镜像、独立项目副本、受控模型通道、预算、修改结果与独立评分交接尚未建立，因此不能运行任务。无个人账号复用、付费云端、仓库脚本或D4/D5调用。本轮停在D3-0等待验收。

## 2026-10-30：D2b-1真实编程对照、停止恢复与人工发布

用户在D2b-0验收后要求继续。复核development.md、开发/实施规范、D2b方案及已有草案、Python控制器与表结构，按[harness-fit-d2b-validation-plan.md](harness-fit-d2b-validation-plan.md)实施本切片；交付说明见[harness-fit-d2b-validation.md](harness-fit-d2b-validation.md)。沿用ponytail和ui-ux-pro-max；本地UX查询`py -3 D:/CodexData/skills/ui-ux-pro-max/scripts/search.py 'asynchronous progress cancel form' --domain ux`用于真实进度、停止反馈和表单可达性。没有更换设计体系、下载模型、调用模型或进入D3。

### 实际修改

- 新`apps/desktop/src-tauri/src/benchmark/harness/programming/validation.rs`及`validation/tests.rs`；`programming.rs`接入子模块。验证用途harness_programming_validation复用generation_jobs/generated_cases，绑定固定草案修订/hash、三份代码hash、固定镜像、策略与评分版本；保存真实环境、逐容器归属、三组逐用例结果。发布用途harness_programming_tasks复用datasets/reviews，人工批准当前成功修订后追加不可变版本，旧版本不覆盖，taskExecution仍Not Run。
- `benchmark/python.rs`增加本机Python3.13镜像列表、环境记录、带所有者的既有评分入口和受限遗留容器清理；仅本地Docker端点，固定Image ID，创建前记录归属并写实际容器label。清理核对端点、名字、label及镜像，不做全局prune。资源/网络/挂载策略沿用原控制器，不执行仓库脚本或宿主Python。
- `python_tests.rs`增加CLI各阶段取消专项；`evaluation.rs`引擎从2.1升2.2，标识准备/握手前取消分类修复，答案规则不改，旧评分不复算。
- `db.rs`仅为本用途validating增加恢复到interrupted；schema仍11，没有新迁移、历史迁移或依赖改动。`build.rs`、`src/lib.rs`和`capabilities/default.json`只增加7个精确业务命令，无通用终端权限。
- 新`src/benchmark/harness/ProgrammingValidation.tsx`，修改`ProgrammingDrafts.tsx`、`programmingTypes.ts`和`harness.css`：独立组件、真实进度/停止/清理、三组结果表优先、逐用例与版本按需展开、审核批准和只读发布历史。轮询串行并防过期响应；修订变化后清除批准。复用现有Tokens、AppDialog、SelectMenu、FitStatus；预期失败显示“未通过（符合预期）”，不改真实Failed结果。
- 新`tests/harness-programming-validation.spec.ts`与opt-in Playwright项目。同步本轮方案/交付、实施计划、数据库契约、设计/检查规范、草案历史指引及文档索引。保留此前D1/D2变更，没有提交、推送或修改正式发布物。

### 实际命令与检查

- `git rev-parse --show-toplevel`确认独立D:/vibecoding/CortexOS。代码审阅复核用途/工作区、过期修订、独立数据和不可变记录；只执行必要的信任边界与新增交互专项，没有无关全套单元。
- `npm.cmd run typecheck`通过；`cargo check --lib --offline`首轮26.20秒，去掉unused import后复查4.10秒通过。
- `cargo test programming_controls_bind_evidence_reject_stale_approval_and_preserve_versions --lib --offline -- --nocapture`：1通过、89未运行，执行0.17秒。受控合成观察只用于边界测试，不冒充Docker证据；覆盖未验证/伪造hash/旧修订/未批准拒绝、工作区和用途隔离、对照规则、不可变两版、恢复及聊天/知识/执行零写入。
- 新取消专项`cargo test scoring_python_cancellation_is_preserved_before_handshake_in_every_cli_phase --lib --offline -- --nocapture`：1通过、90未运行，0.15秒。随后仅扩展相关共享控制器`cargo test scoring_python_ --lib --offline -- --nocapture`：5通过、3显式忽略、83未运行，0.40秒。忽略的旧真实Docker测试没有在此命令执行；后续独立原生Docker场景另列。输出中synthetic controller error为预定catch_unwind异常测试，最终通过。
- `npm.cmd run tauri -- build --debug --no-bundle`三次通过（随控制器修复及界面调整重建），最终Rust28.07秒，前端`index-yn_TJqDt.css`/`index-0kqG8DIN.js`。构建内TypeScript/Vite通过；原有>500kB块提示保留，没有因npm提示升级依赖。

### 真实原生检查、初轮失败与修复

使用真实已安装Docker Linux/Python3.13.18固定镜像，独立测试数据库和WebView2；不打开生产数据库、不请求模型。命令前设置CORTEXOS_HARNESS_DOCKER_REAL=1并把已安装Docker的resources/bin加入本次进程PATH。

首轮`npx.cmd playwright test --project=harness-docker-real --output=../../artifacts/harness-validation-real`两项未通过：成功流程在镜像/开始按钮尚未就绪时触发5秒断言，尚未创建验证；改为明确等待实际镜像选择器及启用按钮，未伪造环境，未确定更细的延迟根因。停止场景则真实发现容器启动后、Python握手前取消被误记Execution Error/python_did_not_start；现保留各CLI阶段termination reason并优先分类Cancelled，清理失败仍单独报错。失败证据保留，不记为通过。

修复后同一真实项目命令输出到`artifacts/harness-validation-fixed`：**2项通过，1.6分钟**（54.7秒与36.5秒）。成功场景实际执行每修订3×7断言，两次验证、人工批准发布v1/v2，旧v1全文不变；旧验证拒绝批准新修订、跨工作区拒绝、普通聊天为空、来源嵌套详情与固定关闭按钮通过。另一场景实际错误参考拒绝发布、循环实现取消清理、运行中强杀仅测试进程、重开保留interrupted、UI只清理本次拥有容器，3次验证历史保留。`recovery.json`保存真实恢复观察。

人工查看浅色1366与深色1920截图后，把7个发布条件收进展开区，三组表提前；原代码/错误实现按预期失败单独标注。此UI变更后执行最后必要回归：

`npx.cmd playwright test --project=harness-docker-real --project=desktop harness-programming-validation.spec.ts harness-programming.spec.ts harness-tasks.spec.ts --grep 'real controls, human approval|programming drafts keep|project tasks bind' --output=../../artifacts/harness-validation-final`

最终**3项通过**（`.last-run.json`为passed、failedTests为空，三个场景均留下成功元信息）：真实两版对照/发布、原草案修订/保护、旧D2a只读审核发布。此前已通过的强杀场景未因纯UI调整重复运行。独立场景共4种，不把重复复验累加成新用例。三种视口1366×768、1600×900、1920×1080浅/深6张实际原生截图及横向溢出检查通过；最终已查看浅1366、深1920原图。

最终证据入口：`artifacts/harness-validation-final/harness-programming-valida-f8e50-ound-to-each-draft-revision-harness-docker-real/acceptance.json`和`validation-{light,dark}-{1366,1600,1920}.png`。实际workspace=6094f530-c6ad-4bbd-9867-692cb6d0e713，draft=03d419ce-ce1f-462b-b237-8fb8d0c5749a，第二次验证=75fad6de-06b1-443c-a0ec-0d2cbc8352ba；发布2版，无模型调用。恢复场景证据在`artifacts/harness-validation-fixed/harness-programming-valida-bd335-clean-only-owned-containers-harness-docker-real/recovery.json`。

### 剩余限制与验收点

本切片只覆盖已有纯函数的变更，未完成新空函数、跨文件补丁、完整仓库构建或D3第三方Harness执行。用户仍须审核用例语义/覆盖，受控对照成功不代表真实模型能力。强杀整个应用会结束宿主超时控制器，遗留容器保留资源/网络/挂载限制，需回到记录手动清理；中断结果不得发布。Windows实际系统缩放、实体输入法和读屏本轮未复验，视口截图不替代这些人工检查。Core60历史题库与D2b-0旧20项记录未被此切片读取或重写，没有模型、云端或额外下载。

验收窗口：只读`docker ps -a --filter name=cortexos-bench- --format {{.Names}}`观察无本次前缀容器。`node --check artifacts/harness-validation-acceptance/launch.cjs`通过，执行该脚本打开最终成功专项的独立QA数据，PID4048、CDP57210，界面定位“4.测试任务/编程任务草案/验证与发布”，修订2及已发布2版可查看。没有新模型请求或重新执行代码。`pwsh.exe -NoProfile -File artifacts/harness-validation-acceptance/show.ps1 -AppProcessId 4048`返回visible=true、foregroundGranted=false；窗口已显示但Windows未允许抢占前台，用户可从任务栏进入。ready.png和launch.json保留实际状态。最终`git diff --check`通过，仅已有CRLF提示。本轮停在D2b-1等待确认，不进入D3。

## 2026-10-30：修复Docker启动、完成D2b-0真实隔离与20项独立补验

用户完整Windows重启后提供Docker Desktop“getting backend binary path / cannot find registry key”截图。实际LastBootUpTime已为2026-10-30T02:10:49.5000000Z、HypervisorPresent=true、CBS RebootPending=false，WSL2前提就绪。按原安装授权完成修复、固定镜像和实际验收；未重复索要原安装授权，也没有修改CortexOS业务数据/评分规则/迁移或提前进入D3。详细环境及结果见[harness-runtime-setup.md](harness-runtime-setup.md)、[harness-docker-verification.md](harness-docker-verification.md)。

### 安装恢复的实际依据

Docker文件及HKCU卸载登记存在，HKCU/HKLM的Registry32/Registry64均缺截图指定键，启动日志同样报错。当前工具与普通Windows Shell两次独立只读探针结果一致，进程均无包身份（GetCurrentPackageFullName=15700），物理文件位置真实，未把公开Issue关于MSIX的猜测用于本机结论。原完整安装包重新核验SHA2562668273f…59744与Docker Inc签名Valid。实际wsl --list --verbose无发行版，D盘和默认Docker/wsl路径均无VHDX，符合本次新安装尚未创建Docker数据的事实。

在原当前用户、WSL2、D盘范围内，签名完整安装包直接install返回3，缺失状态未恢复，未将其算作成功；内部具体返回3原因没有完整日志，不将推断写作确定事实。保留旧日志/卸载登记后，按Docker官方当前用户卸载入口执行已签名uninstall --quiet，退出0；原完整包以`install --user --quiet --backend=wsl-2 --wsl-default-data-root=D:\CortexOSRuntime\DockerData`重装，退出0。只进行一次受控卸载重装，不手工改注册表、不删目录、不切全用户安装，不触及CortexOS/Ollama数据。随后实际启动成功，日志已拉起com.docker.backend，docker version得到Linux Server29.9.2；旧注册表键仍不存在，因此不能声称“补回了该键”，具体恢复机制尚未定位。

实际磁盘在D:\CortexOSRuntime\DockerData\disk\docker_data.vhdx和main\ext4.vhdx，C盘默认Docker/wsl无VHDX。用户PATH登记及当前进程docker可见性确认。`docker pull python:3.13-slim`真实成功，实际Python3.13.18，linux/amd64，Image ID/RepoDigest为sha256:2d44738ebf4ce5f63b9d9ea871e9c7cfdc0426f7b0c97cac915fd88ded1d86bb。完整版本、端点、cgroup2及身份在artifacts/harness-runtime-setup/docker-environment.json；修复脚本/结果与原日志在同目录docker-repair-attempt。安装脚本未使用accept-license，未自动确认协议、登录、改BIOS/启动项或重启。

### 源码修改与真实安全验证

本轮产品侧只在`apps/desktop/src-tauri/src/benchmark/python.rs`追加cfg(test)模块登记；新增`python_live_tests.rs`两个显式忽略的真实Docker集成检查。复用原environment/create/inspect/score/有界command与cleanup，没有第二套执行器、模拟Docker响应或宿主Python回退。标准Case、评分规则、数据库迁移与前端界面未修改。

- `cargo test scoring_python_live_docker_limits_and_cleanup --lib --offline -- --ignored --nocapture`：最终1项通过、87项筛除，实际91.08秒，编译13.93秒；通过CORTEXOS_PYTHON_IMAGE固定实际ID，PATH仅为检查进程补入Docker目录。覆盖正确、错误、bool类型、修改输入、容器内实际UID/NoNewPrivs/CapEff/cgroup/网络/只读/Socket、超量输出、真实OOM、5秒超时、取消及清理。各场景expected/actual、逐断言、实际inspect与移除前State都保存到artifacts/harness-docker-live。
- 首轮同一检查实际正确实现已Passed，但证据筛选把context inspect的端点字符串误当容器inspect，测试失败（编译54.04秒，运行6.81秒）。只修正测试筛选排除context，并要求实际容器配置非空后复验；未变更运行参数或业务判定，不将失败算作通过。该轮已创建容器同样清理。
- `cargo test scoring_python_live_case_deadline --lib --offline -- --ignored --nocapture`：1项通过、88项筛除，运行60.24秒，编译14.43秒。受控合成32断言任务实际60.217秒触发Execution Error / case_wall_timeout，记录11项真实超时结果，其余未执行；容器全部清理。未修改原Core60题目。
- 最终10场景的所有实际拥有容器均已清理；评分正常/错误/类型等场景保留真实Passed/Failed，取消为Cancelled，整题超时为Execution Error，不混入普通正确率。额外docker ps按cortexos-bench名称筛选无遗留。CPU/PID配置与内核实际cgroup值检查通过，内存另有真实OOM；不宣称证明容器绝对安全或函数纯度。

### 标准题独立补验与构建

新增`apps/desktop/tests/benchmark-python-real.spec.ts`及Playwright独立显式opt-in项目，默认不运行真实模型。它先要求上述安全证据，复核原报告SHA256、原模型tag/digest、原Case/Dataset hash，固定Python ID，然后创建独立SQLite/Profile及新计划。没有改写旧20项。

- `npm.cmd run typecheck`通过。`npm.cmd run tauri -- build --debug --no-bundle`通过（含TypeScript，Rust33.97秒，Vite10.56秒）；前端资产保持index-D4XkGKGj.css / index-Bad9TjYB.js。原>500kB块提示保留，另有Vite回调计时提示；npm升级提示未触发升级。
- `CORTEXOS_BENCHMARK_PYTHON_REAL=1`后执行`npx.cmd playwright test --project=benchmark-python-real --output=../../artifacts/benchmark-python-real`，**1项通过，4.2分钟**。真实Ollama0.40.9及两原模型digest与历史一致，原4096/1024/temp0/120秒参数保留，未下载模型或调用云端。
- 新批次8a933739-988e-42c1-aac8-6d22cf0a9aba：20/20请求finished，qwen3:4b-instruct的10/10题通过、93/93真实断言通过；qwen3:4b的10/10因output_truncated保持Needs Review，未执行Python断言。调用覆盖100%、评分覆盖50%；后者正确率不适用，不能称0%或20题全通过。SQLite实际评分原因分别python_assertions_completed×10、output_truncated×10，无新Docker/评分器错误。
- 保存20组旧runId→新runId及case hash，原报告前后SHA256始终29b8f0d51f045f63492fbcb59340e24de72fe6d926bf5365e891793b7245a047。隔离库只含20条benchmark用途会话，普通聊天为空、knowledge_nodes=0，全局设置未改。
- 报告/每项详情/关联/实际环境/acceptance.json及原生截图路径：`artifacts/benchmark-python-real/benchmark-python-real-Pyth-98815-ut-changing-historical-runs-benchmark-python-real/`。已实际查看python-rerun-report.png，模型两组的正确率与评分覆盖分开呈现。一次只读摘要命令因转义失败未执行，改用PowerShell here-string后正确读出上述SQL结果，无数据修改。
- `git diff --check`通过，仅既有CRLF提示。未跑无关全套单元，无新依赖/锁文件/迁移；已有D1/D2a/D2b草案变更保留，没有提交、推送或Release。

当前停在D2b-0验收。草案的参考/原代码/错误实现对照、人工批准和正式发布仍为下一切片；不提前开发D3 Adapter。输出截断可后续独立增加预算作对照，不能覆盖当前基线。Windows系统缩放/读屏及长期跨硬件验收本轮未运行。

结果窗口已打开：`node artifacts/benchmark-python-acceptance/launch.cjs`复用本轮已完成的独立库，PID7552，未发送新模型生成；show.ps1确认Win32 visible=true，Windows置前请求返回false，不声称已置顶。用户可从任务栏进入查看。ready.png/launch.json及最终进度`artifacts/harness-runtime-setup/progress.json`记录交付点；Docker Desktop保持运行。

## 2026-10-30：恢复D2b-0，重启后环境仍未就绪

用户表示“重启好了”，恢复WSL/Docker的原安装授权，无需重复询问安装权限。重新阅读总纲/开发规范、实施计划、D2b方案、安装恢复及本机进度；没有修改应用业务代码或进入D3。

实际执行：Get-Process未发现Docker Desktop/com.docker.backend/CortexOS；Get-Command未找到PATH中的docker，故用已安装的绝对CLI路径，不能据PATH缺失假称未安装。提升只读Get-CimInstance Win32_OperatingSystem与安装前wsl-baseline.json对比，LastBootUpTime仍为2026-10-29T08:07:25.5000000Z。`C:\Program Files\WSL\wsl.exe --version`显示3.0.3.0及内核6.18.54.1；初次终端把UTF-16作为其他编码显示，随后用Python subprocess仅捕获/UTF-16解码`wsl --status`，取得真实报错“WSL2无法启动，因为此计算机上未启用虚拟化”，退出0。该退出码未当作就绪。

`Get-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform`返回Enabled；JSON明确读取固件虚拟化=true、SLAT=true、HypervisorPresent=false、CBS RebootPending=true、WindowsUpdate RebootRequired=false。`bcdedit.exe /enum '{current}'`仅查询，未显示显式hypervisorlaunchtype Off，未执行任何/set。已安装Docker CLI29.9.2运行`version --format '{{json .}}'`返回Client但Server=null，并报告本地docker_engine管道不存在；Docker settings-store.json尚不存在。D:\CortexOSRuntime\DockerData仍存在，实际磁盘位置未核验。

已实际查阅微软[WSL故障排查](https://learn.microsoft.com/en-us/windows/wsl/troubleshooting)，其0x80370102部分要求启用VirtualMachinePlatform后重新启动，并检查启动配置；当前没有据此盲目更改BIOS或启动项。向用户询问刚才使用Windows“重新启动”、关机再开机还是应用重启，以确定后续路径。系统仍报告待重启，不推断用户操作错误或环境已成功。

诊断保存在`artifacts/harness-runtime-setup/post-reboot-check.json`；恢复/实施文档同步。未启动Docker或容器、拉镜像、执行代码/模型、改旧20项、运行单元/构建或触发自动重启。保留下一步实际Linux服务、D盘磁盘、固定Python镜像及隔离实测门槛。

用户随后确认是Windows关机后再开机；实际查阅微软[System power states](https://learn.microsoft.com/en-us/windows/win32/power/system-power-states)，其中快速启动保留内核会话、Restart进行完整启动的说明与本机时间未变现象吻合。没有额外读取或更改快速启动/BIOS设置。已明确请用户通过开始菜单→电源→重新启动，再打开Docker Desktop；保存恢复点后等该外部步骤，不自动触发重启。

## 2026-10-30：D2b草案准备，暂存重启门槛

用户要求“暂存该步骤，先把不需要重启的做了”。保留WSL/Docker已安装的恢复记录和全部未验收项，按[D2b方案调整](harness-fit-d2b-plan.md)提前交付[Python任务草案](harness-fit-d2b-drafts.md)，不进入D3或实际代码执行。复核development.md、开发/实施/界面规范、D2a数据与UI、现有Python控制器和评分合同，沿用ponytail与ui-ux-pro-max的最小复用/可访问表单原则；本地UX查询`py -3 D:/CodexData/skills/ui-ux-pro-max/scripts/search.py 'progressive disclosure form validation' --domain ux`返回表单标签、提交反馈与字段检查建议，未生成新视觉体系或依赖。

### 实际修改

- 新`apps/desktop/src-tauri/src/benchmark/harness/programming.rs`及`programming/tests.rs`：人工草案、固定分析/项目/文件范围、评分合同静态验证、目标/边界/回归用例、参考/错误代码文本、来源与内容hash、追加修订/重开和发布隔离。复用D2a快照检查、Case验证/封装及现有Python资源策略；代码字符串不执行。
- 修改`harness/mod.rs`和`harness/tasks.rs`，仅接入模块并将快照校验共享到同一模块范围；Approval字段同范围可见用于必要发布拒绝检查。`src-tauri/build.rs`、`src-tauri/src/lib.rs`及`capabilities/default.json`只增加读取/保存编程草案两个业务命令和精确权限，没有通用文件/终端权限。
- 新`src/benchmark/harness/ProgrammingDrafts.tsx`、`programmingTypes.ts`，修改`TaskWorkbench.tsx`与`harness.css`，默认折叠嵌入现有步骤；类型/用例/代码、历史只读、切页草稿、关闭/放弃确认、来源查看和错误恢复。沿用页面列宽、Tokens、SelectMenu/AppDialog/FitStatus，未增加重复标题或卡片体系。
- 新`tests/harness-programming.spec.ts`并登记Playwright desktop项目。同步D2b计划、运行恢复、实施/数据库/设计/检查文档及文档索引。没有新迁移或依赖，schema仍11；既有0001–0010与HEAD无差异。此前D1/D2a未提交变更完整保留，没有提交、推送或更新正式发布物。

`harness_programming_drafts`用途复用generation_jobs/generated_cases，所有读写限定工作区与用途；计划及修订沿用不可变触发器，旧修订拒绝覆盖。草案不进入普通模型/只读任务发布，不写入聊天、知识、执行或正式题库。容器/任务执行及正反对照实际状态均为Not Run，publishable=false；结构合法不代表语法、行为或题目覆盖充分。

### 实际命令与结果

- `git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS；`git diff --check`通过，仅既有CRLF转换提示。
- `npm.cmd run typecheck`通过；两次最终构建也含TypeScript检查。
- `cargo check --lib --offline`通过，27.23秒。
- 必要信任边界专项`cargo test harness_programming_drafts_preserve_scope_history_and_execution_gate --lib --offline -- --nocapture`：最终1项通过、86项未运行（执行0.08秒，编译7.53秒）。最初测试错误假设新库已初始化2个内置题库，实际尚未惰性加载、计数0；已将期望修正为草案路径不新增题库，并保留其他表零写入检查。没有为通过测试改业务行为；没有运行无关全套单元。
- `rustfmt --edition 2024 src/benchmark/harness/programming.rs`只格式化新增模块及其测试。`npm.cmd run tauri -- build --debug --no-bundle`两次通过；最终Rust24.65秒，前端`index-D4XkGKGj.css`/`index-Bad9TjYB.js`。原Vite>500kB块提示保留；npm版本提示未触发任何升级。
- 原生首次`harness-programming.spec.ts`发现第二次保存时新revision先到、旧元数据尚未刷新，渲染对不存在修订取值导致空白；SQLite已正确保留两版。保留初次失败及一次诊断重现，随后改为只渲染当前确实存在的修订详情，不改保存语义，未将失败记作通过。失败目录`artifacts/harness-programming-drafts`与`artifacts/harness-programming-drafts-debug`。
- 修复后`npx.cmd playwright test --project=desktop harness-programming.spec.ts harness-tasks.spec.ts --output=../../artifacts/harness-programming-verified` **2项通过，17.4秒**（新草案7.9秒、旧D2a7.5秒）。新草案覆盖真实系统ZIP选取、类型错误恢复、保存/追加、发布拒绝、切页草稿/关闭提醒、放弃取消、历史只读、来源行、重载和原修订不变；旧D2a审核/修订/两版发布通过。两者都使用独立测试库，不执行导入代码或模型。
- 人工审阅截图发现新检查脚本只写data-theme，未操作实际data-color-scheme，初轮所谓dark图仍为浅色，未将其记为深色验证通过。改为应用“外观”选择器并断言实际根属性，重新执行`npx.cmd playwright test --project=desktop harness-programming.spec.ts --output=../../artifacts/harness-programming-final` **1项通过，9.7秒**（场景7.6秒）。它是同一场景复验，不累计为新增用例。最终1366×768、1600×900、1920×1080浅/深6张原生截图和横向溢出检查通过；已实际查看浅色1366与深色1920原图。路径`artifacts/harness-programming-final/harness-programming-progra-719ad-fuse-unverified-publication-desktop/drafts-{light,dark}-{1366,1600,1920}.png`，元信息`programming.json`。

### 保留门槛

本轮未继续Linux服务启动、数据磁盘位置核验、Python镜像下载、安全容器执行、原20项Python题补验、云端或第三方Harness调用。原安装授权与已安装文件保留，不重启用户电脑。实际Windows系统缩放、实体中文输入法及读屏本轮未验收；视口截图不替代这些检查。草案代码只有静态结构检查，正式容器对照/人工批准及发布后续继续；本轮停在草案准备验收点。

验收窗口：`node artifacts/harness-programming-acceptance/launch.cjs`成功启动调试版PID11328，复用最终专项的独立QA数据库，显示第2次草案修订和浅色界面，未生成或批准新任务。`pwsh.exe -NoProfile -File artifacts/harness-programming-acceptance/show.ps1 -AppProcessId 11328`确认Win32 visible=true，SetForegroundWindow返回false；后续标准WScript.Shell.AppActivate返回true，但实际前台PID仍非11328，不能声称已置顶。用户可通过任务栏切到该窗口；ready.png及launch.json保留真实状态。没有强制置顶、发送全局按键或关闭其他窗口。

## 2026-10-29：D2b-0官方运行环境安装，待重启

用户明确批准官方WSL2、当前用户Docker Desktop及Python3.13镜像下载，数据拟放D盘。按[恢复记录](harness-runtime-setup.md)执行系统安装；本轮没有应用业务源码、迁移或依赖变化。

### 实际执行与失败恢复

- 安装前确认官方默认用户/系统路径均无Docker，D:\CortexOSRuntime尚不存在。下载/日志及拟定数据目录只使用该用户批准的新目录。
- 首次Get-WindowsOptionalFeature传数组因API只接收单一名称失败；改为逐项读取，保存真实Disabled基线。`wsl --install --no-distribution --web-download`退出1，未安装成功；按Docker官方文档的MSI途径从微软稳定发布绑定3.0.3。
- `Enable-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform -All -NoRestart`成功，返回RestartNeeded=true；功能显示Enabled。没有调用重启，没有额外安装Ubuntu或为WSL1启用旧组件。
- Docker/WSL官方大文件遇到慢连接及WSL curl56 TLS中断；只停止经进程名/命令核对属于本任务的下载父子进程，保留部分数据。使用8路、8MiB有界Range、固定官方版本地址与重试续传，90段最终完成；没有关闭TLS验证或使用第三方镜像。下载辅助脚本仅位于ignored artifacts，不进入产品。
- Docker官方更新XML/校验文件确认4.97.0 build244584、620496816字节、SHA256 2668273f82b342a058a28f9ab53ea7bddde10bb6d51307ce2ec54c2d36459744；微软稳定发布API确认WSL3.0.3、367575040字节、SHA256 ae977286974255bd26bf925f00f40f5692c6b2084155cbc0563c09aea091f211。合并后两个完整hash与官方一致，Get-AuthenticodeSignature均Valid，发布者分别Docker Inc、Microsoft Corporation。
- 一次较长的只读安装后检查因自动审批审查超过时限而未执行，不代表安装失败；按工具允许的单次重试缩小为两个已装程序文件版本读取后成功。没有为了检查而读取个人Docker凭据或启动容器。

### 实际安装与当前门槛

PowerShell安装脚本先语法检查（未执行），随后在再次验证完整大小/hash/签名后执行：WSL MSI `/qn /norestart REBOOT=ReallySuppress`退出0；Docker `install --user --quiet --backend=wsl-2 --wsl-default-data-root=...`退出0，未使用--accept-license。Windows文件路径会规范化重复分隔符，恢复脚本已统一使用规范写法；只读确认目标为D:\CortexOSRuntime\DockerData。

实际文件`C:\Program Files\WSL\wsl.exe`版本3.0.3.0，当前用户`AppData\Local\Programs\DockerDesktop\Docker Desktop.exe`版本4.97.0.244584。安装结果JSON明确LinuxRuntimeVerified=false、PythonImageDownloaded=false、RebootRequestedByAgent=false；数据目录创建与安装参数不替代首次启动后VM磁盘路径验证。

Windows已要求一次手动重启。本轮止于此真实依赖门槛，不启动Linux容器、不下载尚无服务可承接的Python镜像、不执行模型或仓库脚本。下一步用户保存编辑并重启、打开Docker Desktop并自行确认首次协议，之后继续服务/镜像/安全与原20项补验。无需再询问安装授权。

通过读取原report.json，保存旧20项docker_unavailable的模型、case/run/hash引用到`artifacts/harness-runtime-setup/previous-python-blocks.json`，原报告hash及历史数据未改。没有运行无关单元、前端构建或模型测试；项目无应用源码变更，只有准备/恢复文档与本机安装操作，不伪造D2b任务或D3执行通过。`git diff --check`用于文档空白检查。

## 2026-10-29：Phase D继续，D2b安装前复核

用户要求“往下做phased”。重读Phase D附件、D2分片/交付、D0就绪报告和实施/开发规范，复核`benchmark/python.rs`的本地Docker检查、固定Image ID、受限create/inspect/score/cleanup及Runner入口，未新建执行器。新增[harness-fit-d2b-plan.md](harness-fit-d2b-plan.md)，以安装和安全环境验收作为下一切片。

实际Get-Command仅找到wsl.exe，未找到docker/winget；Python只读捕获`wsl.exe --status`，退出50且UTF-16解码明确提示WSL未安装。沙箱内CIM被拒绝，提升后只读核对Windows11专业版build26200、64位、VirtualizationFirmwareEnabled/SLAT均true；该检查进程具管理员权限。盘剩余C23.5GiB/D581.7GiB为本次观测，不保证之后不变。没有据沙箱内False推断实际用户无管理员权限。

已实际打开Docker官方Windows安装/参数、微软WSL安装及基本命令文档，确认可按当前用户、WSL2后端和no-distribution方式准备。补充只读核对物理内存15.7GiB，LanmanServer为Running/Automatic，无需为此更改服务设置。拟将Docker镜像数据放D:\CortexOSRuntime\DockerData，具体命令/验证/恢复门槛已写计划，等待用户按development.md第0节批准系统安装；尚未下载安装包、启用系统功能、拉镜像、运行编程题或修改宿主环境。本轮未运行单元/模型测试，不伪造安全环境通过。

## 2026-10-29：项目适配状态独立标签

用户要求状态不再用“·”与名称拼接，安装版本未知等提示使用单独红框。新增仅用于本模块的纯呈现FitStatus组件与既有语义Tokens样式，统一目录列表/能力详情、静态比较、任务未运行、审核及评分规则检查状态。未知、未运行、待核实、明确不兼容和检查失败保留各自文字；红色只作提示，不改为Failed或“未安装”。已人工批准/规则检查通过有独立成功标签；评分负对照被正确拒绝仍为符合预期，不因实际答案结果为Failed而冒充评分器失败。元数据/引用定位与原始资料内容保持原语义。

修改范围：`harness/FitStatus.tsx`、`HarnessFitLab.tsx`、`FitReport.tsx`、`TaskWorkbench.tsx`、`TaskReview.tsx`及`harness.css`；D1原生专项增加五个未知版本标签检查和浅深目录截图，界面规范同步。没有数据结构、评分规则、模型请求或新依赖变化。

用户确认关闭当前窗口后，`npm.cmd run tauri -- build --debug --no-bundle`通过（含TypeScript，Rust24.52秒），资产index-BXXv1-Af.css / index-B7M9RPgG.js；Vite原分块提示保留。`npx.cmd playwright test --project=desktop harness-fit.spec.ts harness-tasks.spec.ts --output=../../artifacts/harness-status-labels` **2项通过（21.1秒）**：D1场景11.3秒、D2场景7.5秒，保留三尺寸浅深、步骤对齐、原文/保存/审核发布。未运行单元或无关全套测试；新失败提示分支以逻辑检查为主，实际UI场景为未知版本、未验证/运行、人工批准与规则检查通过，不虚构真实失败运行。

已查看`artifacts/harness-status-labels/harness-fit-local-document-edc9c--reviewed-evidence-versions-desktop/catalog-status-{light,dark}.png`及D2浅色1366实际原图：工具名正常文字，五个“安装版本未知”红色细框独立对齐；未运行与人工批准也分开显示。状态有文字，不依靠颜色，非交互标签不加入实时播报。只读UI检查没有自动检测/安装任何第三方工具，仍处于D2a验收。`git diff --check`通过，重新打开原独立QA数据供查看。

## 2026-10-29：四步布局稳定与重复标题移除

用户反馈第四步测试任务页面左移，并要求取消上方入口已经表达的重复页面标题。代码确认根因为第四步隐藏资料aside、切换单列，导致正文左缘改变。最终四步共享相同资料列/间距/正文轨道，第四步保留资料摘要并限制修改，返回项目资料后再编辑，固定任务来源不被意外改动。四步导航和原版本/保存操作合并为透明操作行，删除“项目适配”和“项目任务/文档分析 · 本地处理”的重复介绍。第四步分析保存按钮保留占位并隐藏，导航和版本控件不会因按钮消失重排；任务仍通过原审核栏发布。

实际修改限`harness/HarnessFitLab.tsx`、`harness/harness.css`、两个原生专项及界面规范/本文；没有后端、数据库或依赖变化。`npm.cmd run tauri -- build --debug --no-bundle`通过（含TypeScript，Rust46.38秒），资产index-BNvH4Sly.css / index-DDIM6dHE.js；原Vite分块提示保留。用户确认关闭原验收窗口后更新程序。

`npx.cmd playwright test --project=desktop harness-fit.spec.ts harness-tasks.spec.ts --output=../../artifacts/harness-layout-stable` **2项通过（20.0秒）**，D1场景10.3秒、D2场景7.5秒。在1366×768、1600×900、1920×1080分别实测第三→第四→第二步的导航/正文/资料列/版本操作横坐标与宽度，前后差异均小于1 CSS px；确认没有同名页面heading，另复核原文、保存/历史、草稿及任务审核发布。截图覆盖三尺寸浅深，已查看浅色1366和深色1920实际原图，目录`artifacts/harness-layout-stable/harness-tasks-project-task-887cc-isolated-immutable-versions-desktop`。未运行单元或无关全套测试。

正文继续独立滚动，步骤和版本操作常驻，原任务未运行状态保持。重新打开原D2独立QA数据供验收，不自动出题、不推进D2b。`git diff --check`通过。

## 2026-10-29：移除项目适配标题底色块

用户继续反馈“项目适配”标题所在区域突兀。本次直接移除标题的背景，不再用近似颜色模拟页面底色。标题及版本/保存操作置于滚动区外，正文新增透明fit-scroll滚动区；任务审核栏在正文顶部固定，标题和正文不重叠。删除原标题ResizeObserver/高度变量，不改业务状态、后端、数据库或依赖。

实际修改：`src/benchmark/BenchmarkPage.tsx`仅为项目适配分支增加宿主布局类；`harness/HarnessFitLab.tsx`调整标题/正文容器并删除测高；`harness/harness.css`设置透明标题与独立滚动；`tests/harness-tasks.spec.ts`改用实际滚动区，并核对滚动前后标题位置不变；界面规范同步记录最终规则。

用户确认关闭原窗口后，`npm.cmd run tauri -- build --debug --no-bundle`通过（含TypeScript，Rust21.24秒），资产index-CWImVkxQ.css / index-Bd6B25KJ.js。`npx.cmd playwright test --project=desktop harness-fit.spec.ts harness-tasks.spec.ts --output=../../artifacts/harness-title-transparent` **2项通过（20.3秒）**，覆盖文档导入/原文、保存版本/历史、任务修订发布/草稿、三尺寸浅深截图，以及滚动后保存/审核操作可达、标题位置稳定；未跑单元或无关全套检查。

已实际查看`artifacts/harness-title-transparent/harness-tasks-project-task-887cc-isolated-immutable-versions-desktop/tasks-light-1920.png`及`tasks-dark-1366.png`：标题直接显示在连续页面底色上，没有独立矩形填充。`git diff --check`通过，原Vite大块提示保留；本轮不推进D2b。验收助手改为复位fit-scroll，重新打开原独立QA数据供用户查看，不生成新任务。

## 2026-10-29：项目适配固定栏底色修正

用户指出项目适配标题、逐题审核两条固定栏与底色不符。本轮仅修改`apps/desktop/src/benchmark/harness/harness.css`及界面规范/本文，保留全部布局、固定高度测量和业务代码。旧surface叠加background过亮；最终统一使用现有glass叠加background形成不透明、接近外层页面的浅深底色，滚动文字不会穿透，没有新增边框或卡片。

初次尝试3%微色层及嵌套24px背景模糊，原生截图仍有泛白，未将它当作最终视觉通过；改用现有玻璃颜色直接合成，不做第二层backdrop-filter。预览addStyleTag被现有CSP拒绝，未放宽CSP；随后通过元素CSS属性检查效果，再写入样式并重新构建。用户已确认关闭原验收窗口；中间预览进程在检查无未保存草稿后正常关闭，没有强制结束进程。

最终`npm.cmd run tauri -- build --debug --no-bundle`通过（含TypeScript，Rust20.20秒），资产index-CMRUB8CS.css / index-DUkYY_A8.js；原Vite大块提示保留。`npx.cmd playwright test --project=desktop harness-tasks.spec.ts --output=../../artifacts/harness-surface-final` 1项通过（9.5秒，场景7.4秒），包括原文、修订发布、历史、三尺寸浅深截图和固定栏实际命中。未运行单元或无关全套测试。已实际查看最终浅色1920/深色1366原图，路径为`artifacts/harness-surface-final/harness-tasks-project-task-887cc-isolated-immutable-versions-desktop/tasks-{light,dark}-{1366,1600,1920}.png`。`git diff --check`通过；没有数据库、模型、Runner或新依赖变更。

最终窗口重新使用原D2独立QA数据与Profile，不生成或批准新任务。D2a验收门槛保持，不进入D2b。

## 2026-10-29：Harness Fit Lab D2a任务生成与审核

D1后用户授权“往下做”，并明确选择“先交付任务生成与审核闭环（推荐）”。先重读Phase D附件、总纲/规范/实施计划和D1实际交付，复核Phase C扫描器、候选/发布/Runner链路。Get-Command未检测到docker；本轮不重新声称WSL或真实执行已就绪。按[harness-fit-d2-plan.md](harness-fit-d2-plan.md)限定D2a，详细文件与边界见[harness-fit-d2.md](harness-fit-d2.md)。基线仍0a1c7a9，上轮D1变更原样保留，没有提交/推送/发布。

### 实现与逻辑检查

从保存的D1确认要求生成规范原文题，从用户明确导入的项目快照生成代码片段定位题。标准答案/合法重复路径从真实来源导出；带来源hash/行号、分析/仓库版本和只读合同，最多12项/30行/8KB片段。复用现有exact_alias v2，实际检查每个参考答案Passed、空答/错误答/多余工具调用Failed；记录输入与实际结果，任务仍为Not Run。相同片段产生重复题时拒绝，不编造Bug、修复或调用链语义。

复用Phase C四张计划/候选/审核/题库表，JSON purpose=harness_tasks。普通个性化查询/读取只接受personalized，缺省旧记录继续兼容；项目任务不插入普通benchmark_cases，Runner读取明确拒绝。修订/任务集只追加，事务复核最新revision/expectedVersion，任务发布Case绑定任务集version/hash。逻辑审阅发现最新候选若按修订rowid排序会错配原始来源，已在首次验证前改为该题最初rowid稳定顺序；检查覆盖连续修订不同题及来源不串位。

共享已有项目选择/扫描帮助函数，保持原云端入口前后guard，本地导入不要求云端密钥；C项目列表排除仅文档快照，避免摘要字段不足。新增七个实际业务命令，无通用文件/终端权限。前端复用第四步、列表/details/SelectMenu/AppDialog与现有Tokens；统一父级批准状态，修订/重读失效旧批准，跨步骤保留草稿及关闭提醒。截图复核后把审核栏偏移改为测量实际标题高度，消除固定70px假设；没有全局UI重构或新依赖。

### 实际执行的验证

- `git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS；按用户要求仅必要专项，不跑无关全量单元。
- `cargo check --lib --offline`通过（54.42秒）。`npm.cmd run typecheck`独立检查通过，最终桌面构建也包含TypeScript检查。
- `cargo test harness_tasks_ --lib --offline -- --nocapture`最终2项通过、84项未运行（0.16秒；编译12.03秒）。覆盖用途/工作区、无效来源/未确认/行号、重复任务、多合法答案、对照判定、修订并发及来源稳定、发布版本/内容hash、不可变/重开和普通聊天/题库兼容。多轮仅因新增重复题及发布版本校验复验，不累计为更多用例。
- `cargo test personalized_publication_revisions_are_immutable_and_reuse_existing_scoring_and_trace --lib --offline -- --nocapture`已有兼容专项1项通过（0.14秒，85项未运行）。无云端真实请求。
- `npm.cmd run tauri -- build --debug --no-bundle`最终通过，Rust24.67秒；资产index--oG4WsOd.js 693.46kB / index-tKdnOMtb.css 67.17kB。仍有既有Vite大于500kB分块提示，不额外引入构建优化或安装包工作。
- 首轮 `npx.cmd playwright test --project=desktop harness-tasks.spec.ts cloud-native.spec.ts --output=../../artifacts/harness-d2-native` 2项通过（13.5秒）。实际截图核对后增量补齐动态固定栏/草稿/批准状态，完成新构建再复验。
- 最终 `npx.cmd playwright test --project=desktop harness-tasks.spec.ts harness-fit.spec.ts cloud-native.spec.ts --output=../../artifacts/harness-d2-verified` **3项通过（22.0秒）**：C准入4.0秒、D1实际文件选择/历史9.1秒、D2实际ZIP导入/审核6.6秒。D2导入development.md使用Debug指定路径实际读取，未冒充该次由系统选择；ZIP使用真实系统文件选择器，文件内容为自建受控源码，不执行它。D1回归仍使用真实系统选择/取消。
- D2验证来源逐行定位、重复片段两合法文件、修订、跨步骤草稿/关闭提醒、两版发布/只读历史、旧版不变、跨工作区拒绝、三尺寸浅深截图、固定栏实际命中/不被标题覆盖、无普通聊天/运行以及无页面console错误。原生截图目录：`artifacts/harness-d2-verified/harness-tasks-project-task-887cc-isolated-immutable-versions-desktop/tasks-{light,dark}-{1366,1600,1920}.png`；已实际查看浅色1920/深色1366原图。没有独立浏览器替身检查，不把CDP视口尺寸称为系统缩放。
- 只读SQLite核对：schema11，D1分析1、来源快照2、任务计划1、候选修订4、发布审核2，任务集v1有2项/v2有1项；execution_runs/knowledge_nodes/source_documents均0，foreign_key_check无错误。验收操作是QA数据，不是用户对真实项目任务的批准。
- `git diff --check`通过（既有CRLF提示），新D2源码空白检查通过；逐字节比较0001–0010与HEAD相同，0011未修改，无D2迁移、依赖或锁文件变更。

### 门槛与后续

本轮仅D2a。D2b功能/修复/跨文件/回归/兼容/工具/安全行为仍需安全环境、失败基线、参考补丁与负对照；没有运行第三方Harness或模型Task Performance，也没有付费/安装/下载。系统150%与读屏本轮未重测。Docker缺失及历史20项编程阻断保持；完成后停在D2a等待验收，不自动进入D2b或D3。

验收窗口17012使用harness-d2-verified下的独立QA数据，另设WebView2 Profile，展示“评测 → 项目适配 → 测试任务”；当前代码项目是受控小型示例，规范来自实际development.md，不是原用户库。打开助手首次在about:blank阶段过早写hash导致定位超时，属于验收脚本问题；已加载实际tauri.localhost并补上等待应用URL/导航，重新定位成功，没有改动产品代码或重写任务。Win32确认visible=true、foregroundGranted=true；`artifacts/harness-d2-acceptance/ready.png`已实际查看。未自动生成/批准新任务；任务版本仍为检查时的两版。

## 2026-10-29：Harness Fit Lab D0–D1

用户通过附件授权先D0、无文档分析阻断后进入D1，完成后停止。基线0a1c7a9，初始工作区干净。完整阅读附件及项目总纲/规范、Phase C及本机120项验收证据，按实际代码复核；D0输出[harness-execution-readiness.md](harness-execution-readiness.md)，D1方案/文件/接口/边界见[harness-fit-d1-plan.md](harness-fit-d1-plan.md)及[harness-fit-d1.md](harness-fit-d1.md)。没有把C3–C5替身/受控验证当作真实付费云端验收。

### 审计、资料及范围

Get-Command未发现docker，wsl.exe --status退出50，按UTF-16LE解码明确提示WSL未安装；最初PowerShell管道显示乱码，后用只读Python子进程捕获纠正，没有执行安装。当前允许D1本地文档处理，D3执行继续阻断。根design.md实际不存在，采用AGENTS.md指定HTML规范和既有Soft Glass Tokens。复用ponytail/ui-ux-pro-max及OpenAI Docs；官方页面已实际打开核对，不仅使用搜索摘要。两次本地“progressive disclosure”UX搜索未找到精确匹配，未把不相关结果作为规范依据，折叠/固定操作延续既有界面规则。

本轮查阅Codex非交互/Windows/项目指令、Claude程序化/Windows/记忆、Aider脚本/Ollama/安装/编码约定、OpenHands终端/headless、Cursor CLI/输出格式等官方资料，目录逐事实绑定URL和查阅日。若URL失败（早期Aider脚本路径、Codex agents-md路径），改查实际可访问官方页面，未将404页面当证据。目录实际安装版本/运行认证均未知，不因CortexOS自身可运行或已配置API Key标为Harness兼容。未运行任何第三方Harness、付费推理、依赖/模型下载或宿主安装。

### 实际实现

新增benchmark/harness的extract/fit/mod和版本化JSON目录；复用sources::read_selected及benchmark_repository_snapshots，不写生产知识。12类原文线索、行/hash/引用、提取置信和用户确认/修改可保存；否定/未来/可选/举例不自动关联成正向要求。硬性不兼容排除，未知保持待核实；四类权重0–5、相同能力只计一次，展示支持下界/未知上界、证据覆盖及来源，实测数量0。只查看已保存结果不重算，不产生未保存提示。

0011只增加harness_fit_assessments追加版本表，expectedVersion保护并发，正文绑定目录快照/规则/用户修改和前hash；旧迁移不改。lib/build/capabilities仅登记5个本地业务命令。前端新增HarnessFitLab/RequirementEditor/FitReport/types/CSS，BenchmarkPage增加项目适配子页；长需求按类别折叠，保存栏固定，源文件用原AppDialog逐行定位。没有第二套Runner、Provider、评分器或图谱。

### 实际命令与验证

- `git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS。npm命令在apps/desktop，Cargo在src-tauri；测试数据和Profile均使用仓库artifacts隔离目录，不操作正式用户库。
- 首次Rust check发现MutexGuard借用及Result错误类型推断问题，修正后 `cargo check --lib --offline`通过（6.86秒；格式化后5.26秒）。新增检查首次编译又发现数据库方法名和usize FromSql不匹配，只修正检查代码，未虚构执行通过。
- 必要 `cargo test harness_ --lib --offline -- --nocapture` 3项先后随置信字段、保守映射和大小写后缀修订复验通过，最后0.08秒，81项未运行。覆盖否定/未来候选、未知和硬过滤/去重计权、来源防伪、工作区隔离、追加/并发/不可变/历史和重开；迁移专项从schema10/旧聊天故意失败、检查备份可读/回滚，再应用11，旧聊天保留。没有无关全套单元测试。
- 类型检查初次因新原生检查的未使用类型导入失败，移除后通过。`npm.cmd run tauri -- build --debug --no-bundle`各次成功（Rust51.77/28.15/32.25/最终29.88秒），分别覆盖初版、折叠固定栏、保守映射及只读查看修正。最终含TypeScript/Vite通过：主块index-Ct0nUFbQ.js 677.49kB、样式index-Cxp4D9RX.css 65.46kB、GraphViewport941.05kB；保留既有大块提示，没有生成安装包或改变依赖。
- `npx.cmd playwright test --project=desktop harness-fit.spec.ts cloud-native.spec.ts --output=../../artifacts/harness-d1-native`：2项通过（20.3秒；4.1/13.0秒）。真实系统窗口取消/选取development.md，Rust/SQLite需求分析、原文定位、硬要求、权重、两版/旧版本/重载和三尺寸浅深；原个性化直接调用仍被Guard拒绝，固定题库和原四页导航可用。旧cloud-native页面标题断言按此前去重设计改为顶部标题，没有恢复重复标题。UIA助手仅增加可选窗口标题/打开按钮匹配，默认保存行为和指定进程/仓库路径限制保持。
- 后续针对折叠/固定栏、保守提取及只读浏览的具体改动，仅顺序复验D1场景：harness-d1-native-final 1项通过16.7秒、harness-d1-delivery 1项通过12.5秒、最终harness-d1-verified 1项通过19.8秒（场景17.5秒）。不累计重复运行作为不同场景。最后验证只读进入结果不会产生beforeunload草稿提示，保存按钮实际命中未被遮挡。
- 只读SQLite核对最终库schema11、2个分析版本、1份项目资料快照、0个ExecutionRun/知识节点/知识来源，PRAGMA foreign_key_check为空。没有用模拟生成文字冒充本轮适配事实，也没有创建虚构执行轨迹。查看实际深色1366/浅色1920及最终验收窗口图；六尺寸/主题截图由真实WebView2生成。没有独立浏览器D1替身检查，未声称做过。

### 验收窗口与剩余门槛

用户已确认关闭所有旧验收窗口。最终启动进程3480，复用独立D1真实文档/自动化审核验收库及新WebView Profile，进入“评测→项目适配”。show.ps1返回visible=true、foregroundGranted=true；截图artifacts/harness-d1-acceptance/ready.png已查看。只读准备没有增加分析版本或模型调用，生产库及原验收库保持原样。实际窗口可供用户继续导入/编辑/检查来源。

规则分析不是完备自然语言理解，未关联的复杂约束需用户确认；本轮无Harness安装/版本/模型兼容/权限运行实测。Docker/WSL和此前20项编程题复测仍待授权环境准备；系统150%/读屏、跨硬件未测。D2任务生成、D3终端/补丁执行、D4组合实验、D5图谱联动均未启用。D1交付后停止等待验收，不自动提交/推送或发布。

## 2026-10-29：Benchmark 8——真实本地完整计划与交付

用户授权按原计划执行最后一步，7作为已验收依赖。完成[切片8](benchmark-acceptance-slice8.md)本机可执行范围，停止等待用户验收；Docker Linux/跨厂商等环境门槛保留未完成。没有修改生产业务代码、数据库迁移、题库、标准答案或依赖，没有发布/提交/推送。新增tests/benchmark-acceptance-real.spec.ts，playwright.config.ts只增加显式开关，阶段/实施/测试/索引文档同步。

读取现有阶段计划、预检、调度、计量、评分/状态及导出实现；复用ponytail最小方案。实际loopback的tags/ps/version确认Ollama0.40.9、两个不同digest的已安装Qwen身份，Docker CLI未检测到。未下载模型、安装系统工具或调用云端。实验numCtx4096、numPredict1024、temperature0、超时120秒，冻结Core60 v2 hash及模型digest。实际条件、版本、原文和参数见environment/report/details，不作品牌能力或最大上下文推断。

类型检查通过。显式CORTEXOS_BENCHMARK_ACCEPTANCE_REAL=1运行 `npx.cmd playwright test --project=benchmark-acceptance-real benchmark-acceptance-real.spec.ts --output=../../artifacts/benchmark-8-real`：真实运行约12.9分钟，先两题两次重复两模型共8项，实际停止/恢复并核对2个已完成runId不变；再完整120项。终态100次完成调用、20项docker_unavailable，所有输出/评分和逐题详情已保存。检查脚本最后误用in_progress/not_run状态键，断言失败；实际键为“In Progress”/“Not Run”。改正检查脚本，原数据和评分不变，不把首轮组合写成通过。

设置CORTEXOS_BENCHMARK_ACCEPTANCE_SAVED为原隔离证据目录，仅复用其数据库/已保存原文，输出到benchmark-8-saved-verification；同一专项1项通过（约1.2分钟）。追加100条独立评分、原成绩不变，原生系统窗口实际保存JSON/CSV。无新模型生成。SQLite只读对账确认model_request_started仍107（含小批次7次）、execution_runs128、messages256、context_snapshots128、execution_events844，知识节点/版本0；评分128→228，原全计划120个评分ID/outputHash/metrics/details逐条相等。JSON含120计划项，8,113,312字节；CSV标准库实际解析37列220条评分明细，1,852,659字节，行列数一致，哈希归档。107不是全计划请求数，220不是题目数。

全计划instruct Passed38/Failed12/Docker阻断10，已评分38/50、覆盖50/60；另一身份Passed9/Failed22/Needs Review19/Docker阻断10，已评分9/31、覆盖31/60。两者实际执行成功均50/50，未运行/不支持/评分器错误0。截断和环境错误没有混入正确率分母；数学可见解释性文字违反输出格式不等于不会计算。RAG事实和引用分别记录，原生工具真实对象各5/5；Python0/0、20项环境阻断，不是编程能力0分。分类、难度、引用和Token/资源口径详见切片8，不生成综合能力榜。

本轮未运行单元或无关套件，复用最新已构建产品程序（最近构建33.03秒，SHA256 345CD4E06B164ABA3412A720D321B6CFB6F10D34461975687A48190D2295B4E8），新增的仅为外部验收脚本和文档。完整真实执行中留存100次资源记录，模型间速度不是独占硬件/受控冷缓存比较；没有用两个同系列身份代替多厂商验收。

启动独立真实结果窗口30088，工作区“Core60 v2 真实本地验收”，使用本轮隔离库，不覆盖原用户验收窗口和正式数据。只读导航并查看artifacts/benchmark-8-acceptance/ready.png，报告统计与原始记录一致。show.ps1返回visible=true、foregroundGranted=false，可从任务栏进入；没有自动新调用。Docker真实安全执行、其他厂商模型、长时跨硬件、新评测页系统150%/读屏等仍是剩余门槛。完成本轮后停止等待确认。

## 2026-10-29：区分导航与知识列表按钮

用户随后确认已保存关闭旧窗口。`npm.cmd run tauri -- build --debug --no-bundle`成功，含类型检查/Vite，Rust33.03秒，保留既有大JS块提示。复用独立验收数据打开进程16064，执行本机check-list-controls.cjs实际点击两组开关，核对分别收起/展开而不改变另一组状态，动态提示正确；查看实际截图list-controls-ready.png。未修改知识内容或启动模型。show-current.ps1确认visible=true、foregroundGranted=false，窗口可从任务栏打开。完成本次图标调整，无新单元测试或无关套件。下段为此前等待关闭时的记录。

用户指出两处分栏图标功能不清。本轮修改Knowledge.tsx，将局部列表开关改为已有lucide ListTree图标加“知识列表”文字，保留原展开状态/逻辑，补动态title及aria-controls关联；AppShell.tsx的全局导航title改为明确的展开/收起操作。同步HTML规范，没有新增依赖或调整业务数据。`npm.cmd run typecheck`通过。按用户规范未运行单元测试；本次为低影响图标/文案调整，不重复无关套件。当前旧验收进程11940仍占用程序，已请用户保存关闭，桌面更新待其关闭后完成，未声称新图标已在运行窗口显示。

## 2026-10-29：正文标题去重与控件比例微调

用户提供三张截图，选择保留顶部标题、移除正文同名标题的最小方案。实际修改 Settings.tsx、Knowledge.tsx、Graph.tsx、benchmark/BenchmarkPage.tsx：h1保留读屏语义并视觉隐藏；设置/评测移除纯标题行，知识操作/图谱切换保留。styles/features.css统一设置/图谱分段控件的字高、选项高度和内距；图谱数据下拉移除padding-left:0，恢复12px左留白及32px箭头区，34px控件高度。无数据库/模型逻辑/依赖变化，未重做业务容器或对话页。

扩展原tests/window-drag.spec.ts检查顶部标题/隐藏h1、34px选择器和内距、模式标签切换及各页1366/1600/1920截图；旧物理拖动专项目标改为可见顶部标题。同步HTML规范。`npm.cmd run tauri -- build --debug --no-bundle`通过，包含类型检查/Vite，Rust34.52秒；保留既有JS大块提示。`git diff --check`通过（既有换行提示）。未运行单元测试。

首次原生界面检查page-heading-controls等待透明且pointer-events:none的radio输入，60秒超时；改为点击实际label并断言选中，不修改产品交互。最终 `npx.cmd playwright test --project=desktop window-drag.spec.ts --grep "native dialogs" --output=../../artifacts/page-heading-controls-final`：1项通过（10.0秒，场景6.5秒），各页面标题/控件布局、三尺寸截图、图谱模式切换及既有窗口操作/弹窗检查通过。查看实际1366深色图谱和设置截图，正文无重复页面标题、控件图文分离且留白均匀。未重跑受桌面环境阻挡的物理拖动检查。

用户已保存关闭旧窗口。使用原独立验收数据启动最新版进程11940；没有自动运行模型，未提交/推送/发布。Benchmark7等待验收状态不变。

## 2026-10-29：全页面窗口标题栏一致性修复

用户反馈非对话页无法拖动、窗口操作嵌在业务区域。本轮仅修改 `apps/desktop/src/styles/layout.css` 的共享布局：移除901px以上非对话页绝对定位/隐藏标题/压缩拖动空白的特例，所有页沿用对话页48px基础标题栏，增加flex-shrink:0。保留现有WindowControls拖动及交互控件排除规则，弹窗仍下移并独立关闭，全局按钮始终可用。同步HTML规范，扩展原 `tests/window-drag.spec.ts` 检查各页空白拖动区域及标题栏与内容区不重叠。无数据库、模型逻辑、依赖或权限变动。

读取总纲/开发/实施/HTML规范，复用ponytail与ui-ux-pro-max；定向搜索dragging movements规则只适用于内容拖动替代操作，未据此引入窗口拖动新控件。`git rev-parse --show-toplevel`确认独立仓库。用户已保存关闭旧窗口。`npm.cmd run typecheck`通过；`npm.cmd run tauri -- build --debug --no-bundle`通过，Rust1分35秒，主CSS index-Dz21PZzE.css 61.00kB，保留既有大JS块提示。没有运行单元测试。

原生命令 `npx.cmd playwright test --project=desktop window-drag.spec.ts --output=../../artifacts/window-titlebar-unified`：弹窗专项通过5.1秒，物理拖动专项失败6.1秒，原因是Win32命中另一个msedge进程11384而非被测2728，安全保护阻止继续操作，不作为真实拖动通过。随后尝试拦截拖动IPC的辅助断言未观察到调用，移除该未验证的模拟方法，继续沿用独立物理专项。该中间组合window-titlebar-unified-verified失败，不计为通过。

最终 `... --project=desktop window-drag.spec.ts --grep "native dialogs" --output=../../artifacts/window-titlebar-unified-layout`：1项通过（7.6秒，场景5.1秒）。核对非对话页正常状态48px标题栏/足够空白/与正文分离；三尺寸浅深弹窗截图、所有页全局按钮命中、真实最大化/还原/最小化及关闭、焦点返回与Tab均通过。物理鼠标拖动仍需用户验收，不以布局断言替代。检查脚本的一次临时TS窗口类型声明错误已修正，类型检查随后通过；该模拟段最终删除。

复用原独立验收数据启动最新版进程31468，show-current.ps1返回visible=true、foregroundGranted=false，窗口已显示但Windows未授予前台切换，可从任务栏进入验收。没有自动运行评测，未提交/推送/发布。Benchmark7仍待验收，本次不进入8。`git diff --check`通过，仅既有换行提示。

## 2026-10-29：Benchmark 7——导出与独立重新评分

用户验收6.5并授权下一步，完成 [切片7](benchmark-report-slice7.md)；实际文件清单见该文，[导出协议](benchmark-export-schema.md)记录格式及边界。复用现有表、评分器和共享弹窗，新增限定范围的导出及重新评分命令。原报告显式读取原评分，新轮次事务追加，取消整体回滚；所有尝试、原始输出、来源与计量可导出。无新迁移或依赖，未提交、推送、发布，保留既有工作区变动。

实际命令：Cargo位于apps/desktop/src-tauri，npm/Playwright位于apps/desktop。必要 `cargo test report_transfer --lib --offline -- --nocapture` 最终2项通过（5.63秒），其余79项未运行；此前 `cargo test report_ --lib --offline -- --nocapture` 的原2项报告检查通过。早期新增检查发现用途字段实际位于conversations、夹具标准答案应为95、终态fixture更新无效，以及预检查事件副本中的认证字段漏清理，已分别修正并验证。检查覆盖归属、源指纹、追加/取消/重复轮次、原数据不变、全部尝试、JSON校验、CSV转义/公式保护、旧文件保护及临时文件清理。`cargo fmt --all`、`cargo check --lib --offline` 最终通过（后者3.47秒）；一次从desktop目录误运行fmt因无Cargo.toml失败，随后在正确目录完成。

`npx.cmd playwright test --project=edge benchmark-result-drawer.spec.ts personalized.spec.ts --output=../../artifacts/benchmark-7-browser-final`：8项通过（17.2秒），含保存取消/错误恢复、确认/取消评分、切页清理、三尺寸浅深与既有个人向导。`npx.cmd playwright test --project=desktop standard-benchmark.spec.ts --output=../../artifacts/benchmark-7-native-pwsh7`：1项通过（38.5秒），受控HTTP实际8次回答经SQLite保存后追加8条独立评分，模型请求仍8次，原统计不变，实际重复轮次正确；JSON/CSV系统保存与取消、原图谱往返/停止恢复均通过。此前native-visible组合中的300项分页专项通过（10.3秒），标准专项因旧PowerShell无法取得保存控件失败，未将失败组合记录为通过。改用本机现有PowerShell7后通过，未安装软件。CSV标准库解析37列、16条评分明细，所有行列数一致。原生检查后仅补切页清理与评分来源元数据，前者由最终浏览器检查覆盖，全部源码由最终构建覆盖。

最后 `npm.cmd run tauri -- build --debug --no-bundle` 成功，含类型检查/Vite，Rust26.49秒；资源index-Bccl34JH.js 657.66kB、index-DE8nMJ2F.css 61.72kB、GraphViewport-XUr_07_H.js 941.05kB，保留既有大块提示。此前构建会话在中断后不可读取，因此重新取得明确成功输出。查看实际深色1920评分历史及原生1366截图，固定操作可见；不把浏览器夹具成绩当模型能力。

用户指出窗口未弹出后，先恢复独立受控排查窗口并实际检查保存对话框。最终运行launch-metrics.cjs、show-current.ps1和prepare-transfer.cjs：最新版进程3460，visible=true、foregroundGranted=true，复用原phase-c-acceptance独立数据库与用户编辑，已打开“导出评测报告”。准备前后统计一致，无自动模型请求或文件保存，截图artifacts/phase-c-acceptance/transfer-ready.png。正式用户数据库保持原样。

Docker未安装，真实容器重评分未测；真实付费模型、完整本地模型新生成、跨硬件长时、断电/磁盘故障、系统150%及读屏未执行。全量长答案导出的内存压力仍待验收。切片7完成后暂停，下一步8真实模型验收等待用户确认。

## 2026-10-28：Benchmark 6.5——页面收尾与体验检查

6.4 用户验收后明确授权“做”。按 [6.5 文件/验收方案](benchmark-report-slice6-5.md) 完成前端增量，交付后停在 6.5 验收；7 导出/复算与 8 真实模型验收不自动启动。复用 HTML 规范、语义 Tokens、SelectMenu、FileName、现有报告/详情/调度。没有数据库、评分器、模型调用、权限、依赖或版本号修改，未提交/推送/发布，保留此前用户及阶段改动。

### 实际修改与检查依据

- PagedResults：原 layout effect 会在每个模型首批结果到达时滚动，新逻辑跳过默认第一页的首次揭示及同页后台刷新，保留明确翻页和单题定位。数据未到时不再显示虚假的总页数/0 匹配；已知页码、读取中/失败/重试仍可见，旧查询结果不混入。
- PersonalizedReport / PersonalizedWorkbench：运行操作固定在当前报告滚动区顶部，以 ResizeObserver 实测操作栏高度供分页定位与焦点留白，卸载清理。个人结果步骤的旧步骤条回归文档流，避免遮挡。原启动、停止、刷新、导出方法和确认要求保持。
- benchmark.css：当前模型分页固定在运行操作下方；长名称/来源正文按需断行，计数保留稳定宽度，表格区域独立横向滚动及内嵌焦点。初次截图复核发现半透明固定栏会透出底下题目，已将原 surface 叠在 background 上形成遮挡，无新颜色/动画系统。
- PerformanceDetails 及总比较表：增加区域名称和键盘焦点，保留原 caption/字段与统计口径。FileName 在三尺寸确实截断主体、保留 .py；外观菜单及 Esc 正常，因此这两个共享组件未改。
- 新增 tests/benchmark-report-usability.spec.ts，扩展 benchmark-result-drawer.spec.ts / personalized.spec.ts；playwright.config.ts 登记原生专项。同步阶段顺序、实施/测试计划和 HTML 规范。完整文件清单与原始性能样本位置见 6.5 说明。

### 实际命令与结果

npm/Playwright 位于 apps/desktop，文档/仓库检查位于仓库根；UI 检查使用已批准的 require_escalated、独立 SQLite 与 WebView Profile。没有运行单元测试。

- 确认 `git rev-parse --show-toplevel`；读取 AGENTS.md、development.md、开发/实施规范、原阶段计划与相关组件/接口。沿用 ponytail 最小增量与 ui-ux-pro-max；`py -3 -X utf8 .../ui-ux-pro-max/scripts/search.py "focus not obscured" --domain ux -n 3` 匹配焦点避免遮挡及 scroll-padding 指导，区分 AA 最低和 AAA 增强要求，未生成新设计体系。
- `npm.cmd run typecheck` 首次通过。`... playwright test --project=edge benchmark-result-drawer.spec.ts --grep 'report loading' --output=../../artifacts/benchmark-6-5-browser-first`：新增加载/长内容/三尺寸浅深/固定栏/菜单与键盘专项 1 项通过（7.8 秒）。
- `npm.cmd run tauri -- build --debug --no-bundle` 三轮依次通过：首轮 Rust 32.81 秒；补错误加载计数后 25.32 秒；固定栏遮挡最终版 31.68 秒。最终含类型检查/Vite通过，资源 index-8xoEkA12.js 649.25 kB、index-D4JwgpA2.css 61.32 kB、GraphViewport 941.05 kB；保留既有大块提示，没有修改打包架构或生成安装包。构建前无 CortexOS 进程，未强制关闭用户窗口。
- `... --project=desktop benchmark-report-usability.spec.ts --output=../../artifacts/benchmark-6-5-native-first`：900 项实际 SQLite/读取/滚动停止专项通过（16.9 秒）；服务端为受控 HTTP 模型端点，不是真实模型。
- 首次同时运行上述原生与浏览器组合，浏览器复用了原生进程拥有的 1420 开发服务；原生结束关闭服务，导致浏览器后四项 ERR_CONNECTION_REFUSED。组合 `... --project=edge benchmark-result-drawer.spec.ts personalized.spec.ts ui-usability.spec.ts --output=../../artifacts/benchmark-6-5-browser`：5 项通过，4 项环境连接失败（21.8 秒）；已通过导航/个人重载/晚到请求/原详情/新长内容和 1366 向导。没有将失败写成产品成功。后续全部 Playwright 组顺序运行，不再共用会提前退出的服务。
- 一次补跑命令的无空格含管道 grep 在 Windows cmd 参数转发中被拆分，未运行测试；改为直接指定两个文件。`... --project=edge personalized.spec.ts ui-usability.spec.ts --output=../../artifacts/benchmark-6-5-browser-recovery`：5 项通过（14.4 秒），含三尺寸长文件名/向导、菜单和共享面板。连同前次通过的四个报告场景，共 9 个相关浏览器场景有通过证据。
- 最终 `... --project=desktop benchmark-report-usability.spec.ts benchmark-report-pages.spec.ts standard-benchmark.spec.ts --output=../../artifacts/benchmark-6-5-native-final`：3 项通过（总约 1 分钟；12.7/10.6/34.4 秒）。覆盖最终固定栏遮挡、三尺寸浅深、焦点避让、900 项性能、实际停止、300 项分页/单题/筛选/返回，及已有双模型实际执行记录、原始采样、图谱往返、停止/恢复。未重复无关知识/相机/模型全套检查。
- `git diff --check` 通过，仅既有 CRLF 提示；新增文件另查尾空白。当前大量旧未提交变动未撤销，未读取根目录令牌。

### 性能和可见窗口

900 项未运行计划（3×60×5），只渲染 60 个结果列表项（每模型20）。WebView2 内顺序各 5 次，未清缓存：概览中位 152.5 ms、48,891 JSON UTF-8 字节；第8页中位10.2 ms、11,353字节；未运行单题中位70.1 ms、798字节。翻页至新行出现中位68.7 ms（5次，含自动化往返）。完整 min/max/样本与口径见 6.5 及 performance.json。不是纯 SQL、纯帧时间或模型速度，也不代表 900 条完成长回答的负载。读取期间请求0、计划统计不变；随后单独让1条受控响应等待，滚动到最后模型仍能停止，余899项未发送。

已查看原生1366浅色顶部、1920深色滚动停止及最终遮挡修复截图；三尺寸浅深均实际生成并检查溢出/按钮位置，但未逐张人工复核。另查看实际验收窗口截图，清楚标记未运行与无输出，没有伪造答案。

执行现有 launch-metrics.cjs 和基于原只读准备脚本的 prepare-usability.cjs，复用原独立数据/用户编辑，进程25552，stage=benchmark-6.5。仍打开原批次 rag-001，重载保留查看条件，前后统计相等，没有创建新计划或自动运行，也没有调用真实 Ollama、下载模型或付费云端。show-current.ps1 恢复窗口 visible=true，Windows foregroundGranted=false；不抢前台，保留窗口/任务栏供用户切换。截图 artifacts/phase-c-acceptance/usability-ready.png。

### 限制与停止点

系统150%和真实读屏本轮未执行，不以旧版本用户验收、内容视口或自动名称/焦点检查代替。人工检查表已写进6.5说明，保留用户系统设置。900条已完成长答案、跨硬件/长时真实评测、Docker及真实付费个人评测待后续环境验收。当前大块JS提示保留。6.5已交付待验收，下一步建议7导出与复算；本轮停止。

## 2026-10-28：Benchmark 6.4 用户验收完成

用户确认“验收完”。更新阶段顺序、实施计划和 6.4 交付说明为已验收；下一步为 6.5 页面收尾与体验检查。本次仅修改文档，没有修改代码、操作验收窗口或启动任务，未运行单元测试或构建。实际执行 `git rev-parse --show-toplevel`、读取上述文档、按原文精确更新状态并回读核对；既有测试/未测边界保持原记录。

## 2026-10-28：Benchmark 6.4——导航与返回定位

用户授权“做导航与返回定位”，随后明确已保存并关闭原验收窗口。本轮按 [阶段顺序](benchmark-phase1-remaining-plan.md) 完成 [6.4](benchmark-report-slice6-4.md)，停在本增量验收。标准/个人报告共享 URL 状态、只读概览与定位；刷新/前进后退恢复排序、模型、筛选、各模型页码和单题，实际执行图返回原题，独立图按真实执行 ID 定位。保留停止/继续、原文/评分/来源/采样和全局窗口操作。没有新依赖、表、迁移、题库或评分修改；不推进 6.5、导出/复算或发布。

实际文件完整列于 6.4 交付文档。新增 reportNavigation.ts / useBatchOverview.ts / BenchmarkLink.tsx；接入 BenchmarkPage、StandardBenchmarks、PersonalizedWorkbench、PersonalizedReport、PagedResults、ResultDrawer、navigation、App、ExecutionTrace；AppDialog 增加可选焦点返回解析函数。后端 report_read.rs 复用数据库筛选 SQL，新增 get_benchmark_result_location；lib/build/capability 仅登记这一限定范围的业务读命令；standard_tests 扩展已有两个数据库专项；三个 UI 专项和阶段/测试/HTML 文档同步。工作区此前已有大量未提交改动，未撤销或提交推送。

逻辑审阅：定位先校验工作区/用途/批次，再验证 item 与实际 execution_run_id 的关联；结果页和定位共用筛选投影，不能拿原题配另一执行 ID。直接链接按数据库查目标页，避免读取全量回答。概览/列表/详情均以当前查询身份防旧响应；批次读取不会改写新的导航地址。个人准入沿用原 gate；标准入口不需云端密钥。链接操作只读，不批准、发送或重新评分。跨工作区返回链接、外站链接、重复/无效参数和筛选不匹配均提供可恢复状态。没有 OS 深链接或新通用文件/数据库能力。

### 实际命令与结果

npm/Playwright 命令目录 apps/desktop，Cargo 目录 apps/desktop/src-tauri；原生/浏览器进程检查使用受批准的 require_escalated 和独立数据库/Profile。

- 复核 AGENTS.md、development.md、开发/实施规范、当前阶段计划、已有报告和执行图；复用 ponytail / ui-ux-pro-max 的最小增量和焦点可达规则。确认仓库根；未读取令牌或修改凭据。
- `cargo fmt --all`、`cargo check --lib --offline` 通过。只运行必要 `cargo test report_ --lib --offline -- --nocapture`：2 项通过（0.85 秒），其余 77 项未运行。扩展检查覆盖跨工作区/用途拒绝、错配题目/执行、缺失目标/不合法页大小、按实际执行反查、模型独立页码及筛选不匹配。新增只读数据库命令存在安全边界，因此保留这两项专项，没有运行无关单元套件。
- 类型检查早期发现未使用导入及定位回调放置错误，修正后通过。新增 UI 检查又因局部 `location` 变量遮蔽浏览器对象而使 `npm.cmd run typecheck` 和构建前置检查失败；仅重命名检查中的变量，随后最终 `npm.cmd run typecheck` 通过。
- 最终 `npm.cmd run tauri -- build --debug --no-bundle` 通过，Rust 29.64 秒；包含类型检查和 Vite。主块 index-CLxB3fjq.js 648.39 kB，样式 index-BnHMZ8fH.css，GraphViewport 941.05 kB，保留既有大块提示。没有生成或发布安装包。
- 初次浏览器 fixture 在自身初始化时引用 rows，导致 IPC stub 未安装；修正受控输入后 `... --project=edge benchmark-result-drawer.spec.ts --output=../../artifacts/benchmark-6-4-browser-smoke`：原单题检查通过（4.7 秒）。这不是产品运行错误或真实模型成绩。
- 新导航浏览器组合先后因检查写错“返回结果列表”“重置查看条件”名称而失败；修正名称，不改产品标签。`... --project=edge benchmark-result-drawer.spec.ts personalized.spec.ts --output=../../artifacts/benchmark-6-4-browser-verified`：5 项通过，1 项上述名称失败（总 26.2 秒）。最终仅重跑该项 `... --project=edge benchmark-result-drawer.spec.ts --grep 'report links' --output=../../artifacts/benchmark-6-4-browser-links`：1 项通过（总 6.7 秒）。共 6 个相关场景分别有通过证据，未将失败组合写成全通过。覆盖翻页历史、重载、筛选/单题/复制/打开内部链接、错误配对不读详情、个人报告重载、晚到批次隔离、长原文/错误/焦点和三尺寸原个人向导。
- `... --project=desktop benchmark-report-pages.spec.ts standard-benchmark.spec.ts --output=../../artifacts/benchmark-6-4-native`：300 项实际 SQLite 分页/末题定位/筛选刷新/单题焦点/三尺寸浅深截图通过（11.9 秒）；标准批次新检查误把返回链接写成 button，等待 120 秒超时。修正为 link 并给检查设置 10 秒定位超时，产品保持原链接语义。
- `... --project=desktop standard-benchmark.spec.ts ui-usability-native.spec.ts window-drag.spec.ts --grep 'standard batches|native long records|native dialogs' --output=../../artifacts/benchmark-6-4-native-verified`：3 项通过（45.1 秒；28.7/6.3/6.4 秒）。受控双模型真实 HTTP→SQLite→执行图，图谱重载后返回原筛选/模型/轮次/题目，独立执行图反查结果，返回过程请求仍为 8 次；原停止/继续只发送剩余项。知识历史、编辑/来源菜单、弹窗期间全局窗口按钮回归通过。没有运行物理鼠标拖动用例。
- 查看真实 1366 浅色抽屉顶部、1920 深色滚动后及本机验收窗口截图；其他尺寸截图由原生专项实际生成并检查无横向溢出，未逐张人工复核。实际视口检查不替代系统 150% 或读屏。
- `git diff --check` 首次发现 ExecutionTrace 空行尾空白，清理后通过，仅保留原 CRLF 提示。最终类型检查通过；不重复已通过的全套或无关检查。

### 验收窗口与停止点

运行已有 `node artifacts/phase-c-acceptance/launch-metrics.cjs`，复用原独立数据库和 WebView Profile，启动进程 18644，调试端口仅供本机检查。新增本机只读准备脚本 `prepare-navigation.cjs`：打开原批次 68f2568b-1bfe-46cb-98cd-40be4a50f5f6 的 rag-001，带模型/轮次/能力/排序/单题定位，刷新后条件和题目一致，前后统计相等。没有重建数据、生成问题、批准运行或调用模型，也没有覆盖生产库或用户编辑。manifest 更新为 benchmark-6.4，实际图 artifacts/phase-c-acceptance/navigation-ready.png 已查看。

运行 `show-current.ps1 -AppProcessId 18644`，Win32 visible=true，foregroundGranted=false；Windows 未授予抢前台，保留可见窗口和任务栏入口供用户切换，没有绕过限制。原两题仍未运行，便于先验收链接/刷新；执行图往返另有上述受控原生证据，不伪称为真实 Qwen 回答。

当前风险/未测：内部链接依赖当前本地数据库，不能作为跨机器报告分享；个人历史恢复仍受原连接准入限制。真实个人云端、Docker、真实 Qwen 本轮生成、多硬件/长时运行、读屏和新增评测页系统 150% 均未执行，原因是本轮只读导航范围或环境未就绪。此前用户对其他页面 150% 的验收不代替本次新增页面验证。新建/生成向导的未保存选择不在报告链接中序列化。下一步建议 6.5 页面收尾，本轮在 6.4 验收点停止。

## 2026-10-28：Benchmark 6.3——单题详情面板

用户确认回到主线继续，按原计划实施 [6.3](benchmark-report-slice6-3.md)，完成后停在本增量验收。复用 6.1 单题只读接口、6.2 分页/筛选以及现有共享弹窗、Markdown 和采样组件；新增右侧按需详情、固定标题/刷新/关闭、底部返回/页内切题、完整原文、评分/题目要求/固定资料/请求/采样分层查看。没有修改后端业务、迁移、模型调用、题库或评分，不进入 6.4–6.5、导出/复算或发布。

实际文件：新增 apps/desktop/src/benchmark/ResultDrawer.tsx；修改 PagedResults.tsx、PersonalizedReport.tsx、benchmark.css、components/AppDialog.tsx、styles/layout.css；新增 tests/benchmark-result-drawer.spec.ts，适配 benchmark-report-pages.spec.ts / standard-benchmark.spec.ts 和 playwright.config.ts。完整功能与文件清单见 6.3 说明，同步原计划/界面规范/测试计划。当前工作区有大量此前阶段修改，未撤销、提交、推送或发布。

逻辑检查：响应与工作区/用途/批次/题目身份绑定，切题立即隐藏旧内容且忽略晚到结果；正文 DOM 随题目重置，采样状态不串题。当前题已有保存评分即停止轮询，关闭/切题清理旧请求与计时器。未运行不显示模型输出/执行图入口；运行无文字、读取错误、执行错误与不支持保持不同状态。固定资料保留编号与 1 起始行号，不称其为生产检索命中。请求查看区区分保存的云端 wire 与本地快照，保留实际事件与所有评分记录。

实际执行（npm/Playwright 位于 apps/desktop）：

- 阅读 development.md、开发/实施规范、阶段计划、HTML UI 规范与现有接口/数据序列化代码。沿用 ponytail 与 ui-ux-pro-max 定向规则；`py -3 .../ui-ux-pro-max/scripts/search.py "drawer close return focus" --domain ux` 命中焦点可见/避免遮挡的相关项，未生成新设计体系。
- `npm.cmd run typecheck` 通过。用户明确保存并关闭验收窗口后，`npm.cmd run tauri -- build --debug --no-bundle` 多轮构建通过，最终 28.19 秒；内含类型检查/Vite。最终资源 index-MAQF9par.js / index-BnHMZ8fH.css，主包约 638 kB、GraphViewport 约 941 kB，原大块提示保留。
- 初次原生 `npx.cmd playwright test --project=desktop benchmark-report-pages.spec.ts standard-benchmark.spec.ts --output=../../artifacts/benchmark-6-3-native`：标准批量/计量/停止恢复通过（29.1 秒）；新固定标题坐标检查在入场动画途中测量失败。为几何测量显式设置 reducedMotion=reduce，不修改产品动画。
- 初次浏览器新专项在 Esc 关闭处失败（6.6 秒），晚到响应、完整原文、重试/错误已先通过。检查确认 React StrictMode 重放 effect 会移除仍打开 dialog 的内容限制注册，旧 openDialog 仅凭 open 提前返回。共享入口现仅跳过“已打开且已注册”的实例；重放时重新注册，恢复正文 inert/Esc/返回焦点。
- `npx.cmd playwright test --project=edge benchmark-result-drawer.spec.ts ui-usability.spec.ts --output=../../artifacts/benchmark-6-3-browser-final`：3 项通过（8.4 秒），覆盖上述修正、新详情及既有共享面板/菜单。UI 合成输入明确标记，不是模型成绩。
- 第一次最终组合 `... --project=edge --project=desktop benchmark-result-drawer.spec.ts benchmark-report-pages.spec.ts standard-benchmark.spec.ts ui-usability-native.spec.ts window-drag.spec.ts --grep "drawer ignores|native report pages|standard batches|native long records|native dialogs" --output=../../artifacts/benchmark-6-3-final`：5 项通过（53.2 秒）。包含真实 SQLite 的 300 项计划/固定资料，受控双模型、已保存评分/原始采样/停止恢复，知识历史/草稿/来源以及弹窗期间全局窗口操作。
- 截图复核仍发现旧 side-drawer 同权重规则将新面板覆盖为 430px，且非聊天页弹窗顶栏与原页面标题重叠。已提高专用面板宽度规则优先级，弹窗打开时隐藏被顶栏替代的页面 h1，关闭后恢复；未更改页面布局。截图检查增加 720px 精确宽度与等待读取完成，保留顶部/滚动后截图，不将加载画面当成内容还原通过。
- 最终 `npx.cmd playwright test --project=edge --project=desktop benchmark-result-drawer.spec.ts benchmark-report-pages.spec.ts window-drag.spec.ts --grep "drawer ignores|native report pages|native dialogs" --output=../../artifacts/benchmark-6-3-acceptance`：3 项通过（22.3 秒）。复核最终样式、晚到请求、Esc/焦点、真实资料、三尺寸浅深、固定操作区与全局窗口按钮；不重复无关检查。已查看最终 1366 浅色顶部、1920 深色滚动后原生图和实际验收窗口图。
- `git diff --check` 通过，保留原 CRLF 提示。按用户要求未运行单元测试；本轮无后端算法或数据库变更，不重复既有数据库/评分全套检查。

已运行本地验收启动脚本，复用原独立数据与用户编辑，主进程 27252。原验收库没有标准历史，首次准备两题计划因 Ollama 离线而停止，未写假模型身份。随后启动此前授权安装的本地 Ollama（仅 127.0.0.1:11434，进程 28108）；确认 loadedModels=0，只读取真实模型列表。根据实际 qwen3:4b-instruct 及其 digest/现有请求设置，在独立工作区“单题详情验收（未运行）”创建 math-001 / rag-001、单次两题的未运行计划，并打开 rag-001 详情。没有下载/加载模型或生成回答，没有覆盖生产数据或原工作区。已有应用窗口恢复显示，Win32 visible=true；验收图为 artifacts/phase-c-acceptance/detail-ready.png，窗口保留供测试。

当前限制：真实个人云端、Docker、读屏、新页面 150% 和跨硬件/长期任务未在本轮执行；物理鼠标拖动保留此前环境命中待核对，不由窗口按钮通过代替。下一步为 6.4 稳定内部链接/刷新/图谱返回定位，等待本增量验收后再实施。


## 2026-10-28：修复任务栏仍显示旧 Logo

用户反馈任务栏仍为旧图标。核查已生成的新 SVG/PNG/ICO 与 Windows 编译链发现：安装的 tauri-codegen 2.7.3 在 Windows 从应用图标资源 32512 读取默认窗口图标；tauri-build 2.7.3 的构建输出只监听配置/权限等文件，没有监听图标。旧 resource.lib 时间为 13:33，新 ICO 为 17:47，因此之前 18:03 的 EXE 仍链接旧图标资源。此前只检查应用内标识并声称桌面图标已应用的验证不充分，由本条更正。

本次产品代码只修改 apps/desktop/src-tauri/build.rs：增加 cargo:rerun-if-changed=icons，图标目录变化即重新编译原生资源。复用原 Tauri 图标链，无依赖、权限、数据库或界面变动。

实际验证：

- `npm.cmd run tauri -- build --debug --no-bundle` 通过（36.74 秒），包含类型检查/Vite；原有大块提示保留。构建输出已出现 `cargo:rerun-if-changed=icons`，resource.lib 更新到 18:32。
- 一次性本地 artifacts/icon-embedding/verify-icon.ps1 使用 LoadLibraryEx/FindResource 读取 EXE 的图标组和各帧，并逐字节与现有 icon.ico 比较；6 帧全部一致。没有加载执行待检 EXE，也没有以文件图标缓存替代资源读取。
- 已复用独立验收数据重开进程 3280，原页面就绪，未自动运行评测。首次启动因上轮 PowerShell 写入 JSON BOM 失败，已修正本地验收脚本的 BOM 读取和写入后正常启动；该脚本不属于产品功能。
- 实际窗口检查补齐 WM_GETICON 的大/小类型，保存并查看运行窗口图标 runtime-window-icon.png，已为知识节点 Logo。初次只查大图标返回空，是检查脚本覆盖不足，不当作产品无图标。
- 通过 Windows UI Automation 仅定位任务栏的 CortexOS 按钮，匹配 `CortexOS - 1 个运行窗口`，截取该按钮 55×60 像素图 cortex-taskbar.png；已查看，实际任务栏为新青绿/靛蓝三节点标识。证据位于 artifacts/icon-embedding/，未清除系统图标缓存或修改固定项。

按项目约定没有运行单元测试或无关业务检查；使用实际构建资源和真实任务栏验证本问题。窗口保留供用户继续验收，Benchmark 6.2 的停止点保持不变。


## 2026-10-28：Benchmark 6.2、Logo 02 与应用内弹窗标题栏

用户在 6.1 交付后授权继续原计划，暂选 Logo 02“知识节点”，随后明确全局窗口标题栏始终可见、可操作，弹窗下移并有自己的关闭入口。本轮交付 [6.2](benchmark-report-slice6-2.md)，停止在本增量验收；不进入 6.3、导出/复算或发布。无新依赖/权限/迁移，无实际模型生成、云端调用或下载，生产数据库未用于检查。

实现：共享概览保留全计划统计，比较表支持计划顺序/正确率/评分覆盖率/已记录速度排序，缺失放最后、同值按模型序号；逐题模型/状态/能力/难度/重复轮次筛选以数据库全部匹配为准，再分页，显示匹配与计划数量。切换筛选重置页码和展开项，旧请求不会渲染到新筛选下。冻结比较条件显示题库/模型标识和请求参数差异，不推测量化方式、实际用量或跨机器可比性。

原生 showModal 会把弹窗外的全局标题栏也设为不可交互，因此新增内容范围 AppDialog/openDialog；正文与侧栏 inert、标题栏保持可操作，弹窗 Portal 到 body 并预留顶部 76px。原 close/cancel、Esc、审核与未保存保护仍由原业务处理。长面板沿用固定标题/关闭/操作区，紧凑确认框补独立且随滚动保留的关闭按钮。Logo 按已选 PNG 方向整理透明 SVG，导航/欢迎页/favicon 共用 BrandMark；现有 Tauri CLI 转换原六个桌面图标，额外生成的移动端/商店文件已清理。

实际修改文件完整列表见 6.2 交付文档；下列为实际执行结果（npm/Playwright 在 apps/desktop，Cargo 在其 src-tauri，其他命令在仓库根）：

- 阅读项目总纲、开发/实施规范、当前计划及 HTML UI 规范，使用 ponytail / ui-ux-pro-max 的既有组件与最小共享修正。UX 定向检索 `py -3 .../ui-ux-pro-max/scripts/search.py "dialog keyboard focus close" --domain ux` 命中焦点可见与避免遮挡规则，没有生成新设计体系。
- `npm.cmd run tauri -- icon --help`、`npm.cmd run tauri -- icon public/cortex.svg --output src-tauri/icons` 成功。字体扫描提示 mstmc.ttf malformed，SVG 无字体，图标正常生成；已检查原方案及实际应用标识。
- `npm.cmd run typecheck`、`npm.cmd run build` 通过。`cargo fmt --all` 完成。初次 `cargo test benchmark::standard_tests::report_ --lib -- --nocapture` 匹配 0 项，不能视为专项通过；更正模块匹配后 `cargo test report_ --lib -- --nocapture`：2 项通过、77 项未运行，0.80 秒。涵盖六类能力、三档难度、轮次组合、跨页计数、零匹配、非法条件、用途隔离、完整/概览等价及正文不进列表；没有运行无关单元测试。
- `npm.cmd run tauri -- build --debug --no-bundle` 最终通过（29.11 秒）。包含类型与前端构建，资源为 index-DC8liaLT.js / index-BnSkaf_Y.css，GraphViewport 约 941 kB。原 Vite 大块提示保留，没有因此拆分无关模块。
- 原生初次 `... playwright test --project=desktop window-drag.spec.ts benchmark-report-pages.spec.ts ui-usability-native.spec.ts --output=../../artifacts/benchmark-6-2-ui`：长记录/知识历史/嵌套未保存保护/来源菜单通过（5.4 秒）；新筛选检查等待字段标签超时。加跟踪重查发现包裹 label 的文本包含选项，已为新 SelectMenu 显式设置可访问名称，后续实际筛选通过。
- `... --output=../../artifacts/benchmark-6-2-check`：300 项分页与筛选通过（5.4 秒）；双模型检查的表格定位误命中详情内的另一张表，修正为全计划比较表选择器。
- `... playwright test --project=desktop window-drag.spec.ts standard-benchmark.spec.ts --output=../../artifacts/benchmark-6-2-native-controls`：标准批量/缺失速度排序/同分顺序/模型筛选不改比较表/指标采样/停止恢复通过（26.3 秒）。实际回答均来自明确的受控服务，非真实模型成绩。物理拖动项仍未通过，原因如下。
- 物理鼠标拖动专项多次在实际发送前命中另一个 Edge 进程（26232），不是本次测试窗口；确认目标仍可见后，归属保护继续拒绝发送。保留失败记录在上述目录和 benchmark-6-2-diagnose，没有绕过归属检查，也没有把 IPC 调用或 DOM 位移当作实际拖动。此项留待人工验收。
- 将弹窗操作与物理拖动拆为独立原生检查。最终 `npx.cmd playwright test --project=desktop window-drag.spec.ts benchmark-report-pages.spec.ts ui-usability-native.spec.ts --grep "native dialogs|native report pages|native long records" --output=../../artifacts/benchmark-6-2-final`：3 项通过（19.8 秒）。实际核对 300 项数据库筛选/翻页/清除、全计划不变；三尺寸浅深弹窗与比较截图；五页弹窗的标题栏命中、正文 inert、Esc/焦点返回、Tab 范围，以及弹窗打开时真实最大化/还原、Win32 最小化状态和全局关闭。长记录/知识历史/草稿/资料导入回归保持通过。
- 已查看最终 1366 浅色紧凑弹窗、深色比较表，以及同布局长处理记录原生截图。截图是实际 WebView2 渲染，不用静态图充当页面，也未声称像素相似度或读屏通过。
- `git diff --check` 通过（保留工作区已有 CRLF 提示）。当前有大量此前阶段未提交修改，未撤销、提交、推送或发布。

已运行 `node artifacts/phase-c-acceptance/launch-metrics.cjs` 与 `verify-report.cjs`，复用原独立验收目录/用户保存的内容，进程 18076，页面就绪，未找到已有标准批次，保留新建计划入口；没有自动创建计划或发送模型请求。随后仅匹配该 PID 下标题为 CortexOS 的主窗口，`show-current.ps1` 恢复显示，Win32 visible=true、foregroundGranted=true。窗口保留供用户验收。

当前限制：物理鼠标拖动、新增页面的系统 150% 与真实读屏留待人工复核；Docker、完整真实 Core60、跨显卡/长期大计划沿用既有待办。下一步 6.3 单题详情面板需先验收本增量。


## 2026-10-28：修复窗口标题区域无法拖动

用户报告窗口无法拖动。检查发现非聊天页顶栏为右侧收缩布局，原 WindowDragRegion 只剩窄空白；页面标题和大部分标题行留白未绑定拖动。复用现有原生 startDragging/toggleMaximize 与既有权限，在 AppShell 的 workspace-stage 对顶栏/页标题区域委托鼠标事件；WindowControls 共享同一处理函数，避免重复触发。明确跳过按钮、链接、标签、输入/选择/可编辑控件及其交互角色；正文、图谱和分屏事件保持原逻辑。只为标题加不选中文本和默认指针，不改变布局位置或权限。

实际修改：apps/desktop/src/components/AppShell.tsx、WindowControls.tsx、src/styles/layout.css；新增 tests/window-drag.spec.ts / window-drag.ps1，在 playwright.config.ts 注册。同步 HTML 界面规范与本记录。测试辅助脚本只对自身独立窗口短暂置前、确认鼠标命中归属后输入，在 finally 释放鼠标、移除临时置前并恢复光标；使用线程级 DPI 坐标转换，不改变系统缩放。

实际验证（命令位于 apps/desktop）：

- 修改前旧程序执行 `npx.cmd playwright test window-drag.spec.ts --project=desktop --output ../../artifacts/window-drag-before`，评测页标题的真实窗口水平位移为 0，断言失败，复现问题。测试通过 Win32 原生鼠标输入与 GetWindowRect 核对位置，没有以 DOM 事件或 IPC 成功冒充实际移动。
- `npm.cmd run typecheck` 通过；`npm.cmd run tauri -- build --debug --no-bundle` 通过。Vite 原有主包/图谱大块提示保留，资源为 index-BWmboGBM.js / index-lsdRfeFC.css。
- 修复后 `npx.cmd playwright test window-drag.spec.ts --project=desktop --output ../../artifacts/window-drag-fixed`：1 项通过（14.7 秒）。五页标题分别触发真实窗口水平位移 >60、垂直位移 >10 物理像素，窗口尺寸不变；聊天空白拖动区也产生实际位移。另核对标题双击最大化、按钮还原、最小化及原生状态、主题下拉和关闭按钮。已查看实际 WebView2 截图 window-titlebar.png。

已用最终构建重开原独立验收数据和 WebView 配置，进程 17288，保留用户保存的编辑；入口就绪并保留窗口，没有开始评测。恢复主窗口显示只匹配本次主窗口，未操作输入法窗口。

按用户约定没有运行无关单元测试、模型请求或整套业务检查，没有修改数据库、模型或 Benchmark 计划。Benchmark 6.1 仍等待验收，不因此进入 6.2。

## 2026-10-28：Benchmark 6.1——完整计划与报告按需读取

用户先要求当前阶段完整步骤计划，再继续下一步。已给出 [6–8 顺序及停止点](benchmark-phase1-remaining-plan.md)，本轮交付 [6.1 报告按需读取/分页](benchmark-report-slice6-1.md)，停在本增量验收。使用已有评分、计量、状态和 UI Tokens，未新增依赖或迁移；schema 10、旧题库/评分及完整接口保持兼容。

实际执行（npm/Playwright 位于 apps/desktop；Cargo 位于 apps/desktop/src-tauri）：

- 阅读 development.md、开发规范、实施计划及 HTML UI 规范；按 ponytail / ui-ux-pro-max 复用现有实现。本地分页设计规则检索未命中，按既有组件与实际截图实现，没有生成新设计体系。
- `cargo check --lib --offline`、`npm.cmd run typecheck` 通过。代码逻辑审阅发现数据库为互斥连接，已限制概览查询持锁区间，并在单题读取后释放锁再调用完整读取，避免递归持锁。
- `cargo test --lib report_paging --offline` 初次因测试误用 list_conversations 方法名编译失败，改为现有 conversations 后，1 项通过（77 项未运行，0.72 秒）。核对统计/性能等价、20 万字符回答不进入概览/列表、页码与数量限制、模型/工作区/用途拒绝、未运行详情和聊天隔离；没有运行无关全套单元测试。
- `npm.cmd run tauri -- build --debug --no-bundle` 最终通过；内含类型与前端构建。最终 main 约 624 kB、GraphViewport 约 941 kB，Vite 既有大块提示保留；前端资源 index-CI3_D-6h.js / index--FCu5Lod.css。
- `npx.cmd playwright test personalized.spec.ts --project=edge --output ../../artifacts/benchmark-report-browser` 的三项分别完成，但受限进程未正常结束清理；已结束该旧命令的专用进程。改在授权环境复用开发服务，`... --output ../../artifacts/benchmark-report-browser-final` 正常退出，3 项通过（5.3 秒），覆盖既有个人评测三尺寸流程。
- 原生 `benchmark-report-pages.spec.ts / standard-benchmark.spec.ts` 初次运行，标准批量/计量/停止恢复专项通过（23.6 秒）；分页的接口和单题展开已通过，脚本尝试改写 WebView2 内置 invoke 统计调用数无效，断言失败。已移除无效拦截，改用真实返回、实际展开内容及组件 effect 逻辑审阅；未把失效的调用计数当作通过证据。
- `... playwright test benchmark-report-pages.spec.ts --project=desktop --output ../../artifacts/benchmark-report-pages-final`：1 项通过（4.6 秒）。最后补充长问题截断省略号，并按 UTF-8 字节计量概览样本，重新构建后执行 `npx.cmd playwright test benchmark-report-pages.spec.ts standard-benchmark.spec.ts --project=desktop --output ../../artifacts/benchmark-report-acceptance`：2 项通过（30.5 秒）。

最终原生证据：300 个未运行逻辑项，15 页 × 20 项、末项 ordinal=299 / 重复第 5 次；该样本概览小于 100000 UTF-8 字节。前后翻页、展开/收起、三尺寸浅深截图正常，模型答题请求为 0。另一个受控标准批量专项继续实际检查既有终帧计量、原始采样、图谱入口、取消后不重发及恢复剩余项，并核对概览与完整接口的统计/速度/GPU/CPU峰值一致；它的回答由受控服务产生，不是实际模型成绩。

已查看 artifacts/benchmark-report-pages-final 的浅深分页截图；最终截图与检查数据在 artifacts/benchmark-report-acceptance。个人新分页完整原生链路、全完成大计划读取耗时、排序/筛选、右侧详情、稳定页码链接及外观菜单复核按后续增量继续；150%/读屏、真实 Docker 和完整 Smoke 验收保持独立待办。没有发起真实模型问答、云端计费、模型下载或全量业务重验。

已执行 artifacts/phase-c-acceptance/launch-metrics.cjs 与 verify-report.cjs，复用原独立验收目录和 WebView 设置打开最终程序，保留用户刚保存的编辑；没有重拷生产库或自动开始评测。进程 16768；当前工作区无标准报告历史，显示已就绪的新建标准评测页。实际截图 report-ready.png；主窗口可见性由 Win32 另行核对，窗口前台请求遵循系统限制。

`git rev-parse --show-toplevel` 确认为 D:/vibecoding/CortexOS，`git diff --check` 检查差异；未读取 token、未提交或推送已有工作树。完成 6.1 后等待验收，下一增量为 6.2 比较排序与筛选。

## 2026-10-28：用户确认 Benchmark 切片 5 验收

用户明确反馈“验收完毕”，已同步指标交付说明及两份实施计划。下一步为原切片 6 的报告比较、筛选分页、大列表和单题详情交互；Docker、其他 GPU、读屏及完整 Smoke 等独立待办保持原状态。

本次只更新验收状态，没有修改应用代码、启动评测或重新运行单元测试/构建。实际执行 `git rev-parse --show-toplevel` 核对当前独立仓库，并用 `git diff --check` 检查文档差异。

## 2026-10-27：Benchmark 切片 5——计时、Token 和资源

用户验收标准批量链路并授权下一步，按原计划切片 5 实施。文件、指标公式、隔离、采样范围和风险见 [指标交付](benchmark-metrics-slice5.md)。复用 schema 10 和不可变采样表，无迁移；题库、评分器版本、生产知识及聊天配置保持原样。

实际命令（npm/Playwright 在 apps/desktop；Cargo 在 apps/desktop/src-tauri）：

- `cargo check --lib --offline` 初次因启用既有 Tokio 的进程读取功能而缺 `signal-hook-registry`，未通过；普通联网重试遇到 Windows TLS 凭据错误。获自动授权后 `cargo check --lib` 下载锁定间接依赖并执行编译，发现 rusqlite 不接受 u64 的采样时间，改用检查转换的 i64 后 `cargo check --lib --offline` 通过。没有系统软件安装。
- `cargo test --lib telemetry --offline` 最终 3 项通过（74 项未运行），覆盖缺失/零、配对速度、分位数、溢出、GPU 不支持字段、工作区隔离、追加约束、取消和重启保留评分。没有运行无关全套单元测试。
- `npm.cmd run typecheck`、`npm.cmd run build` 通过；`npm.cmd run tauri -- build --debug --no-bundle` 在最终代码上通过。最终资源为 `index-_QZFzzf6.js` / `index-5uHxzl7B.css`；main 约 620 kB、GraphViewport 约 941 kB，既有大块提示保留。
- `npx.cmd playwright test personalized.spec.ts --project=edge --output ../../artifacts/benchmark-metrics-browser`：3 项通过，三尺寸既有个人评测界面兼容。
- 原生受控 `standard-benchmark.spec.ts --project=desktop` 初次计量/取消恢复通过。扩展浅深主题和采样查看后，先后遇到测试的主题选项定位歧义、错误选中处理事件而非采样原文；分别限定 listbox 和本题计量区域修正。现有外观菜单在鼠标选择后观察到仍展开，截图用 Esc 收起，未借机修改共享菜单。
- 显式设置 `CORTEXOS_BENCHMARK_METRICS_REAL=1` 后运行 `benchmark-metrics-real.spec.ts --project=benchmark-metrics-real`：首次脚本请求 32 Token 被既有 64 最低预算拒绝，未生成；改为 64 后完成一个实际模型 × 一题 × 两次、保存并核对原文/计时/终帧/资源。截图阶段因脚本创建工作区后未刷新状态失败；修正为重载，并用 `CORTEXOS_BENCHMARK_METRICS_SAVED` 指向该存档目录，仅重开验证，不重发问题。
- `... playwright test standard-benchmark.spec.ts chat.spec.ts benchmark-metrics-real.spec.ts --project=desktop --project=benchmark-metrics-real --output ../../artifacts/benchmark-metrics-verified` 中既有聊天回归 1 项通过，真实存档复查 1 项通过；标准专项遇到上述主题定位问题，后续单独修正复验。
- 最终同一已构建程序，设置上述 REAL/SAVED（SAVED 指向 artifacts/benchmark-metrics-real-final/benchmark-metrics-real-two-cc0fb-g-tokens-and-resource-scope-benchmark-metrics-real），执行 `npx.cmd playwright test standard-benchmark.spec.ts benchmark-metrics-real.spec.ts --project=desktop --project=benchmark-metrics-real --output ../../artifacts/benchmark-metrics-acceptance`：2 项通过，43.7 秒。最终标准专项的受控答题请求为 12 次，串行、仅授权释放、半截输出不重发、恢复只补剩余 3 次；首帧空白不算首字，中间帧 999 计数不累计，缺时长速度为 null。覆盖三尺寸浅深截图、本题原始采样查看、聊天顶栏位置；看过实际浅深及资源截图。

实际模型为目录当时选出的 qwen3:4b-instruct，math-001 两次均为 95；输入合计 174、输出 6 Token，30 份 CPU/内存/可用 NVIDIA 资源采样。首字分别 24231 / 36 ms，响应 26008 / 64 ms，请求前状态分别未加载 / 已加载。原始记录保存在 artifacts/benchmark-metrics-real-final；后续多个复查没有追加真实模型调用。仅为一题两次观测，不是冷缓存保证、正式排行或完整 60 题验收。

已运行 artifacts/phase-c-acceptance/launch-metrics.cjs / verify-metrics.cjs，用最终程序重开原独立验收目录及 WebView 配置，保留用户保存的编辑，没有重拷生产库。进程 3884，标准评测入口读取到 62 个模型/题目复选项；没有自动发题，截图 metrics-ready.png。初次截图早于读取完成，等待真实复选项就绪后补拍。原生主窗口 IsWindowVisible=true；前台请求未获 Windows 同意，保留任务栏入口，不强制抢占前台。

没有下载模型、发起付费请求、运行 Docker、重新检查系统读屏/150% 或极限大计划。普通聊天回归使用受控服务，不以它声称全部知识/3D 功能重新验收。`git rev-parse --show-toplevel` 确认为当前独立仓库；未读取 token、未提交或推送已有工作树。

## 2026-10-27：窗口控制修复与标准批量评测（原切片 4）

用户指出聊天页窗口按钮落在回答依据卡片上；随后明确选择“补齐标准评测的批量运行与恢复”。先修正聊天顶栏定位并检查，再复用 C5 调度进入原切片 4。实现/文件清单/边界见 [标准批量交付](benchmark-standard-batches.md)。本轮新增标准任务范围入口，保持本地、无云端密钥/偏好准入；没有新数据库迁移，没有修改固定题库或正式发布物。

- UI 根因是聊天页窗口按钮绝对定位、依据卡片 -60px 上边距和标题 32px 补偿共同造成归属混淆。移除它们，并删除 Chat.tsx 的动态顶栏留位观察器，复用 AppShell 原有控件和事件。三尺寸展开/收起检查通过，未改聊天/知识/模型业务。
- 标准计划固定 Core60 v2/哈希、实际模型指纹、选题顺序、1–5 次重复、最多6模型和可选释放名单。默认短题，逐模型串行；与聊天共用任务锁。停止状态持久化；恢复只继续未发送项，完整输出缺首次评分时只补评分，已有尝试终态和评分仍不可变。
- 复用 benchmark_batches/items、Runner v2、评分存储与报告；个人历史与标准历史按数据集身份分开。标准入口拒绝云端模型，个人入口保留连接 Guard。可选释放核对目录及已加载指纹，仅处理授权名单，并记录实际请求/确认事件；无关已加载模型保持原样。官方 keep_alive 协议已查阅，链接见报告。

实际命令：npm/Playwright 在 apps/desktop，Cargo 在 apps/desktop/src-tauri。

- `npm run typecheck`、`cargo check --lib --offline` 通过。`cargo test --lib standard_ --offline` 最终 2 项通过；`cargo test --lib scoring_storage --offline` 4 项通过；`cargo test --lib personalized --offline` 最终 11 项通过。只运行新调度/恢复与相关兼容检查，没有运行无关单元或全套测试。
- 初次标准定向检查指出选题顺序与计划声明不同、补分路径触发已结束尝试保护。改为按批准题序排程，并保持旧尝试字段不变、仅追加缺失评分；修正后重启补分无新模型请求检查通过，没有放宽原 SQL 保护触发器。
- `npx.cmd playwright test --project=edge cloud-connection.spec.ts --grep 'chat window controls' --output ../../artifacts/window-controls-fix` 1 条通过，含三尺寸与收起前后坐标。
- `npx.cmd playwright test cloud-connection.spec.ts personalized.spec.ts --project=edge --output ../../artifacts/standard-benchmark-browser` 7 条通过、1 条旧文本选择器因新题库标题/历史列表重名失败；改用明确标题后 `--grep 'cloud onboarding' --output ../../artifacts/standard-cloud-compatibility` 单条通过。不是把失败算通过。
- `npm.cmd run tauri -- build --debug --no-bundle` 最终完成 TypeScript/Vite/Windows debug 构建，Rust 39.25s；index-Ayjl6kre.js、index-CFQRapux.css，保留既有 Vite 大块提示。一次测试等待同一构建目录的 Cargo 文件锁，未更换依赖/系统配置。
- `npx.cmd playwright test standard-benchmark.spec.ts --project=desktop --output ../../artifacts/standard-benchmark-native-final` 经允许使用隔离原生窗口与合成服务，1 条完整流程通过（22.3s）。首轮脚本过早读取异步计划 URL，等待实际报告就绪后复验。实际12次答题请求、最大并发1：2模型×2题×2次=8；第二计划停止1次后只继续3次。核对授权释放1次、无关模型未释放、原聊天/配置不变、标准执行图无密钥可读、取消原文/运行ID保留、评分覆盖75%与正确率分母正确。
- 已查看 native-chat-controls-1366.png 与 standard-report-1920.png，专项同时保存1366/1600/1920原生截图。按钮位于卡片上方的应用顶栏。没有以合成回答声称真实模型成绩，也没有跑真实云端/Docker/系统缩放或读屏验收。

用户保存并关闭了旧验收窗口。已用最终程序重新打开同一独立验收目录和窗口配置，保留用户刚保存的验收数据；进程28380，真实1440×900聊天页顶栏控件范围检查通过，截图 artifacts/phase-c-acceptance/window-controls-fixed.png，未自动开始评测。原生产数据库不升级。原生 IsWindowVisible=true；SetForegroundWindow=false，遵守系统前台限制，不声称强制置顶。

`git rev-parse --show-toplevel` 确认为当前独立仓库，`git diff --check` 通过（仅既有 CRLF 提示）；未读取 token、未提交/推送已有工作树。完成本切片后停止等待验收；下一步资源采样/指标、大报告分页/导出与真实完整验收各自保留，未把整个八片计划标为完成。


## 2026-10-27：Phase C / C3–C5 开发交付与验收窗口

用户验收 C2 后授权完成 C 阶段余下部分，并要求完成后弹出测试窗口。本轮按 C3 项目快照、C4 发送计划/候选审核、C5 复用 Runner 的串行评测推进。实现、文件清单、迁移和风险见 [C3–C5 报告](benchmark-phase-c3-c5.md)。保留已有 Core60、生产知识和聊天用途隔离；没有运行付费模型、下载模型、安装 Docker、更新发布安装包或推送。

实际命令（npm/Playwright 在 apps/desktop；Cargo 在 apps/desktop/src-tauri）：

- ZIP 读取新增 zip=2.4.2（仅 deflate），Cargo.lock 同步；`cargo fetch` 经允许获取构建依赖。扫描不解压到宿主机、不执行仓库脚本。
- `cargo fmt --all`；`cargo check --lib --offline` 通过。`cargo test --lib personalized --offline` 最终 11 项通过；`cargo test --lib benchmark:: --offline` 32 项通过、1 项真实 Docker 按原条件跳过，两组有重叠。为 v10 备份数量补强后单跑 `cargo test --lib personalized_migration_v10 --offline`，1 项通过，确认失败和成功尝试均产生可读的 v9 备份。
- `npm run typecheck` 与最终 `npm.cmd run tauri -- build --debug --no-bundle` 通过；最终 Rust 构建 31.99s，前端 index-BUZcD2s8.js / index-CmOAG0Y3.css，保留 Vite 大块提示。没有新增 npm 依赖；一次离线 prettier 因 ENOTCACHED 没有运行，也没有安装工具。
- `npx.cmd playwright test --project=edge personalized.spec.ts --output ../../artifacts/phase-c-ui-final`：1366×768、1600×900、1920×1080 各一条完整流程通过，已查看真实组件截图；接口数据明确为 UI 替身。`cloud-connection.spec.ts --project=edge` 4 条通过。
- 最初受限原生检查停在 CDP 超时/ECONNRESET，没有进入功能断言；使用已授权桌面检查权限，串行复核：`cloud-native.spec.ts --project=desktop` 1 条通过，含 13 个新接口绕过尝试；`trace.spec.ts knowledge.spec.ts --project=desktop` 2 条通过；设置 CORTEXOS_BENCHMARK_RUNNER=1 后，`benchmark-runner.spec.ts --project=benchmark-runner --grep 'native v2 scoring|native development benchmark'` 2 条通过。使用独立数据与本机合成响应，不是实际模型能力成绩。
- 必要修正包括：避免复用单连接时锁重入、按数据集身份区分个人 v1 与 Core v1/v2、固定审核修订、代码/Schema/工具请求大小纳入预算、停止/解除连接可取消、原有聊天执行图保留消息字段。新题库结构检查和人工审核职责分开，不声称语义已自动证明。
- `git rev-parse --show-toplevel` 确认为本仓库，`git diff --check` 通过（仅既有 CRLF 提示）；Gittoken.txt 与 artifacts 仍忽略，没有读取 token 或提交已有无关改动。

按用户要求启动最终调试版，验收窗口进程 23156，页面真实可见 1440×900；原生 IsWindowVisible=true，SetForegroundWindow=false，尊重系统前台限制。SQLite 只读备份原数据库到独立验收目录，原库仍 schema 7，副本 schema 10，副本保留 before-migration-8 的 schema 7 备份。验收目录与截图见报告；窗口留给用户测试，没有自动推理。真实云端权限/账单、Docker、新页面系统缩放和读屏仍是未验证项；本轮交付后停止。


## 2026-10-27：输入、阅读与预览栏六项微调

用户要求一次完成六项体验规范，同时限定不全面重构。本轮保留 Soft Glass 页面结构与现有业务状态，仅修改相关前端：

- `apps/desktop/src/components/ChatComposer.tsx`：组合输入状态与结束时间，选字后500ms拦截发送回车，保留换行和显式发送。
- 新增 `components/useChatScroll.ts`，在 `Chat.tsx` 接入：按行高判断一行跟随/三行跳转；用户上滑意图暂停，关闭浏览器自动锚定，正文/容器尺寸变化时复核位置。
- 新增 `components/PaneDivider.tsx`，在 `Chat.tsx` 与 `SourceBrowser.tsx` 的现有列间距内接入分隔条：透明遮罩、Pointer Capture、Esc/失焦回退、双击/Enter均分、方向键调整、主内容超过80%后松手收起。原关闭/恢复与窄窗抽屉继续可用；取消和恢复保留键盘焦点。聊天顶栏按实际依据栏宽度留位。缩放换算使用实际物理间距，修复快速双击后立即拖动被误当第二次点击的问题。
- 新增 `components/FileName.tsx`，资料列表、导入预览和详情标题保留扩展名；完整名称仍可查看/复制，版本号独立一行。
- `styles/tokens.css`、`controls.css`、`layout.css`、`features.css`：次级按钮默认无框，单层内嵌轮廓与分隔线，菜单/明确嵌套圆角按间距关联，保留选中态与键盘焦点。
- 新增 `tests/ui-micro-native.spec.ts`，`playwright.config.ts` 仅注册该专项；同步 `docs/design/html-prototype-ui.md` 与 `docs/implementation-plan.md`。

没有修改 Rust 业务、SQLite/题库、Provider、图谱相机、依赖或已有 Benchmark 数据；工作树中先前各阶段的未提交修改保留，不归入本轮改动。没有进入 C3、提交、推送或发布。

实际命令与检查过程：

- `npm.cmd run typecheck` 在 `apps/desktop` 通过。一次误在仓库根运行 npm dev 返回缺少 package.json，随后按项目目录执行，未安装依赖。
- `npm.cmd run tauri -- build --debug --no-bundle` 实际完成 TypeScript/Vite/Windows debug 构建；每次代码或布局修正后重建，保留既有 Vite 500kB 提示。
- 原生专项在独立库与本地合成流式服务运行。早期检查发现位置变化，关闭 overflow-anchor 并强化用户上滑意图；又通过临时事件/scrollTop调用轨迹区分浏览器惯性与程序写入。最终定位测试的“距离大于0”不能证明上滑：WebView2底部存在不足1px舍入差，改为等待相对初始位置真实上移，再分别检查惯性期间不能下拉与停止后位置稳定。临时 DOM setter 诊断已从正式测试移除，历史诊断输出保留在本机 artifacts。
- 测试资料首次放在隔离 data 目录外，被现有授权守卫拒绝；修正合成夹具路径，未放宽守卫。一次最终运行因自动审批服务用量限制未执行；用户要求继续后同项正常获批。未绕过审批或伪报通过。
- 既有必要回归：`npx.cmd playwright test tests/ui-usability.spec.ts tests/ui-usability-native.spec.ts tests/markdown-camera.spec.ts --project=edge --project=desktop --grep 'browser|native long records|native Markdown preview' --output=../../artifacts/ui-micro-regression`，4/4，17.1秒。2项浏览器菜单/面板，2项真实WebView2长记录/知识编辑/历史/资料菜单与Markdown原文/流式/引用偏移；数据与模型响应为隔离合成输入。
- 最终构建通过：Vite 781ms、Rust debug 19.81秒；主JS570.64kB/gzip172.67kB，未制作安装包。
- 最终专项：`npx.cmd playwright test tests/ui-micro-native.spec.ts --project=desktop --repeat-each=2 --output=../../artifacts/ui-micro-final`，连续两轮均通过，合计16.7秒。检查composition事件、500ms内回车、Shift+Enter、真实流式阅读位置、一行/三行阈值、按钮hover不改变尺寸、两处分隔条/透明遮罩/Esc/双击/键盘/80%收起恢复、125% CSS zoom后的比例、菜单12=8+4圆角、长名后缀与跨页功能。重复不累计为不同测试；本轮共3项不同原生检查、2项不同浏览器检查。
- 最终每轮9张实际截图：1366×768/1600×900/1920×1080浅深聊天6张，CSS zoom 100%/125%/150%资料预览3张，分别保存于上述目录的基础测试目录与repeat1目录。已审阅代表聊天浅色、深色资料放大截图，确认顶栏避让、版本号单独排布与后缀显示。没有声称像素相似度或Windows系统缩放验收。
- `git diff --check` 通过，仅保留已有CRLF提示；本机哈希复核旧题库/迁移/Benchmark引擎33份文件字节未变。

本轮不跑单元或无关全量测试。真实中文输入法候选操作、触控设备及 Windows 系统缩放仍保留手工验收；原生专项使用 composition 模拟事件，CSS zoom 截图不是系统缩放。完成后停在本轮交互验收。

## 2026-10-27：Phase C / C2 使用偏好与不可变版本

用户明确确认 C1 验收，授权下一步。复核开发规范、Phase C 计划、C1 Guard/界面、SQLite 迁移机制后交付 C2，详情与实际文件清单见 [C2 报告](benchmark-phase-c2.md)。仅新增工作区画像、三步可选问卷、查看/修改/跳过/重置、版本冲突保护和 schema 9；不进入项目扫描/云端生成/模型推荐。旧评分器/Runner、Core60 与既有 8 份迁移保留。

实际验证：

- `npm.cmd run typecheck` 在 `apps/desktop` 通过。第一次误在仓库根目录运行返回 ENOENT（根目录无 package.json），随后在规定目录执行；未安装任何依赖。
- `cargo check --lib --offline` 通过。
- `cargo test --lib profile_ --offline`：3/3，0.09 秒。覆盖真实 SQLite 保存/版本/并发冲突/工作区隔离/无效输入/重置/重开/不可变约束，8→9 前备份、故意迁移失败后的事务回滚与重试，以及 Guard 阻断未配置/操作中/已撤销状态。
- `cargo test --lib db::benchmark_foundation_tests --offline`：8/8，0.23 秒。原测试只更新新 schema 版本和新增表数量预期，旧聊天/知识/评测行及旧迁移 checksum 保护断言保留。共 11 个相关 Rust 检查，未运行无关全量测试。
- 浏览器先完成四项流程，首轮工具会话随后手动结束；后续干净退出的最终完整命令为 `npx.cmd playwright test tests/cloud-connection.spec.ts --project=edge --output=../../artifacts/phase-c2-browser-verified`：4/4，8.2 秒。C1 连接兼容三项 + C2 多领域/水平/任务/工具/权重/草稿/修改/重置/跳过一项。IPC 是明确替身，不代表真实云端身份验证。
- 截图审查后压缩第一步及权重行间距，并修正共享 input 样式优先级造成的滑块高度问题；`npx.cmd playwright test tests/cloud-connection.spec.ts --project=edge --grep 'profile questionnaire' --output=../../artifacts/phase-c2-layout-final` 最终 1/1，4.5 秒，为上一项重复，不重复计数。新增第七项权重位于底部操作区上方的边界断言通过。三尺寸第一步、1366 任务页及深色权重页均为实际渲染截图；查看代表图，没有用静态图实现页面。
- 正常权限 `npx.cmd playwright test tests/cloud-native.spec.ts tests/chat.spec.ts --project=desktop --grep 'native personalized|native chat preserves' --output=../../artifacts/phase-c2-native`：2/2，44.1 秒。隔离库中验证无凭据不能直接调用新增画像读写、原固定题库和导航可用，以及真实 SQLite 聊天取消/错误/重启；聊天服务为本地受控替身。无真实云端 key。
- `npm.cmd run tauri -- build --debug --no-bundle` 完成类型/Vite/Windows debug 构建；布局修订后重新构建。保留原 Vite 大块提示。未制作安装包或发布。
- `git diff --check` 与 `cargo fmt --all -- --check` 通过，既有 CRLF 提示保留。

数据仅写入测试隔离库，没有启动生产数据库升级。新迁移沿用升级前 VACUUM 备份和单事务；旧版程序不可直接打开 schema 9，回退需要兼容程序或升级前备份。真实云端账号下的原生画像端到端、系统缩放/读屏、Docker 实测继续待办。C2 完成后停止，等待验收；下一步 C3 尚未实施。

## 2026-10-25：Phase C / C0 复核及 C1 云端连接准入

按用户完整 Phase C 提示执行 C0 后直接 C1，完成后停止。读取总纲/规范/计划、评分器/Runner/目录/Schema/页面代码与阶段 B 本机证据，确认已有六类评分和单题本地执行，但没有完整 Benchmark UI/云端执行，Docker 实测仍未完成。新增 [Phase C 计划](benchmark-phase-c-plan.md)与[C1 交付报告](benchmark-phase-c1.md)，实际文件清单、5 个命令及已知边界见报告。

实现三家官方 HTTPS 模型目录验证、Windows Credential Manager 单连接原子记录、真实状态 Guard、取消/超时/互斥/更换/删除/恢复。仅新增必要评测入口、目录展示和连接表单，复用 Soft Glass/SelectMenu/Toast。未改评分器、Core60、SQLite 迁移或历史数据，不进入 C2–C5。用 ponytail 控制范围、ui-ux-pro-max 表单建议和 OpenAI Docs 官方接口资料。没有真实云端密钥或推理请求。

实际命令与结果（在独立仓库内执行，git rev-parse 确认根目录）：

- `cargo check --lib` 初次受沙箱 TLS 阻碍；`cargo fetch` 正常权限下载 18 个新增锁定包。随后 offline check 因 reqwest 未启用 query API 失败，改用现有 Url query_pairs_mut 编码，不额外增加 query 依赖。
- `cargo test --lib cloud_ --offline -- --nocapture` 初次 6 通过、1 凭据检查失败（沙箱登录令牌限制）；正常权限 `cargo test --lib cloud_windows_vault_synthetic_roundtrip --offline -- --nocapture` 1/1 通过。只创建随机测试目标并删除，不读取用户其他凭据。
- 增补超时后 `cargo test --lib cloud_ --offline -- --skip cloud_windows_vault_synthetic_roundtrip` 7/7 通过，运行 30.01 秒。包括真实等待 30 秒的 Mock 超时、六种错误、状态恢复、绕过/模型/Provider 检查、取消删除。最终取消竞争修复后 `cargo test --lib cloud_cancel_and_delete --offline` 1/1 通过，为前述检查的重复子集。共 8 个不同 Rust 检查，不是整个库全测。
- `npm.cmd run typecheck` 通过；`npm.cmd run tauri -- build --debug --no-bundle` 完成 TypeScript/Vite/Windows debug 构建。新增原生测试曾误用 createWorkspace 的 void 返回导致一次类型失败，修正后重建成功。最终 Vite 681ms；主 JS 555.55kB、gzip167.99kB，保留已有 500kB 警告；没有安装包/Release。
- 浏览器第一轮 1 通过、1 模型标签定位失败，修复明确 aria-label 和替身事件清理后 2/2 通过。最后增加取消晚到测试，`npx.cmd playwright test tests/cloud-connection.spec.ts --project=edge --output=../../artifacts/phase-c1-browser-final2` 3/3，6.5 秒。1366×768、1600×900、1920×1080、800×600 浅深主题共 8 张截图；已查看代表浅/深图。无真实系统缩放或读屏结论。
- 正常权限 `npx.cmd playwright test tests/cloud-native.spec.ts tests/chat.spec.ts --project=desktop --grep 'native personalized|native chat preserves' --output=../../artifacts/phase-c1-native` 2/2，43.4 秒（聊天 40.3 秒，C1 2.2 秒）。真实 WebView2/SQLite；聊天为本地受控 HTTP 服务。检查未配置、伪造 allowed 的 IPC/深链接、页面重载、v1/v2 固定题库与四个原导航，bootstrap 未被连接操作修改。
- `cargo fmt --all -- --check`、`git diff --check` 通过（既有 CRLF 提示保留）。本机 `artifacts/phase-c1/before.json` 比对：既有应用源码只改 App.tsx/navigation.ts/AppShell.tsx/NavigationIcon.tsx/lib.rs；既有 benchmark 评分/Runner、9 个数据文件、8 份 SQL 均字节不变。

最终构建 Rust 24.34 秒完成；最终二进制又运行 `npx.cmd playwright test tests/cloud-native.spec.ts --project=desktop --output=../../artifacts/phase-c1-native-final`，1/1 通过（4.8 秒），为前述 C1 检查重复，不累计为新测试。本机范围证据保存至 `artifacts/phase-c1/scope-check.json`，两份新增报告的本地链接检查通过。

真实三家认证/配额/推理权限、企业网络、真实账号完整进程重启待验收；Python Docker 待办沿用用户决定。不把目录成功写成生成能力或余额已验证。未下载模型/镜像、未生成 Level2/3、不发送付费请求、未推送或发布。当前停在 C1 验收。

## 2026-10-24：Core60 v2阶段B——运行时评分与单题闭环

用户验收阶段A后要求继续。复核三份设计/审计、v2契约、运行/存储/Provider实现，沿用ponytail复用既有SQLite、Ollama、TaskLease和请求快照。异步询问运行环境后，用户确认未安装Docker，要求先完成实现并保留实测待办。本轮不安装Docker或WSL，不下载镜像/额外模型，不进入原切片4，不生成120题或32场景。

### 本轮实际文件与功能

- 新增`src-tauri/src/benchmark/evaluation.rs`、`evaluation_json.rs`、`evaluation_tests.rs`，实现中文/整数、实际JSON Schema/精确数值、RAG事实/引用/生效条件、真实工具字段判定及独立状态/分母。
- 新增`python.rs`、`python_runner.py`、`python_tests.rs`：本机Linux Docker、镜像内容ID、无网络/挂载/Socket、CPU/内存/PID/时间/合并输出限制、只读/非root/禁提权、逐断言类型/输入观察与宿主比对、所有路径清理；Docker缺少时Execution Error，不执行宿主机提交。真实Docker检查显式ignored，不算通过。
- 新增`scoring_repository.rs`、`scoring_repository_tests.rs`、`runner_v2.rs`；修改dataset/mod/preflight/repository/runner接入显式版本2，沿用原三个业务命令和互斥/取消。评分保存在原表，包含独立引擎版本、规则/契约/输出hash，输出hash同时包含真实工具帧；重复启动/完成不重复请求或插评分。保存失败保留原文，不给模型填0。
- 修改`generation.rs`、`ollama.rs`：v2严格JSON帧校验、实际tool_calls帧保存、无文字工具响应完成、客户端单调计时；运行前能力检查保留证据。普通聊天/v1维持原文字路径。RAG把实际编号资料送入请求并记录fixtureRefs；JSON Schema仅schema题发送，工具声明只用于工具题，没有伪造检索、工具执行或隐藏思维链。
- 修改原repository/tests、dataset_v2_tests夹具，显式指定版本1及空镜像选项；修改`tests/benchmark-runner.spec.ts`新增v2受控原生检查，保留旧v1及真实模型开关。
- `Cargo.toml`/`Cargo.lock`固定jsonschema 0.60.0，default-features=false；serde_json启用arbitrary_precision/raw_value，防止小数舍入误判。新增29个锁定包，原锁定版本未替换；没有npm依赖变更。元数据核对jsonschema、jsonschema-value、referencing为MIT，jsonschema启用features为空，外部Schema网络/文件引用在规则层也拒绝。
- 新增[阶段B交付报告](benchmark-scoring-stage-b.md)；同步docs README、implementation-plan、benchmark-implementation-plan、benchmark-core60-v2、benchmark-scoring-v2、benchmark-evaluation-methodology、benchmark-preflight、benchmark-execution-slice3、benchmark-database-schema、test-plan和本日志。完整应用文件表和接口例子见报告。

数据集两版9份文件及hash不变；数据/schema/规则版本仍1和2，运行时引擎新增`core60-runtime-2.1`。创建时的manifest元信息不静默改写，当前能力放在runtime目录字段。数据库仍开发schema8，没有DDL迁移或新的迁移前备份操作；正式用户库未打开。前端src/既有UI、知识引擎、8份SQL迁移、权限登记与用户reference/desktop.spec.ts等原状态保留。没有NSIS/Release、Git提交/推送或版本发布，未读取凭据。

### 实际命令与结果

1. `git rev-parse --show-toplevel`确认`D:/vibecoding/CortexOS`。开始时保存122个已有应用/题库/迁移/检查文件的hash到`artifacts/benchmark-scoring-v2/before.json`，定向读代码后实施；未根据模型跑分调整答案。
2. `cargo fetch`在受限环境因Windows TLS `SEC_E_NO_CREDENTIALS`失败；获准在受限环境外执行同一命令后成功。仅获取构建依赖，没有模型/系统软件安装。`cargo metadata --offline --format-version 1`实际核对版本/许可及空features；Cargo.lock保留已有包版本，新增29包。
3. `cargo test --lib scoring_ -- --nocapture`首轮10项纯评分/控制器通过。加入持久化专项后首次出现4项失败：旧summary查询只认development_single_case_v1，v2记录被误报不存在；修正为明确的数据版本与模式匹配后全部通过。没有把首次失败记为通过。编程异常测试故意panic，被捕获转为Scoring Error，用例通过；测试输出的panic文本不是程序崩溃。
4. 最后完整评分专项为`cargo test --lib scoring_ -- --nocapture`：16通过、0失败、1 ignored，编译7.86秒、用例1.11秒。覆盖50个非编程题标准响应及重复确定性、合法等价/错误/缺少/多余/错误类型/顺序/null/重复键/真实日期/精确数值、RAG位置与支持/拒答/冲突/过期/缺证据、真实工具与正文伪装/分片、分母、环境/评分器异常和历史/幂等/存储恢复。编程控制器以受控CLI观察核对全部93条结果及错误类型、输入变化、额外输出、超时、取消、错误环境、清理失败；没有实际执行模型程序。补充了评分保存错误事件对应Scoring Error统计的断言。
5. `cargo test --lib benchmark_ -- --nocapture`19/19通过，用例2.47秒，覆盖原数据、备份/迁移、用途/工作区、幂等、旧判定及恢复；`cargo test --lib ollama::tests -- --nocapture`2/2通过，覆盖分片UTF8/多帧/坏流和端点限制。本轮独立Rust检查共37项通过，真实Docker1项ignored；不是全库50项全部运行。
6. `npm.cmd run typecheck`通过。`npm.cmd run tauri -- build --debug --no-bundle`通过（含TS/Vite/Rust），首构建Rust56.59秒、Vite12.77秒。JS546.73kB/gzip165.16kB、图谱941.05kB，前端产物名字与既有相同；保留Vite体积/npm版本提示，无新增Rust告警，不安装npm更新。
7. 设置`CORTEXOS_BENCHMARK_RUNNER=1`，在已知受限环境无法稳定启动WebView2的条件下，获准在受限环境外运行`npx.cmd playwright test tests/benchmark-runner.spec.ts --project=benchmark-runner --grep 'native v2' --output=../../artifacts/benchmark-scoring-v2-native`：1/1通过，总15.4秒、用例12.9秒。包含14个实际隔离SQLite实验，受控HTTP服务共11次生成请求；状态为Passed6、Failed3、Unsupported2、Execution Error2、Needs Review1，均是预声明的正反例结果，不是实际模型能力成绩。复用同launchKey未新增请求或评价，精确请求字节hash一致、两版隔离、Schema/tools/编号资料实际发送、真实工具帧/无文字/片段、普通文字伪装拒绝、初检与运行前能力不支持、Docker缺少不调用模型、断流保留部分、截断待复核及普通bootstrap不变均通过。
8. `npx.cmd playwright test tests/benchmark-runner.spec.ts tests/chat.spec.ts tests/knowledge.spec.ts tests/retrieval.spec.ts tests/trace.spec.ts tests/sources.spec.ts --project=benchmark-runner --project=desktop --grep 'native development|native chat preserves|knowledge drafts|automatic retrieval|execution graph|authorized snapshots' --output=../../artifacts/benchmark-scoring-v2-regression`：6/6通过，总约1.3分钟。旧v1检查7.1秒、聊天38.2秒、知识9.3秒、检索4.7秒、来源6.1秒、执行图9.1秒；都是真实WebView2与隔离库、受控模型服务，不是真实Ollama质量/长期使用验证。
9. 收尾给容器入口补上非Python3.13时在执行提交前直接返回的保护；只影响容器脚本，不改变已通过的聊天/旧接口路径。最终再次`npm.cmd run tauri -- build --debug --no-bundle`通过，Rust19.06秒、Vite744毫秒；再以`--output=../../artifacts/benchmark-scoring-v2-native-final`重跑相同v2原生命令：1/1通过，总14.6秒、用例12.7秒。最终证据`v2-scoring-controlled-evidence.json`仍为14项/11请求。最后`cargo test --lib scoring_python_ -- --nocapture`4通过、1 ignored，用例0.38秒；不把重复执行累计成更多独立检查。
10. `py -3 -X utf8 scripts/benchmark_core60_v2.py`通过，60题/93断言及原5份v2资产逐字节可复现。可信容器脚本用标准库`ast.parse`只检查语法，没有在宿主执行提交。`cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --all -- --check`及`git diff --check`通过，仅既有CRLF提示。
11. `py -3 -X utf8 artifacts/benchmark-scoring-v2/check_scope.py`通过：原122文件只有本轮预期10个应用/检查文件修改，其余112个原文件不变；两版题库9文件、前端src和8迁移保持原样，npm清单/锁文件与HEAD相同。只读打开本轮原生隔离库确认14实验/14评价/120版本化题目、11个真实模型请求开始事件、知识节点/检索/上下文知识写入0；所有记录数据版本2，排除状态无正确率分数。运行表的19文件源码/依赖指纹与最终构建/当前实现一致：`benchmark-scoring-sha256:5583d32618d3f05f042b3e4d7f6844946c3ef30538d1e048d6f8d1e321ca1826`。结果写入同目录scope-check.json，并核对本轮文档本地链接。

### 未运行、风险与停止点

真实Docker容器、Windows/WSL资源/网络/文件隔离压力和93条实际代码执行未运行，原因是环境未安装且用户选择保留待办；没有假称Docker安全验收。已经写好显式ignored的实际容器检查，准备好本地Python3.13镜像ID后可另行运行。容器内类型/输入观察加宿主比对不是对恶意Python的形式化纯函数/防作弊证明；Core60保持Smoke定位。

没有实际本地模型答题、付费API、额外模型/镜像下载、完整60题模型结果、批量/多模型排程、新评测UI、重评分/导出、资源峰值采集或Level 2/3数据。原150%验收不外推新的评测页面；本轮没有视觉改动，不给新截图/读屏/缩放结论。

Unsupported等排除项可见，正确率分母仅Passed+Failed；模型执行完成率与题目评分覆盖率分开，同时给单题计划和全60题覆盖口径，防止缩小分母误称全部100%。Failed3等原生场景是故意错误响应，不是失败的自动化检查。普通文本工具JSON不会被记录为工具调用，实际工具执行为0。旧历史不会因新引擎静默重判；本轮没有重评分接口，启动恢复也不自动重发。

局部评分保存失败已通过触发器验证原文保留；全库不可写仍可能无法记录错误自身，没有做磁盘耗尽实验。当前源码指纹覆盖指定19个文件，不等同完整Git发布提交。阶段B实现交付，按约定停在验收；真实Docker待办保留，确认后才安排原切片4。

## 2026-10-24：Core60 v2阶段A——题库版本化修订

用户原则批准三级Benchmark设计，明确先做阶段A并验收后才做阶段B。复读development、开发标准、计划、三份审计/设计/评分报告、全部60题及现有数据/预检/运行/存储代码；逐条对应Q-01–Q-12。沿用ponytail复用serde/rusqlite/chrono及标准库，没有增加依赖、Docker或评分框架。运行时评分尚未扩展，本轮不进入原切片4。

### 实际文件与版本

- 新建`apps/desktop/src-tauri/datasets/cortex-core-60-v2/`中的cases、manifest、case.schema、scoring-rules、revision-notes共5份JSON。保留原60题ID、类别、难度及v1文件；v2数据/schema/六种规则均版本2，全部标待审阅，定位Smoke。数学等差、工具limit、中文别名/字形、RAG结构化事实/支持范围/固定时间、Python契约/93条断言及JSON日期等变更详见[阶段A报告](benchmark-core60-v2.md)与逐题差异文件。
- 新增`scripts/benchmark_core60_v2.py`，默认从固定v1重新编制并逐字节比较v2，首次创建模式拒绝覆盖已存在目录。只有可信作者的固定计算，没有执行模型代码或宿主机不可信提交。编制期间在尚未验收、未被实际运行引用的v2草稿内补齐RAG评分专用schema标识并移除旧evidence变体；最终文件/hash以下述值为准。v1始终逐字节保留。
- 修改`src-tauri/src/benchmark/dataset.rs`：注册两个版本，按版本校验/入库/读取、单事务安装、同版本不覆盖；新增可省略Scoring.contract，旧v1序列化保持原内容。新增`dataset_v2.rs`及`dataset_v2_tests.rs`做作者数据的有限校验与6项专项，不是模型响应评分器。
- `src-tauri/src/benchmark/repository.rs`只将开发入口从数组首项改为明确查找版本1；传v2 hash会拒绝，不误用v1引擎。`tests/benchmark-preflight.spec.ts`更新双版本目录预期、增加v2读取，并保存版本/题目/契约证据。前端src、聊天、知识、模型生成协议、图谱和迁移均未修改。
- 新增`docs/benchmark-core60-v2.md`，同步README、implementation-plan、benchmark-implementation-plan、benchmark-evaluation-methodology、benchmark-preflight、benchmark-tiers-design、benchmark-scoring-v2及本日志。原审计、原题目审阅表、用户reference和desktop.spec.ts状态保留。

没有新SQL迁移、正式用户库访问、云端请求、模型下载、模型问答、工具实际执行、打包发布或Git提交/推送。开发schema仍8，正式v0.2.0仍schema7。没有新迁移所以不声称执行了新的迁移前备份；本轮回归了既有备份/失败恢复专项。

### 实际命令与结果

1. `git rev-parse --show-toplevel`确认独立仓库`D:/vibecoding/CortexOS`。开始时将119个现有应用/测试/数据/依赖文件的hash保存到`artifacts/benchmark-core60-v2/before.json`，未读取凭据。读取旧case全字段、评分规格及执行代码后确定保守契约；不是依据审计摘要直接改标准，也没有按模型分数调答案。
2. `py -3 -X utf8 scripts/benchmark_core60_v2.py --write-new`首次创建新目录成功。编制草稿协议补充后，`py -3 -X utf8 scripts/benchmark_core60_v2.py`最终通过：60题、93条断言、全部5份资产可复现。原32条断言保留为每题前缀；阶乘0–12穷举；重复偶数和大小写回文反例已在作者数据检查中核对。只是可信固定输入/标准计算，不是运行模型提交。
3. `cargo fmt --all`完成。第一次`cargo test --lib benchmark_v2 -- --nocapture`遇测试用切片不满足encode的Sized约束；将通用编码函数放宽为`Serialize + ?Sized`后，最初5项通过，补版本冲突项后6项通过。期间`cargo test --lib benchmark_bundle -- --nocapture`2/2、`cargo test --lib benchmark_development -- --nocapture`3/3通过。
4. 最终数据/协议定稿后，`cargo test --lib benchmark_ -- --nocapture`19/19通过，编译11.90秒、用例3.11秒。包含新增v2六项、原题库两项、开发执行三项，以及原数据库基础八项；没有运行全库33项。覆盖数据/规则/hash/别名契约、坏类型/缺失/额外字段/无效日期、支持范围/错误来源/冲突/生效时刻、定义域、同版本冲突拒绝、原子失败回滚、重开读取、v1合成运行/评分保持、v2误运行拒绝；既有迁移/备份/用途隔离也通过。合成历史记录有明确fixture标记，不冒充用户实际评测成绩。
5. `npm.cmd run typecheck`通过，`npm.cmd run tauri -- build --debug --no-bundle`通过，含TS/Vite/Rust桌面编译；Rust34.77秒、Vite1.26秒。JS546.73kB/gzip165.16kB、图谱941.05kB，前端内容与此前一致。保留已有Vite体积及npm版本提示，无新增依赖或安装更新。未生成NSIS/Release。
6. 设置`CORTEXOS_BENCHMARK_PREFLIGHT=1`，运行`npx.cmd playwright test tests/benchmark-preflight.spec.ts --project=benchmark-preflight --grep 'controlled model' --output=../../artifacts/benchmark-core60-v2-native`。受限环境首次连接到CDP但5秒内页面数仍0；相同命令改输出到`...-native-retry`时CDP健康检查30秒超时。均为启动阶段失败，没有到业务断言；保留error-context。失败后停止对应检查进程，没有为绕过环境问题修改共享native.ts或放宽断言。
7. 获准在受限环境外运行同一命令，输出到`../../artifacts/benchmark-core60-v2-native-final`，1/1通过，总6.0秒、用例2.4秒。补充将datasets/v1Case/v2Case写入证据后，以`...-native-evidence`再次执行，最终1/1通过，总8.1秒、用例3.2秒。是真实原生WebView2/SQLite及受控本机HTTP服务，不是浏览器替身或真实Ollama答题；双版本读取、版本2事实/规则、工作区拒绝和预检未知/不支持等均通过。完整证据在该目录的`controlled-preflight.json`；请求仅tags/version/ps/show，生成请求0。只访问项目内新隔离库。
8. 补充原生证据后再次`npm.cmd run typecheck`通过；`cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --all -- --check`通过。`py -3 -X utf8 artifacts/benchmark-core60-v2/check_scope.py`通过：原119个文件中只有dataset.rs、repository.rs、benchmark-preflight.spec.ts这3个发生本轮预期修改，其余116个原文件不变；v1的4份文件、8份迁移、依赖清单/锁文件与开始一致。全部新题及文件hash、60条差异、原断言前缀、文档本地链接、原生双版本及无生成请求证据检查通过；输出`scope-check.json`。`git diff --check`通过，仅既有CRLF提示。

v2最终manifest SHA-256为`b8d462c2408f948c9a1f74d1e044cb4f78dbfabe52f76f93e6d135ee171edb70`；cases为`8a575711712d352d9e61d915c6b95b01e54c4b280e3cb3fb8175404c4e168174`。v1 manifest仍为`3595faf98b35865e00ba7f156ba89a21f6f5406f8f1b2908f41df031c3f882cc`，cases仍为`784089a5243bb3dc1cf4b723b351df19c5006ff983d97c3f4e8f9a946bc88806`。运行端仍绑定v1，未用v2静默重判任何历史输出。

### 未运行、风险和停止点

没有运行v2模型答题、Docker隔离、六类模型响应评分器、完整60题、付费API、真实本机模型预检或UI全套/缩放/读屏；这些不在阶段A范围。原生检查只确认当前数据接口与受控预检，没有新视觉变更或新截图结论。

运行时Schema方言/数值语义、原生工具对象捕获、逐断言执行/输入修改、RAG各维度评分及六状态报表仍待阶段B。作者标准静态校验采用有限词汇，不取代成熟JSON Schema验证器；引用支持范围与别名仍需要人工语义复核。Python3.13仅是契约目标，尚未安装/验证对应容器镜像及资源限制。v2状态保持needs_human_review，不将技术专项通过冒充全题人工验收。

阶段A已交付，停在用户验收门槛。下一步须用户验收后才实施阶段B评分器及其正反控制测试；不自动继续原切片4、生成Level 2/3数据或发布新版本。

## 2026-10-23：Benchmark测试集质量审计与三级设计

用户审阅原benchmark-dataset-review后，明确Core60仅作基础Smoke，要求先逐题审计并设计新的能力/认知测试体系。本轮复核development、开发标准、实施计划和既有Benchmark方法，读取全部60题、标准/别名、32条代码断言、13份资料22行、5份JSON标准和5份工具参数示例，并追到加载器、模型预检、Runner、存储判定及Provider解析路径。独立保存本轮技术报告；没有将AI复核表述为第三方盲审或独立人工签审。

### 本轮实际修改

| 文件 | 内容 |
| --- | --- |
| [benchmark-core60-audit.md](benchmark-core60-audit.md)（新增） | 全部60题逐条核对、代码/数据位置、12项问题及优先级、32条已有断言值、运行时评分与能力标记边界 |
| [benchmark-tiers-design.md](benchmark-tiers-design.md)（新增） | Level 1版本修订方案、Level 2的120题配额/公开与保留划分/分级评分、Level 3的32个固定快照场景与A/B/O证据诊断；均为提案 |
| [benchmark-scoring-v2.md](benchmark-scoring-v2.md)（新增） | 功能/能力/难度/工具/编程/引用/性能独立指标、互斥数量账本、分母/覆盖率、评分器正反控制及版本审计 |
| [README.md](README.md)、[implementation-plan.md](implementation-plan.md)、[benchmark-implementation-plan.md](benchmark-implementation-plan.md)、[benchmark-evaluation-methodology.md](benchmark-evaluation-methodology.md)及本日志 | 文档入口、Core60的Smoke定位与当前设计验收停止点；保留既有八片顺序和历史结果 |

工作区原有切片1–3及用户文件状态保留，本轮没有应用源码、依赖、迁移、UI或测试文件修改。未覆盖原审阅表、cases、manifest、schema、规则或历史结果；没有创建v2、能力题或隐藏题，没有打开用户库或执行迁移、模型、容器。没有提交、推送、标签或版本发布，未读取凭据。

### 审核结论与实际检查

- 60题均有独立行记录；35道easy、25道medium、0道hard。数学15项标准值经固定算式复算一致，但math-006须额外假设等差，不能标题意无歧义。10道编程题的32条现有预期值一致，仍缺边界/类型/输入不变验证；不代表运行了模型代码或编程题已通过。
- 明确记录math-006、tool-002、zh-014、RAG拒答/说明/跨度及as_of、Python码点/空白/边界等问题。JSON当前只有作者示例的小词汇检查，不能误称通用结构化评分已实现，也没有证据说正在用字符串评分。原生工具字段尚未保存/验证；其余58题当前未开放不能归因模型不支持。
- 事实与协议疑点通过教育部字典、教育局原文材料、BIPM、NASA、PubChem、Python/Unicode、JSON Schema/RFC及Ollama官方资料核对，链接放在相应结论旁。虚构RAG材料只按内部事实审阅；行号检查不代替语义证据支持。
- `git rev-parse --show-toplevel`确认`D:/vibecoding/CortexOS`。使用`rg`及PowerShell定向读文件，未变更仓库边界。
- `py -3 -X utf8 artifacts/benchmark-quality-audit/audit_core60.py`成功：60个唯一ID/顺序/内容与文件hash、15项数学固定计算、32条可信控制计算、5份JSON标准/5份工具参数的有限结构检查、10个引用跨度。脚本没有加载任何待测代码；它不是通用Schema一致性测试或正式评分器。首次输出已有Unicode的读取命令遇cp950编码错误，改用`-X utf8`后成功，没有因该错误修改数据。
- `py -3 -X utf8 artifacts/benchmark-quality-audit/audit_core60.py --verify`通过：审计表60个ID与cases顺序完全相同；87个指定应用源码/数据/迁移/配置/依赖文件与本轮开始快照逐字节一致；三份新报告的本地链接和行尾检查通过；新增数据集版本数0。辅助脚本、完整断言控制结果及开始hash存于已忽略的本机`artifacts/benchmark-quality-audit/`，不混入发布题库。
- 收尾同一`--verify`再次通过，并核对60个题目行锚点都指向相应case_id。另将只读链接脚本经PowerShell的`$documentCheck | py -3 -X utf8 -`执行，8份本轮文档的157个本地链接及带行号目标均可定位；没有据此宣称外部网页永久可用。
- `git diff --check`通过，保留原工作区CRLF提示。本轮只做资料/逻辑审阅和文档检查，按用户规范未运行单元套件、类型检查、构建、浏览器/原生WebView2或真实模型检查；不复用旧切片测试数冒称新结果。

Core60 v1的manifest SHA-256仍为`3595faf98b35865e00ba7f156ba89a21f6f5406f8f1b2908f41df031c3f882cc`，cases仍为`784089a5243bb3dc1cf4b723b351df19c5006ff983d97c3f4e8f9a946bc88806`；needs_human_review保持历史原值，未标全部通过。正式能力集提案为120题（90公开/30保留，含36基础/48中等/36困难），隐藏题不能随public仓库或客户端分发后声称对本机持有者保密。认知场景固定知识/历史/as_of并独立索引，按检索、组装、已给证据后的回答、评分器分别定位；现有未实现的记忆/压缩机制保持系统阻断。

### 停止点

审计和设计交付完成，等待用户确认。建议下一步单独验收Core60 v2质量修订及兼容加载，再回到八片计划；后续正式能力集先少量公开试点而非一次生成120题。实施进度仍为切片3，未启动切片4，不把本轮报告或现有单道开发结果当正式能力排行榜或完整Smoke验收。

## 2026-10-23：Benchmark Phase 1切片3——最小真实执行闭环

用户确认切片2后开始第3片。先复核development、开发标准、原实施计划、Benchmark八片计划、执行/存储/Provider及原生检查；沿用ponytail复用现有解析、SQLite和依赖。当前只实现单模型、单题、一次尝试的开发闭环，不进入切片4的多项排程或恢复开发。60题内容仍待独立人工审阅，本次确认不改变该状态。

### 本轮实际修改文件

应用路径相对`apps/desktop`。切片1/2已有工作区改动继续保留，用户原有desktop.spec.ts状态和reference素材未操作；本表只列本片修改。

| 文件 | 实际改动 |
| --- | --- |
| `src-tauri/src/generation.rs`（新增） | 从commands提取一次性共享执行：实际检查/发送事件、NDJSON、250ms检查点、取消/超时/部分保存和终态；聊天/评测检查策略分开，任务占用由调用方持有 |
| `src-tauri/src/tasks.rs`（新增） | TaskSlot/Lease统一两种任务；按ID/工作区/kind取消，准备阶段摘要、当前执行ID、自动释放和关闭取消 |
| `src-tauri/src/benchmark/repository.rs`、`runner.rs`（新增） | 独立开发配置/模型快照/事务准备、同launchKey幂等、只开放math-001与zh-006；原文先存、读取已存原文做两种精确判定；专用详情和保存失败记录 |
| `src-tauri/src/benchmark/repository/tests.rs`（新增） | 必要合成SQLite专项：准备失败回滚、用途拒绝、终态/迟到写入保护、评分边界/幂等、评分写入失败保留输出 |
| `src-tauri/src/benchmark/mod.rs`、`preflight.rs` | 注册两模块；复用已验证模型参数构造，保持不读写聊天设置 |
| `src-tauri/src/db.rs` | 提取两种用途共用的insert_execution原子记录；内部事件/检查点/终态按用途及完整运行身份校验；详情读取内部复用，公开聊天范围保留；旧仅测试使用的写入帮助函数限制为cfg(test) |
| `src-tauri/src/commands.rs`、`models.rs`、`lib.rs` | 聊天改调用共享执行；bootstrap新增有kind的activeTask且activeRun仍仅chat；实现start/get/cancel_benchmark；关闭标志同时拒绝新任务 |
| `src-tauri/build.rs`、`capabilities/default.json` | 只登记上述3个业务命令，无通用系统权限 |
| `src/studio.ts` | 仅补Bootstrap.activeTask可选类型；运行时UI、状态操作和样式不变 |
| `tests/benchmark-runner.spec.ts`（新增）、`playwright.config.ts` | 可选受控原生专项及显式启用的一道真实本地开发题；默认套件不加入真实调用 |
| `docs/benchmark-execution-slice3.md`（新增） | 单题接口、数据流、占用、判定和失败边界 |
| `docs/README.md`、`implementation-plan.md`、`test-plan.md`、`benchmark-implementation-plan.md`、`benchmark-architecture.md`、`benchmark-database-schema.md`、`benchmark-evaluation-methodology.md`、`benchmark-provider-design.md`、`benchmark-preflight.md`及本日志 | 对齐本片交付、实际验证和等待验收状态，保留历史记录 |

没有新迁移、依赖或模型标签；dataset/规则/hash保持切片2字节。schema仍8，正式v0.2.0仍schema7。没有打开/升级正式用户库，没有NSIS/Release、提交、推送或标签改动。Gittoken.txt未读取，用户系统设置和已安装模型未改动。

### 实际命令、检查结果与证据

- `git rev-parse --show-toplevel`确认独立仓库`D:/vibecoding/CortexOS`，`rg --files -g AGENTS.md`只有根规范。定向读取现有commands、db、models、Ollama、schema8及相关原生用例，记录旧聊天事件/请求合同后提取。
- `cargo fmt --all`和`cargo check --lib`通过。第一次共享代码阶段有临时未使用导入/尚未接入模块提示，后续接入及测试辅助函数cfg(test)后最终check无Rust告警；未增加依赖或接口工厂。
- `cargo test --lib benchmark_development -- --nocapture`：3/3通过，编译12.07秒、用例0.53秒。仅运行必要新增数据专项，没有运行全库单元测试。覆盖事务中途快照写入失败的整体回滚、合法聊天保持、错误用途/工作区拒绝、快照/hash、整数正确/错误/非法格式/输出上限、别名、同标识配置冲突、重复评分不追加、终态后迟到事件/检查点/终态不覆盖、评分插入失败保留原文并记录失败。
- `npm.cmd run typecheck`通过；`npm.cmd run tauri -- build --debug --no-bundle`通过（含类型检查、Vite和Rust），Rust27.74秒、Vite616毫秒。主JS546.73kB/gzip165.16kB、图谱941.05kB，产物名称与此前相同；保留既有Vite体积和npm更新提示，无Rust构建告警。没有重做UI或重打正式包。
- 使用已经缓存的prettier.cjs，经require_escalated读取缓存并用node整理新增测试文件，不联网/不安装。收尾再次`npm.cmd run typecheck`通过；`cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --all -- --check`通过。
- 设置`CORTEXOS_BENCHMARK_RUNNER=1`，运行`npx.cmd playwright test tests/benchmark-runner.spec.ts --project=benchmark-runner --grep 'native development' --output=../../artifacts/benchmark-slice3-native`：1/1通过，总8.0秒、用例6.0秒。验证受控HTTP正文与快照对象/原始字节SHA-256相等、独立64/0参数、用途/跨域/知识保存拒绝、同标识不重发、双向互斥、流中和准备中取消、部分保存、迟到无覆盖、发送前模型替换、坏流不评分。是真实WebView2与SQLite，模型为受控服务。
- 补强隔离夹具：先建立一段真实走过产品请求链路的受控聊天历史，并创建/审核能被普通聊天preview_context实际命中的知识（含明确测试标记）。相同原生命令输出改为`../../artifacts/benchmark-slice3-native-final`：1/1通过，总7.9秒、用例5.8秒。评测仍只有system+当前题目，未带入两个测试标记，原聊天列表/运行及知识未被评测改写。最终受控HTTP共有5次chat请求：2次日常聊天、3次评测（成功、流中取消、坏流）；重复启动、准备取消和指纹替换均未增加评测请求。证据controlled-runner-evidence.json保存在对应输出子目录。
- 另设`CORTEXOS_BENCHMARK_REAL_RUN=1`，运行`npx.cmd playwright test tests/benchmark-runner.spec.ts --project=benchmark-runner --grep 'one actual local' --output=../../artifacts/benchmark-slice3-real`：1/1通过，总8.9秒、用例7.0秒。实际已安装qwen3:4b-instruct，digest `0edcdef34593eac1aa2be9c7d06c432dcf81945adca5eca2f27662c18f168ba0`；只执行math-001一次，原文`95`，integer_equal v1的correctness=1、formatValid=true。请求4096容量、64输出上限、温度0、180秒超时是请求值，不冒称实际有效容量。没有问第二题、下载/释放模型或运行代码题。完整证据actual-single-development-case.json含真实事件、请求、原文、规则/hash及评分；这是闭环开发样本，不是模型综合准确率。
- 共享逻辑改动后，从apps/desktop运行`npx.cmd playwright test tests/chat.spec.ts tests/knowledge.spec.ts tests/retrieval.spec.ts tests/trace.spec.ts tests/sources.spec.ts --project=desktop --grep 'native chat preserves|knowledge drafts|automatic retrieval|execution graph|authorized snapshots' --output=../../artifacts/benchmark-slice3-regression`：5/5通过，总约1.3分钟；聊天44.2秒、知识9.0秒、检索4.4秒、来源6.3秒、执行图9.4秒。覆盖原取消/断流/超时/重启、草稿/审核/冲突、实际检索请求、来源停用、执行图及精确版本/消息定位；是受控服务下的原生业务检查，不冒充真实长期模型或新评测UI验收。
- `py -3 artifacts/benchmark-slice3/check_scope.py`通过：0001–0007和依赖清单/锁文件与HEAD逐字节相等，0008保持切片1hash，迁移仍8份，数据集manifest保持`3595faf98b35865e00ba7f156ba89a21f6f5406f8f1b2908f41df031c3f882cc`。前端只有Bootstrap类型变更。仅只读实际检查的隔离SQLite，确认执行/尝试/评分各1条、生产知识引用/检索各0条；source_revision为`slice3-core-sha256:a824aa550f8ed7c2285cd08a87c9d52cf9a379208b35b40e35d32c3faa8c1a21`，与当前相关十文件内容指纹一致，hardware快照明确unavailable/not_collected_in_slice3。证据artifacts/benchmark-slice3/scope-check.json。
- `git diff --check`通过，仅既有CRLF提示。没有视觉改动，所以本轮无截图相似度、系统缩放或新页面布局结论；原150%用户验收不外推未来评测页。

### 本轮边界和停止点

已具备一个模型×一道开发题×一次尝试的实际闭环，两道题可单独选择；没有队列、多模型分组、实验模板、手动续跑、自动重试、导出或新评测页面。源码内容指纹覆盖相关执行模块，不等于完整仓库/Git发布版本。固定60题仍需独立人工内容审核，代码/资料/Schema/原生工具评分及完整指标继续后续切片。

已有启动中断对账保留，尚未新增切片4的批量恢复协议。取消确认的是本地停止接收，不保证远端立即停止计算；Ollama外部标签在最后预检之后仍可能被修改，API没有跨进程模型锁。评分存储失败保留已落盘原文并标实验失败；全面不可写时错误本身可能也无法保存，最终仅能恢复最后检查点，没有做真正耗尽磁盘的实验。硬件资源没有采集，不填零或由当前显存猜峰值。

本轮停在第3/8切片，等待用户验收。确认后才进入切片4“串行排程与恢复”；不在本轮扩展执行范围。

## 2026-10-23：Benchmark Phase 1切片2——题目与模型预检

用户在切片1后要求“做下一步”。复核八片计划、schema8依赖、开发标准和实际Ollama连接；本轮只实施第2片，完成后停在验收门槛。沿用ponytail复用现有rusqlite/serde/sha2/reqwest和本地Provider，无新增依赖、SQL迁移、Runner或UI。未打开正式用户数据库，测试使用新隔离目录；不生成回答，不运行题目代码，不接云端。

### 本轮实际修改文件

下表应用路径相对`apps/desktop`；之前切片1及用户reference/desktop.spec.ts的工作区状态保留，不混作本轮新增内容。

| 文件 | 内容 |
| --- | --- |
| `src-tauri/datasets/cortex-core-60/manifest.json`、`cases.json`、`case.schema.json`、`scoring-rules.json`（新增） | 60题审阅草案、明确标准和资料、固定顺序/分布/版本、逐题与文件hash、结构schema及六种规则规格；没有可执行评分引擎 |
| `src-tauri/src/benchmark/mod.rs`、`dataset.rs`、`preflight.rs`（新增） | 有界字段/规则/行号/hash校验；事务安装、数据库逐题读取及同版本不覆盖；独立模型参数、真实标签/加载快照、能力来源、指纹前后核对与错误分类 |
| `src-tauri/src/ollama.rs` | 提供内部只读get/show复用入口；既有聊天inspect保持同样请求、错误和本地格式检查，生成/NDJSON逻辑不改 |
| `src-tauri/src/commands.rs`、`lib.rs`、`build.rs`、`capabilities/default.json` | 实现并注册四个带workspaceId校验的业务命令；无任意HTTP、文件或shell权限 |
| `tests/benchmark-preflight.spec.ts`（新增）、`playwright.config.ts` | 可选的原生预检专项：受控服务及另行启用的实际模型元数据读取，不加入默认全套 |
| `docs/benchmark-preflight.md`、`benchmark-dataset-review.md`（新增） | 当前接口/数据/状态契约、限制及60题人工审阅表 |
| `docs/README.md`、`implementation-plan.md`、`test-plan.md`、`benchmark-implementation-plan.md`、`benchmark-evaluation-methodology.md`、`benchmark-provider-design.md`、`benchmark-database-schema.md`及本日志 | 对齐当前状态、验证范围和切片2停止点 |

只在`benchmark_datasets/cases`事务安装不可变内置定义；非法工作区不写入或访问模型服务。现有chat purpose/历史/知识/图谱接口、全局聊天参数及前端src不改。开发迁移仍0001–0008，0008与切片1字节相同，没有新增DDL所以没有新迁移备份；原备份失败/恢复结果仍是切片1的证据，不冒称本轮重跑。

### 实际命令和检查结果

- 开始时`git rev-parse --show-toplevel`确认`D:/vibecoding/CortexOS`，HEAD仍f146359；没有提交、推送、改标签或发布安装包。没有读取Gittoken.txt。
- 以标准库Python辅助编写固定草案，命令`py -3 artifacts/benchmark-slice2/author_dataset.py`、`write_review.py`。最终manifest SHA-256为`3595faf98b35865e00ba7f156ba89a21f6f5406f8f1b2908f41df031c3f882cc`，cases文件为`784089a5243bb3dc1cf4b723b351df19c5006ff983d97c3f4e8f9a946bc88806`。脚本及检查证据留本机artifacts，不把生成过程当独立人工审阅。
- `cargo check --lib`首次发现两个MutexGuard借用类型错误，改为明确借用连接后编译通过。`cargo fmt --all`完成；实际修改范围核对没有带入其他业务格式重写。
- `cargo test --lib benchmark_bundle -- --nocapture`仅运行必要数据专项，最终2/2通过，Rust编译6.62秒、用例0.13秒。覆盖60题分布/ID/hash、重复ID、实际类别替换、空标准、越界/未知资料行、JSON标准类型/工具名不匹配、规则版本/未知字段；真实SQLite中途插入失败整体回滚、重试/重复读取、重启、跨工作区拒绝、不可变保护、聊天bootstrap/消息和知识/运行表不变。先前几轮同一专项也通过；收尾针对新增边界再验，没有运行全库单元测试。
- `npm.cmd run typecheck`通过；`npm.cmd run tauri -- build --debug --no-bundle`最终通过，包含TS/Vite和Rust，最终Rust17.26秒、Vite708毫秒。主JS546.73kB/gzip165.16kB、图谱941.05kB，保留既有Vite大块提示及npm更新提示，不安装更新。中间构建同样通过；不生成NSIS/Release。
- 格式整理首次`npm.cmd exec -- prettier --write tests/benchmark-preflight.spec.ts`受沙箱npm缓存权限限制（EPERM），缓存路径rg也被拒绝；未变更依赖。随后经require_escalated只读取已有缓存的prettier.cjs、用node整理该测试文件，成功，不下载或安装软件。
- 设置`CORTEXOS_BENCHMARK_PREFLIGHT=1`、`CORTEXOS_BENCHMARK_REAL_MODELS=1`，从apps/desktop运行`npx.cmd playwright test tests/benchmark-preflight.spec.ts --project=benchmark-preflight --output=../../artifacts/benchmark-slice2-native-final`：最终2/2通过，总5.8秒（受控2.1秒、实际元数据1.6秒）。是真实原生WebView2与SQLite，不是浏览器替身；受控模型服务验证未安装、缺失/不固定digest、未知/不支持能力、上下文超限、远程/未知格式、检查期间替换、服务失败和ps失败；断言仅访问tags/version/ps/show且POST只有模型名，非法请求不访问服务，聊天bootstrap保持不变。初轮及中间原生检查也通过；最终规则文件标识调整后重新构建和复验，保存JSON证据文件。
- 实际服务Ollama0.40.6。读到qwen3:4b-instruct（digest `0edcdef34593eac1aa2be9c7d06c432dcf81945adca5eca2f27662c18f168ba0`）及qwen3:4b（`359d7dd4bcdab3d86b87d73ac27966f4dbb9f5efdfcc75d34a8764a09474fae7`），本次均通过身份/所需completion元数据检查，status为ready_for_probe。两者元数据声明context_length262144，未据此修改请求或声称8GB设备能运行该容量；本次检查参数为4096/512。工具/thinking为自报，JSON Schema保持unknown；没有能力生成探针、下载、预热、释放或模型答题。实际证据在`artifacts/benchmark-slice2-native-final/.../actual-local-model-metadata.json`，受控请求在同轮controlled-preflight.json。
- 对照Ollama官方tags/show/ps文档，字段链接写入预检契约；缺字段返回未知，不根据Qwen品牌填默认能力。未使用第三方兼容承诺。
- `py -3 artifacts/benchmark-slice2/check_scope.py`通过：0001–0007与HEAD逐字节相等、0008与切片1hash一致、迁移仍8份，两个依赖清单/锁文件及前端src不变，六份相关文档15个本地链接可定位。证据`artifacts/benchmark-slice2/scope-check.json`。`git diff --check`通过，既有CRLF提示保留。

### 验收边界和下一步

本轮具备题目可读取、模型可预检的后端切片，没有评测入口；未改视觉，所以没有新截图或缩放结论。既有聊天/知识/检索/来源/执行图五项原生业务结果沿用切片1，本轮没有重跑全套；后续提取共享执行时须专项再验。两条实际Qwen记录只是本机两个不同digest，不是跨品牌质量比较。

60题是AI辅助起草的needs_human_review版本1；格式/引用/示例自洽检查不能代替独立人工事实、计算和断言审阅。标准、材料、模板或审阅状态需要修改时另增版本。规则文件仅标记为scoring_specification，程序实现状态与不可变数据分开；没有评分、Docker隔离或性能/质量结果，不能声称完整60题验收通过。模型自报能力仍需实际验证，外部标签在预检后仍可能改变，后续Runner应逐次复核。

停在第2/8切片，等待用户验收。下一步为第3片：共享模型占用及少量明确标记的开发题真实执行闭环；未经确认不进入该片。

## 2026-10-23：Benchmark Phase 1切片1——数据隔离与迁移

用户确认Phase 0及三项推荐，只授权实施八片计划中的第一片，完成后等待验收。复核原总纲、AGENTS、开发标准、当前实施计划及benchmark计划/数据库方案，沿用ponytail的小范围复用。按用户本轮明确要求编写并运行必要数据库测试和现有兼容性专项，未跑单元全套或全部UI套件。没有进入题目/模型预检、Runner、Docker执行或云端阶段。

### 实际修改

| 文件（应用路径相对apps/desktop） | 内容 |
| --- | --- |
| `src-tauri/migrations/0008_benchmark_foundation.sql`（新增） | conversations增加不可改用途；10张数据集/配置/实验/模型快照/逻辑项/尝试/评分/资源/事件基础表；作用域复合外键、必要唯一索引、JSON/状态及不可变保护；阻断评测输出成为生产知识、评测使用生产上下文/检索及改聊天检索设置 |
| `src-tauri/src/db.rs` | 增量登记0008；迁移原算法提取私有migrate以测试真实失败；创建会话显式列名；普通会话、消息、改名、运行、检查点/事件/终态及bootstrap限定chat；终态检查整组运行身份；启动恢复保留两种用途并对账实验/尝试，不重发不评分 |
| `src-tauri/src/execution.rs` | 普通执行视角写入也检查chat用途；读取沿用已限定的run/messages，阻止评测进入旧“返回对话”路径 |
| `src-tauri/src/knowledge.rs`、`retrieval.rs` | 评测消息不可作为生产知识起源；已存在会话的检索设置读写必须为chat；新建聊天预览保留原默认检索语义 |
| `src-tauri/src/db/benchmark_foundation_tests.rs`（新增） | 8项真实SQLite检查；旧7版合成库含消息、运行/事件、请求、已确认/草稿知识、审批、关系历史、来源版本与行证据、FTS、位置/视角和设置；逐表内容指纹与旧checksum比较；失败备份恢复及用途/跨域约束 |
| `tests/windows-delivery.spec.ts` | 开发版当前迁移终版本期望由7改8，未重跑NSIS安装检查；历史6版输入样本保留 |
| `docs/benchmark-prd.md`、`benchmark-architecture.md`、`benchmark-database-schema.md`、`benchmark-implementation-plan.md` | 已确认决策、切片1实际状态和剩余范围；说明activeTask/共享占用在实际Runner切片接入，专用评测查询/导出仍在后续切片 |
| `docs/database-schema.md`、`README.md`、`implementation-plan.md`、`test-plan.md`、`milestone-log.md` | 当前开发schema8、已发布0.2.0仍schema7、检查入口与本轮停止点 |

0001–0007、模型Provider、权限、前端界面、依赖和锁文件均无本轮修改。原有desktop.spec.ts状态、reference素材及上一轮未提交设计稿保留。没有提交/推送或替换公开安装包，也没有打开正式用户数据库。

### 实际命令和结果

Git检查从独立仓库根目录执行；cargo/rustfmt在apps/desktop/src-tauri；npm/Playwright在apps/desktop。

- `git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS。定向检索全部会话/运行读写及知识起源调用，发现并修正无列名会话INSERT。旧迁移和锁文件git diff为空。
- `rustfmt --edition 2024 src/db.rs src/db/benchmark_foundation_tests.rs src/execution.rs src/knowledge.rs src/retrieval.rs`格式检查与整理，后续仅对改动文件执行。
- 首次`cargo test --lib benchmark_foundation_tests -- --nocapture`编译发现对临时MutexGuard取引用的类型错误；改为限定块内持锁校验，并在终态事务前释放，未增加新的锁层。修正后8项通过。
- 追加“所有表在拒绝访问前后相等”、错误消息身份拒绝、未创建会话的知识预览及真实备份恢复检查后，最终同命令8/8通过，测试执行0.24秒、最后编译7.19秒。实际输出保存在artifacts/benchmark-slice1/foundation-tests-final.log。7→8升级旧表/旧checksum逐项一致；迁移前备份为真实7版库，PRAGMA完整性和外键检查通过；故意在0008末尾插入不存在表触发中途失败，DDL/数据回滚，原库仍7版且可重试。备份复制到另一个隔离目录后可重新打开、升级并读回相同旧数据。升级对照证据在各upgrade-UUID/upgrade-evidence.json。
- 备份失败用不存在的备份目标目录触发实际VACUUM INTO写失败，确认未执行新DDL且仍7版，随后恢复可写位置重试通过；不声称模拟了所有磁盘耗尽/ACL故障。checksum篡改和旧迁移集合读取8版也会在变更前拒绝。
- `cargo test --lib database_isolates_workspaces_preserves_partial_and_recovers_after_restart -- --nocapture`初次命中旧用例只预期新增1事件，但既有引用协议会追加citations_checked及run_interrupted共2条。改为精确校验新增事件序列，保留末事件、部分输出、迟到完成不覆盖等断言，不改产品事件语义。最终1/1通过，0.05秒，证据chat-compatibility-final.log。
- `npm.cmd run tauri -- build --debug --no-bundle`两次通过，均包含TypeScript/Vite和实际Windows Rust编译，Rust分别22.61秒、10.43秒。最终主JS546.73kB/gzip165.16kB、图谱941.05kB，与原UI相同，保留既有大块提示。没有构建或更新Release/NSIS包。
- `npx.cmd playwright test tests/chat.spec.ts tests/knowledge.spec.ts tests/retrieval.spec.ts tests/trace.spec.ts tests/sources.spec.ts --project=desktop --grep 'native chat preserves|knowledge drafts|automatic retrieval|execution graph|authorized snapshots' --output=../../artifacts/benchmark-slice1-native`：首轮4通过、来源专项失败，原因是强化现有会话用途校验后误拦conversationId=null的新聊天知识预览。修正prepare_context_run的已验证工作区新聊天预览分支，并在新增Rust用途测试中覆盖该合法路径及非法工作区，保持已有评测会话拒绝。
- 最终相同五项原生命令，输出目录`../../artifacts/benchmark-slice1-native-final`：5/5通过、约1.2分钟；聊天39.3秒、知识9.3秒、检索4.7秒、来源6.3秒、执行图9.4秒。是真实WebView2和SQLite下的取消/断流/重启、草稿审批/冲突、来源停用、请求/版本与执行图定位检查，模型为受控本地HTTP替身；不是本轮真实Qwen、Docker或新评测UI验收。
- `git diff --check`通过（既有CRLF提示保留）。本轮无前端视觉变更，不新增截图相似度或系统缩放结论。所有新运行、库和恢复文件都是隔离合成检查产物。
- 收尾标准库Python脚本逐字节对照`git show HEAD:...`确认0001–0007与HEAD完全一致，检查六份benchmark文档及21个本地链接通过，迁移共8份；0008的SHA-256为`264e7619afdfceb4606b496321abf208019d09405dea51573d0db378fdf4644b`。前端、Ollama Provider、权限和两个锁文件的差异为空。

### 边界与下一步

已完成数据基础及现有公开接口的用途隔离，尚无评测入口/业务写入命令、题目发布校验、模型预检、共享Runner、自动评分和导出功能。启动恢复只是保存实际中断及已有完成状态，不自动续跑。已发布0.2.0仍schema7，开发库升级到8后旧版应拒绝直接打开；通用恢复UI未提供，本轮备份恢复只在隔离目录验证。

本轮停在切片1等待用户验收，不执行切片2。获确认后才实现“题目与模型预检”：实际Ollama元数据/能力与固定题目版本框架；仍不提前接云端、计费或Key管理。

## 2026-10-23：Benchmark Studio Phase 0审计与架构规划

用户粘贴Benchmark Studio总要求，并明确本轮只执行Phase 0、生成六份设计文档后停止。本轮读取该授权附件、development.md、开发规范/实施计划、当前HTML UI规范、架构/数据契约/安全文档，检查实际前后端与0001–0007迁移。未找到design.md，采用现行docs/design/html-prototype-ui.md和实际Tokens。使用ponytail确定最小接入范围、positive-spec-writing整理可执行规范、ui-ux-pro-max定向查询比较图/表，保持现有视觉体系。

### 交付文件与主要发现

新增benchmark-prd.md、benchmark-architecture.md、benchmark-provider-design.md、benchmark-database-schema.md、benchmark-evaluation-methodology.md、benchmark-implementation-plan.md；同步docs/README.md入口和原实施计划。本轮没有Benchmark业务文件、SQL迁移、依赖/锁文件或UI修改，没有提交/推送或变更已发布版本。

现有只有本地Ollama；会话历史和已确认知识上下文不等于独立长期记忆；FTS5是词法检索，引用检查只判断编号/位置。执行run与会话/消息绑定，首版建议复用既有事实，新增purpose隔离实验会话和评测排程/尝试/评分。统一共享模型占用，避免新Runner绕过聊天并发保护。云端接入还需版本化运行配置解析和快照阅读器，不能直接把云参数塞进ModelSettings。

末轮核对发现db.rs:292的会话写入没有列名，加purpose列时必须同步改显式列名；bootstrap.activeRun、执行图返回对话及按“本机模型”写死的事件解释也有用途/Provider假设，已写入接入与兼容门槛。原逐题state.clear不能用于释放整实验占用。上述均只写设计，未修改产品代码。

候选0008及后续迁移只做设计，当前仍7份SQL。Phase 1计划明确保留取消、重启已保存结果、手动续跑和基础JSON/CSV导出；固定60题中的10道代码题没有隔离环境就标未运行。Phase 3知识版本对照使用独立快照/索引，不修改生产生效版本。实际表、接口、评分分母、费用未知责任和验收边界见六份文档。

### 实际检查与证据

- `git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS，HEAD=f146359。git status保留原desktop.spec.ts状态和reference素材；对src、Rust、migrations、package及Cargo清单/锁文件的git diff为空。
- `rg --files`及定向`rg -n`核对模块、命令、路由和表。首次按常见名尝试api.ts/model.rs未找到，实际是studio.ts/models.rs；PowerShell中将SQL通配符直接作路径被拒，改成目录加`-g '*.sql'`完成检查，没有文件写入副作用。
- `npm.cmd run build`（apps/desktop）通过，包含`tsc --noEmit`及Vite。主JS546.73kB/gzip165.16kB、图谱941.05kB，保留既有大块提示；Vite构建869ms。没有Rust业务变更，本轮不重跑原生编译、单元/全套或界面专项；构建通过不代表Benchmark已实现。
- PowerShell只读查询`GET http://127.0.0.1:11434/api/version`、`/api/tags`、`/api/ps`和两个已安装标签的`POST /api/show`；只输出受限元数据，不保存模型模板或用户内容。Ollama0.40.6，两个Qwen3 4B标签均为GGUF/Q4_K_M，自报tools/thinking/completion；本次ps为空。没有发问答、卸载、预热或下载；模型标签/digest/能力未通过生成探针实测。
- `nvidia-smi --query-gpu=name,memory.total,memory.used,driver_version --format=csv,noheader,nounits`：RTX4070 Laptop、8188MiB、当前711MiB、驱动610.74，仅整卡单次读数。Get-Command显示本shell PATH没有ollama/docker命令，不据此宣称未安装软件。证据artifacts/benchmark-phase0/environment.json，采集UTC 2026-10-23T03:31:37.9621635Z。
- `py -3 D:/CodexData/skills/ui-ux-pro-max/scripts/search.py "comparison table missing data" --domain chart`返回类别比较、直接标签及可访问表格建议；选择表格为主，不采用其新配色或额外图表依赖。
- 联网核查Ollama、OpenAI、Anthropic、Gemini、Windows Credential Manager、Docker和NVIDIA的官方文档，具体链接留在设计稿。Ollama旧show文档地址404，改用官方导航的新地址；两个API大页抓取受大小限制，改查较小官方指南。未配置或调用云API，没有费用，也没有承诺未实测兼容性。
- `py -3 artifacts/benchmark-phase0/check-documents.py`检查六份文档存在、Phase 0标记、本地链接、JSON示例和尾部空白，通过；并确认迁移仍7份。结果为本地document-check.json。这是文档一致性检查，不是单元、模型或产品功能测试。`git diff --check`通过；保留原有CRLF提示。

停在Phase 0。待用户决定执行记录用途隔离、代码题隔离环境及首轮模型/Provider顺序后，才进入Phase 1。模型下载、系统安装、数据库升级、业务代码及付费测试均未执行；既有150%用户验收不自动覆盖未来评测页面。

## 2026-10-23：用户完成150%系统缩放验收

用户反馈：“150缩放我验证了所有功能没问题”。按本次v0.2.0的实际使用反馈，将Windows150%系统缩放记为用户验收通过，所有功能正常；不虚构截图、窗口尺寸或自动化结果。真实读屏、干净系统安装和长期真实模型等其他检查仍保留原状态，未据此将全部桌面交付门槛标为完成。

同步CHANGELOG、0.2.0发布说明、Windows交付记录及人工表、路线、产品范围、实施计划、测试计划和当前HTML界面规范。历史各轮“未测/待验”保留当时事实，当前结果在各入口补充；产品范围日期和版本同时对齐0.2.0。只更新文档，不重建安装包、不移动版本标签。

实际执行`git rev-parse --show-toplevel`确认独立仓库，`git fetch origin`及`git ls-remote`确认main和发布分支为96fed32、v0.2.0仍指向8d7e56a；`git diff --check`通过。`py -3 artifacts/release-v0.2.0/github-release.py update-notes`已更新GitHub发布说明并重新读取确认：正文与本地发布文档一致，发布ID、预发布状态、标签、目标提交及两份附件ID/大小/校验值均未变。结果保存于本地artifacts/release-v0.2.0/notes-update.json。此次按纯文档变更审阅事实、链接和差异，未运行单元、构建或界面测试；仅提交这九份文档并正常推送，保留原有desktop.spec.ts状态及reference素材。

## 2026-10-23：v0.2.0 公开预发布

用户要求“先这样吧，release一个0.2.0”，次日继续。发布基于已完成的HTML界面和易用性优化，仅同步版本、发布文档与安装检查；无Rust业务源码、数据库迁移、权限或依赖调整。npm包与锁文件、Cargo包与锁文件、Tauri配置及设置页版本统一为0.2.0，正式标识仍为com.cortexos.desktop。README保留能力与用途介绍，Qwen仅作为示例，可使用其他兼容的Ollama本地模型。远端核实已更名为公开的marksimZ/CortexOS，沿用仓库配置的Git Credential Manager，不读取或输出Gittoken.txt。保留用户reference素材及原有desktop.spec.ts工作区状态。

### 实际命令和结果

以下npm/Playwright命令在apps/desktop运行，其余从独立仓库根目录运行。构建和安装检查在10月22日执行，正式窗口复核及发布收尾在10月23日完成；不重跑单元或全量测试。

- `git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS；`git fetch origin`与`git ls-remote origin refs/heads/main refs/heads/codex/html-prototype-ui refs/tags/v0.2.0`确认main为e5bef65、界面分支为8877af4，尚无0.2.0标签。GitHub API核实仓库公开、有push权限，既有0.1.0资产保持不变。
- `npm.cmd version 0.2.0 --no-git-tag-version`同步npm版本，Cargo/Tauri版本显式更新；锁文件只有根包版本变化。
- `npm.cmd run tauri -- build --bundles nsis`通过类型检查、Vite、Rust优化构建与NSIS打包，Rust约3分11秒。正式exe的ProductVersion=0.2.0；包复制至artifacts/release-v0.2.0。安装包2,948,478字节，SHA-256为`b3a18e39b590c2615b024293e172e434949b263daf37cd7d122ff8c71a9ed1eb`，Authenticode为NotSigned。SHA256SUMS.txt与manifest.json记录一致。保留原有Vite大块提示和两处unused_mut提示，未顺带调整产品代码。
- 顺序执行`npm.cmd run tauri -- build --debug --bundles nsis --config tests/fixtures/delivery-previous.json`和`--config tests/fixtures/delivery-current.json`，Rust分别约36.80秒和18.49秒；独立检查产品的版本为0.1.0和0.2.0，来自同一份当前源码，不是历史正式包。
- 设置`CORTEXOS_DELIVERY_CHECK=1`后执行`npx.cmd playwright test tests/windows-delivery.spec.ts --project=delivery --output=../../artifacts/release-v0.2.0/install-check`，1/1通过、24.8秒。实际安装版本匹配，升级前后逐表记录及图谱相等；合成6版库迁移到7版及备份检查通过；卸载退出0、程序移除、检查数据保留。已安装窗口和可访问树检查通过，当前系统125%，不冒称150%或真实读屏。详情见delivery-evidence.json。
- 正式Release启动复核最初通过Process.MainWindowHandle找到了Tao辅助窗口，关闭结果不可靠；仅清理本次拥有的无窗口探测进程。随后生产程序未开放CDP，30秒探测超时，没有新增生产调试入口。10月22日Windows UI Automation复核未执行，原因是自动审批服务用量限制，次日继续。
- 10月23日执行`powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File artifacts/release-v0.2.0/check-production-window.ps1`实际启动正式程序。首次UIA按钮名受Windows PowerShell的UTF-8无BOM读取影响未匹配；改用Unicode码点构造固定名称，带`-VerificationProcessId 26884`复核同一探测进程通过：Tauri Window存在、前端关闭按钮可找到、调用按钮后正常退出，未强制结束。证据production-window-evidence.json。使用单独WebView配置，不编辑正式工作区或发送模型问题；不声称完整生产UI自动化。
- `npm.cmd run tauri -- build --debug --no-bundle`恢复正常开发程序标识，类型检查和构建通过，Rust16.90秒。没有覆盖已生成正式包。主JS546.73kB/gzip165.16kB、图谱941.05kB，保留既有大小提示。
- 收尾GitHub辅助脚本第一次以系统`python`调用时命中Python2并报语法错误，未发送API请求；改为`py -3 artifacts/release-v0.2.0/github-release.py inspect`通过。凭据只在内存中使用，输出仅含公开版本和权限。

### 发布边界

此前UI专项最终浏览器3/3、原生4/4及最终菜单1/1通过，见下节；本轮仅复用证据及运行必要安装/启动验证。正式安装包尚未在干净系统上安装，历史0.1.0正式包升级、150%系统缩放、真实读屏与长期真实模型仍待人工验收。安装包无签名、不包含Ollama/模型、未启用自动更新。保留v0.1.0原标签和资产。

### 实际发布结果

- `git diff --check`和暂存差异检查通过；创建codex/release-0.2.0，仅暂存45个已授权界面/检查/版本/文档文件，不包含用户素材及既有desktop.spec.ts状态。提交`8d7e56ae7160dad77b031b7897ebef89359e754b`，注释标签v0.2.0指向该提交，本地manifest同步源码提交。
- 首次正常push因更名前的仓库路径凭据无法匹配而失败，远端未更改。核实Git配置仍为manager、useHttpPath=true、旧账号hfdsdfgr；辅助脚本`align-git-credential`用现有GCM凭据在内存中核实当前账号marksimZ，将当前仓库的用户名/路径对齐并交由GCM保存，不读取根目录token、不把凭据写入文件或URL。
- `git push --atomic origin HEAD:refs/heads/codex/release-0.2.0 HEAD:refs/heads/main refs/tags/v0.2.0`成功：main从e5bef65正常快进到8d7e56a，无强推；版本标签远端解引用一致。
- 顺序执行`py -3 artifacts/release-v0.2.0/github-release.py create`、`upload`、`publish`、`verify`：先创建草稿并上传两份附件，API大小与摘要匹配后公开为prerelease。发布ID420940620，北京时间2026-10-23 00:18:06；地址为[0.2.0发布页](https://github.com/marksimZ/CortexOS/releases/tag/v0.2.0)。没有修改0.1.0发布。
- 未登录公开下载两份附件复核成功。安装包2,948,478字节，摘要与上文一致；SHA256SUMS.txt为95字节、摘要`7eb00686628d53482ce3c1f8b2c3baffc1e6b29e0bc0bd6b8f5cc95d84e8e505`。发布摘要保存在本地artifacts/release-v0.2.0/release.json。
- 发布后仅补充本节及计划状态，再作独立文档提交并正常推送；不改安装包、不移动标签、不重跑已通过检查。本轮停在0.2.0发布门槛，后续产品开发等待用户指示。

## 2026-10-22：界面易用性优化

用户确认只读审计方案后授权实施。沿用Full Prototype视觉方向，依次完成面板滚动结构、依据栏操作与滚动条、共享选择菜单，停在本轮UI验收。使用frontend-design、ui-ux-pro-max和ponytail；定向检索focus not obscured用于核对固定区域与焦点关系，实际设计继续采用项目Tokens，不引入UI依赖。未运行单元或全量检查。

### 实际修改文件

以下应用路径相对`apps/desktop/`，本表包含本轮全部源码、检查和文档变更。

| 文件 | 变更 |
| --- | --- |
| `src/components/SelectMenu.tsx`（新增） | 保留真实select及原字段事件，以Popover顶层呈现统一选项；鼠标、方向键、Home/End/Enter/Esc/F4、键入匹配、选中勾选、禁用态、边界定位、外部关闭；不支持Popover时回退原控件 |
| `src/components/AppearanceControl.tsx`、`WorkspaceSwitcher.tsx` | 接共享菜单，主题/工作区状态及原回调不变 |
| `src/Settings.tsx`、`GraphMotionControl.tsx` | 模型/镜头设置接共享菜单；参数和相机动画语义不变 |
| `src/Chat.tsx`、`src/components/ChatEvidence.tsx` | 面板标题行右侧收起，聊天区右侧展开；桌面偏好与窄窗临时状态分离，关闭返回焦点，窄窗使用原生模态dialog |
| `src/components/CognitiveInspector.tsx`、`AnswerContent.tsx` | 处理记录与引用标题固定，正文独立滚动，保留实际记录、错误和引用定位 |
| `src/Knowledge.tsx` | 历史固定标题，Portal解除玻璃页面的层叠遮挡；知识编辑固定标题/保存区，版本与类型使用共享菜单，保留审核、冲突和草稿保护 |
| `src/Graph.tsx`、`ExecutionTrace.tsx` | 图谱与执行节点详情顶部操作固定；关系编辑固定标题/保存区；来源/筛选/节点/运行选择接共享菜单；不改相机或领域数据 |
| `src/SourceBrowser.tsx` | 文件预览/阅读固定总标题，预览确认/取消移至独立底部；文件版本接共享菜单；保留导入、快照、选段和停用操作 |
| `src/components/AppShell.tsx` | 窄窗导航固定标题和关闭入口 |
| `src/styles/tokens.css`、`controls.css`、`layout.css`、`features.css`、`motion.css` | 浅深滑块色，原生滚动条统一轨道/滑块/悬停；共享面板头/正文/操作区；稳定滚动占位及边界；长配置值换行；共享菜单与120ms轻反馈；保留减少动态效果规则 |
| `tests/ui-usability.spec.ts`、`ui-usability-native.spec.ts`（新增）及`playwright.config.ts` | 两项浏览器UI检查及一项原生完整易用性检查；使用隔离合成资料与真实界面，不以图片代替应用 |
| `tests/knowledge.spec.ts` | 旧检查改为通过文字节点选取Markdown片段，导航按现有语义定位，历史滚动指向新的正文容器；保留来源/审核/冲突/重启等断言 |
| `tests/prototype-ui.spec.ts` | 窄窗依据栏断言更新为真实dialog角色；本轮未重复运行此整项，等价窄窗开关/Esc由新增浏览器专项覆盖 |
| `tests/graph-preview.spec.ts` | 等待相机收敛后检测连续250ms空闲帧，保留静止零绘制断言；不改相机实现 |
| `docs/design/html-prototype-ui.md`、`docs/implementation-plan.md`、`docs/milestone-log.md` | 交互规范、当前验收门槛及实际结果 |

进入本轮前已有的`tests/desktop.spec.ts`修改和reference素材未覆盖。Git核对Rust源码、数据库迁移、模型调用、权限、依赖与锁文件均无本轮变更，没有发布、推送或替换v0.1.0资产。

### 实际命令和结果

命令在`apps/desktop`运行；git检查在独立仓库根目录运行。

- `git rev-parse --show-toplevel`：确认D:/vibecoding/CortexOS。`git status --short`保留既有改动；`git diff --check`通过（有CRLF规范化提示）。
- `npm.cmd run typecheck`：各阶段及最终通过。首次缺少执行详情X图标导入，补齐后通过；一次读取命令工作目录重复了apps/desktop前缀，没有写入文件。
- `npx.cmd playwright test tests/graph-preview.spec.ts --project=edge --grep "seven-node" --output=../../artifacts/ui-usability-stage1`：旧用例固定等待的空闲帧断言失败（115→139），随后新增专项实际展开长详情并断言正文scrollTop>0、顶部坐标保持不变。收尾改为等待收敛后仍严格检查连续250ms新增绘制0，不改变相机逻辑。
- Node/Playwright临时浏览器脚本：确认Edge155支持Popover，检查真实组件中的菜单指针/键盘选择与原select事件。首次在pointerdown打开会被同一手势的轻关闭逻辑立即关闭，改为click打开后通过；未引入模拟页面或额外依赖。
- `npx.cmd playwright test tests/ui-usability.spec.ts --project=edge --output=../../artifacts/ui-usability-stage2`：当时1项通过；后续增加菜单检查。`--output=../../artifacts/ui-usability-stage3`首次边界检查抓到缩放事件尚未完成的一帧旧坐标，改为等待实际菜单进入新视口后再量测。
- 最终`npx.cmd playwright test tests/ui-usability.spec.ts --project=edge --output=../../artifacts/ui-usability-browser-final2`：2/2通过，5.1秒。覆盖真实图谱长面板、依据栏位置和偏好、800px抽屉Esc/返回焦点、菜单鼠标/键盘/外部关闭/记忆、浅深主题与1366/1600/1920/375视口边界。
- 补充`npx.cmd playwright test tests/ui-usability.spec.ts tests/graph-preview.spec.ts --project=edge --grep "browser panel|browser themed|seven-node" --output=../../artifacts/ui-usability-browser-acceptance`：最终3/3通过，8.8秒，包括上述两项和图谱筛选/选择/静止零绘制/WebGL降级。期间修正共享菜单仅在打开时挂载，关闭后不留下会被原字段非精确getByLabel查询命中的隐藏列表；字段值和回调不变。
- `npm.cmd run build`：通过。`npm.cmd run tauri -- build --debug --no-bundle`前几次因用户可见的原调试进程11824占用标准exe而无法替换；已生成的deps独立产物复制到项目artifacts用于中间隔离检查。用户明确回复保存并关闭后，标准构建成功；最终Rust dev构建8.57秒。未强制终止用户窗口。
- 原生专项沙箱内首次CDP超时；正常Windows桌面权限下运行可连接。中间检查用例修正了合成资料超预算、内容未变化不新增版本、测试文件必须位于隔离data目录等输入，保留所有产品保护。知识旧用例的纯文本DOM和global-rail假设更新至现有Markdown/主导航，未删功能断言。
- 原生实际发现历史抽屉被顶栏层叠遮挡、关闭入口无法点击，改为Portal并复验；共享弹出列表与原字段同名会使getByLabel含糊，改为“字段名＋选项”，保持字段原名称。长表单通过已有可调整高度的textarea制造溢出；文件预览使用600px高度并等待原生缩放布局完成后量测固定操作区。
- 最终`npx.cmd playwright test tests/ui-usability-native.spec.ts tests/knowledge.spec.ts tests/trace.spec.ts tests/sources.spec.ts --project=desktop --grep "native long records|knowledge drafts|execution graph|authorized snapshots" --output=../../artifacts/ui-usability-acceptance`：4/4通过，32.3秒。知识10.0秒、来源6.2秒、执行图9.7秒、面板/菜单5.3秒。实际WebView2执行保存、读取、审核、冲突保护、停用、重启和跨页定位；模型服务为合成fixture，不声称真实模型性能验收。
- 最后菜单改为按需挂载后的`npx.cmd playwright test tests/ui-usability-native.spec.ts --project=desktop --output=../../artifacts/ui-usability-native-releasecheck`：1/1复核通过，6.6秒；只重跑受该修改影响的面板/菜单/资料界面专项，未重复其余已通过功能。

### 截图与验收边界

原生四项验收截图在`artifacts/ui-usability-acceptance`，最终程序菜单专项截图在`artifacts/ui-usability-native-releasecheck`；新增专项子目录均为`ui-usability-native-native-2d644-keep-all-controls-available-desktop`：浅深主题×三种目标尺寸的菜单/滚到底处理记录共12张，另有历史/编辑固定操作图；知识专项另有三种尺寸和375px，执行图含窄窗图。已查看代表性的深色1600记录、深色1366菜单、浅色历史与长编辑图，以及最终程序深色1366记录，标题/关闭保持可见，历史不再穿透显示顶栏，滚动条无原来的上下箭头。最终浏览器截图在ui-usability-browser-acceptance，区分内容视口、系统缩放与真实模型运行。

最终调试程序：`apps/desktop/src-tauri/target/debug/cortexos-desktop.exe`，SHA-256 `eeb70d436779df1a9580d7a9e5e16041505e7b9c88f28e359bdfa97edeed3e6d`；最后标准构建成功，Rust dev构建14.96秒（替代前次8.57秒产物）。前端主块546.73kB/gzip165.16kB、图谱块941.05kB；既有Vite大块提示仍在。未重做发布安装包；真实Windows150%缩放、读屏及长期真实模型负载未在本轮验证。完成本轮已授权UI切片，等待用户实际手感验收。

## 2026-10-22：Full Prototype HTML前端重构

用户明确要求用reference中的Full Prototype HTML彻底替换旧视觉，仅改前端、保留全部功能，并应用五项细节。本轮以design/html-prototype-ui.md替代V3；读取总纲/开发规范/计划、AGENTS及新HTML，使用frontend-design、ui-ux-pro-max与ponytail。技能通用搜索返回的Hero方案不适合已有桌面应用，实际配色、宽度、玻璃层级均以用户HTML为准；不引入外部字体或新依赖。reference HTML及用户其他参考图保持原样，未纳入提交。

### 实际变更

统一252px侧栏、720px对话与输入、336px依据栏，重排知识/图谱/设置；主题浅/深/跟随系统，暗色字重降低；双环输入框、微色阶、数字等宽、Grid自适应、balance/pretty换行、短弹性反馈和2500ms成功提示。新到片段按8个字素渐显，Intl.Segmenter保留组合字符，已显示片段DOM不反复挂载；Markdown原文与引用UTF-16协议不变。右侧依据只读取get_run的真实知识/版本/估算量，连线仅表示实际纳入请求，不搬入原型的模拟冲突、检索算法或数字。窗口拖动和图谱420ms/可打断规则保留。

新组件及逐文件职责见设计文档；删除9份旧覆盖CSS，统一为5份样式和Markdown入口。原页面事务/接口/状态管理复用；git diff核对src-tauri、package.json和package-lock.json无变化，无Rust源码、权限、迁移、模型请求或锁文件修改。App/Knowledge/Graph/Source仅接呈现成功提示，错误、未保存和历史状态保留。窄窗统一导航使用原生dialog，展开时只挂载一套侧栏控件，避免重复ID与工作区选择。

### 构建、参考与截图

所有命令从本独立仓库执行，构建/Playwright从apps/desktop。npm.cmd run typecheck首次通过；npm.cmd run dev提供loopback预览。Edge本地打开HTML时阻止外部字体请求，等待原型入场完成后截图四页；首轮截图过早处于动画空白，已重取且未作为最终对照。初次打印HTML遇宿主cp950无法输出字符，改UTF-8输出。一次过长内联PowerShell写入被Windows 206命令长度拒绝，未写入文件，改apply_patch分文件。

npm.cmd run tauri -- build --debug --no-bundle成功6次，分别Rust约15.05、15.99、12.87、11.43、21.75和16.70秒；补Intl.Segmenter时一次类型检查失败，tsconfig仅增加ES2022.Intl类型后恢复通过。最终类型/Vite/Windows调试构建通过，CSS43.33kB、主JS540.78kB/gzip163.35、图谱941.05kB；保留既有500kB警告，不称为已做包体优化。构建顺序执行，没有并行混用dist。

浏览器实际四页及深色首检无pageerror。Node/Playwright测量1600×900下参考side/cmp/ev与实际sidebar/composer/evidence均为252/720/336px，存artifacts/prototype-ui/layout-comparison.json。用真实原生24张图生成浅/深联系表并查看，另查看代表原图和375窄窗；不是设计图或静态背景冒充UI。最终构建的24张+2张窄窗图在prototype-ui-context-final目录，参考/联系表在prototype-ui目录。媒体查询仿真、CDP内容视口与真实Windows缩放分别报告，未改系统设置。

### 专项命令与修正记录

共同命令前缀为npx.cmd playwright test，后附下列文件、--project desktop、--grep（有时使用）及--output ../../artifacts/对应目录，均为隔离数据和可控HTTP服务，不是新真实Ollama验收：

- markdown-camera.spec.ts trace.spec.ts graph.spec.ts --grep "native Markdown|execution graph|native graph persists" → prototype-ui-regression：1通过2失败。图谱因旧构建导航名改变，已恢复原可访问名称；Markdown因右依据栏盖住错误关闭，修正层级与错误时偏移，后复验通过。
- prototype-ui.spec.ts markdown-camera.spec.ts graph.spec.ts sources.spec.ts --grep "HTML reference|native Markdown|native graph persists|authorized snapshots" → prototype-ui-acceptance：2通过2失败。新增检查原先假定自动命中3条，实际只有2条，改为UI明确勾选3条再校验，未篡改业务结果。图谱固定坐标落入面板，改elementFromPoint核对真实画布坐标，并补回新CSS中的列表开关显示规则。
- prototype-ui.spec.ts graph.spec.ts → prototype-ui-focused：1通过1失败；保存提示与状态同文导致定位歧义，提示改为“镜头与布局已保存”，视图状态保留。
- prototype-ui.spec.ts graph.spec.ts markdown-camera.spec.ts sources.spec.ts trace.spec.ts knowledge-layout.spec.ts chat-design.spec.ts --grep "HTML reference|native graph persists|native Markdown|native focus|authorized snapshots|execution graph|reference sidebar|native chat" → prototype-ui-final：实际7条，6通过，知识宽度因860.000061px舍入失败；上限检查允许1 CSS px舍入，未放宽实际860px样式。
- knowledge-layout.spec.ts chat-design.spec.ts → prototype-ui-reading-final：知识2.6s通过，对话仍用旧“打开会话列表”入口超时。改为新统一导航并验证ESC焦点返回，同时修正重复隐藏侧栏挂载问题。
- prototype-ui.spec.ts markdown-camera.spec.ts chat-design.spec.ts knowledge-layout.spec.ts --grep "HTML reference|native Markdown|reference chat|reference sidebar" → prototype-ui-delivery：3通过1失败，离线断言指向新设计隐藏的顶栏副本；改查设置页实际连接状态。chat-design.spec.ts → prototype-ui-chat-final：1条8.3s通过，总9.0s。
- context.spec.ts prototype-ui.spec.ts → prototype-ui-context-final：最终截图/主题/动画专项12.0s通过，上下文旧脚本漏列P1-B早已存在的citations_checked事件。context.spec.ts → prototype-ui-context-recheck / -confirmed：又发现旧“生效v2/预算/完整请求快照”等用词，依现有UI及追加事件对齐断言并增加10s动作等待；不改后端。context.spec.ts → prototype-ui-context-complete：1条4.0s通过，总4.8s，实际请求/生效版本变化、预览、模型检查失败和重启快照保留均通过。
- app.spec.ts --project edge → prototype-ui-browser-final：7条通过，总8.9s，覆盖主导航/返回/深链、键盘及1440、1280、1024、800、375宽度的可达性和无溢出。仅浏览器预览，不代替桌面结果。

最终不同的9项原生专项均有通过结果：图谱11.7s、知识阅读2.8s、Markdown4.2s、镜头10.2s、新UI12.0s、来源6.1s、执行图9.3s、对话8.3s、上下文4.0s。是按修改范围分次专项，不是一轮全量套件；未跑单元测试。新UI专项验证三知识确来自请求、渐显老片段DOM连续、主题记忆/跟随媒体查询、三尺寸浅深四页、800/375抽屉、2500ms复制提示、无pageerror。图谱/来源/执行专项继续核对版本、关系、视角、停用与跨页。

### 边界与停止点

保留正式Release窗口及数据，测试不访问正式库；不覆盖公开v0.1.0或改其标签。字体用系统回退，内容为真实中文数据，窗口/审核等额外入口和真实3D会与原型存在必要差异；没有像素相似率声明。系统150%、实际读屏、长时真实模型与超长流式性能仍未测。验收文档列明当前差异。本轮完成后停在UI人工验收，不推进Decision Lab。提交排除用户原有desktop.spec.ts与reference文件，正常manager推送；最终差异/链接及提交检查接下方。

最终静态复核：10份Markdown与50个本地链接、围栏和UTF-8通过；6个CSS入口均存在；git diff --check通过（仅既有CRLF提示），src-tauri/依赖清单和锁文件无差异，Gittoken.txt仍忽略。代码在codex/html-prototype-ui独立分支交付，正式v0.1.0及main不在本轮替换。最终调试程序为apps/desktop/src-tauri/target/debug/cortexos-desktop.exe；使用者退出旧窗口后可手动打开验收。未自动关闭用户窗口或改动正式数据库。

## 2026-10-22：README改为产品与用途导向

按用户反馈重新组织README，突出项目讨论、可复用知识、关系探索、回答依据及学习研究/软件开发/产品项目/写作/个人知识/工作资料六类用途；去掉镜头修复、构建验证和开发过程叙述，开发细节改由链接进入专门文档。同步入门指南，说明Qwen轻量模型仅为示例和初始默认值，可选择其他本机Ollama部署的兼容本地对话模型。根据ollama.rs实际检查保留本地GGUF、固定loopback接口、不接受云端别名等兼容边界，不把“其他本地模型”扩写成支持任意部署工具接口。仅修改文档，不修改功能、默认模型或发布资产，不运行单元测试。

检查：Python 3 标准库核对3份Markdown编码、代码围栏与11个本地链接，并确认README已移除镜头修复/420ms/类型检查等开发记录文字；全部通过。git diff --check通过，仅原有CRLF提示。提交仅包含README、入门指南和本条记录，用户原有测试与参考文件保持不变。

## 2026-10-22：公开仓库文档重整

用户告知仓库已公开并要求重写 README 等文档。通过现有凭据调用 GitHub API，确认 private=false / visibility=public、默认分支 main、v0.1.0 为已发布预发布版，license=null。沿用安装包、标签及功能源码，不修改数据库、引擎、UI 或模型逻辑。

重写根 README、桌面 README、PRD、架构和安全说明；新增文档索引、安装、使用、常见问题、路线、CONTRIBUTING、SECURITY 与 CHANGELOG，共13份主体文档。更新发布说明和Windows交付状态，给总纲、规范、实施/纠偏计划、数据库契约、历史审计和检查方法增加当前状态入口，保留原计划、原审计和历史证据。按源码核对默认模型/参数、实际导入范围、FTS检索对象、知识生效规则、不可变快照、Markdown及相机设置；不再把文件全文检索、Decision Lab和通用恢复写成已完成，也不擅自新增许可证。

实际检查：git rev-parse --show-toplevel / status --short / log / rev-list 与 git fetch origin 核对独立仓库及远程。main 相对当前已发布分支为0/28，没有分叉；按用户公开文档请求将默认分支快进到已发布功能与新文档，确保首页说明与默认克隆的代码一致。正常凭据使用manager，未读取或输出Gittoken.txt。浏览官方Ollama Windows、Tauri前置要求和Vite文档核对安装说明。Python 3 标准库检查首批22份Markdown的108个本地链接、UTF-8和围栏闭合，全部通过；git diff --check通过，仅原有desktop.spec.ts的CRLF提示。最终补记检查另记录下方。

本轮仅文档修改，按用户要求未运行单元测试、类型检查、构建或原生界面测试；未将旧测试冒称本轮新结果。已发布安装包和v0.1.0标签保持不变。原有desktop.spec.ts与未跟踪参考文件保持原样并排除提交。Release正文同步为公开仓库说明，二进制资产及其SHA-256不变。

最终结果：23份Markdown、108个本地链接、UTF-8与代码围栏检查通过，git diff --cached --check通过。文档主体提交f2d42fc，git push --atomic将main从65800c1快进到f2d42fc，并发布codex/public-docs-refresh；无强制推送。GitHub API逐字核对main上的README、文档索引、入门、CONTRIBUTING和SECURITY共5份文件与本地相等；更新并回读Release正文，确认两个资产的名称/大小/校验码及v0.1.0标签完全不变，public=true。分支与上游差异0/0，剩余改动仅用户原有文件。本条为上述实际结果补记。

## 2026-10-22：v0.1.0 私有仓库预发布

用户要求 release 一个版本。沿用已完成 Release/NSIS 构建及三项原生专项验证的 b7ad417，不修改业务代码、不重打同一安装包、不重复运行单元测试。新增 docs/releases/v0.1.0.md，补充 windows-delivery 发布入口；人工验收仍待完成，P1-C 第5项不改为全部完成。

实际命令与结果：git rev-parse --show-toplevel / status --short / remote -v / tag --list 核对独立仓库、用户未提交文件和标签；读取现有版本配置及交付 manifest；Get-FileHash -Algorithm SHA256 核对安装包。初次 python 使用宿主旧 Python 2 导致 urllib.request 导入失败，未发生网络写入；改用已安装的 py -3（3.13.7）。通过 Python 标准库 subprocess 调用 git credential fill，仅在内存使用现有凭据；urllib.request 只读确认仓库 private=true、无既有 Release。没有读取 Gittoken.txt，没有新增依赖。

py -3 内联发布程序先验证本地安装包 SHA-256、字节数和源提交，POST /releases 建立草稿，分别上传 CortexOS_0.1.0_x64-setup.exe 与 SHA256SUMS.txt，核对 GitHub 资产 size/state/digest 后 PATCH 发布。随后 GET Release/资产及 git/ref/tags/v0.1.0 确认 draft=false、prerelease=true，标签指向 b7ad41779f28649032ee31c51d66f5a06807f2d2。安装包 2,949,609 字节，SHA-256 1c067bed99cf0e22a738272a067a36eaad3d5f30fe682d09f6fa1d24825d3c23，与上轮交付完全相同。发布结果记录在本机 artifacts/release-v0.1.0/release.json。仓库保持私有，未启用自动更新，未操作正式应用数据和已打开的窗口。

Release：https://github.com/hfdsdfgr/CortexOS/releases/tag/v0.1.0 。发布说明明确未签名、不捆绑 Ollama/模型，以及系统150%、真实读屏、干净系统和长期模型验证边界。用户原有参考图与 desktop.spec.ts 改动不纳入本次发布记录提交。

## Phase 0：可运行应用壳与架构

日期：2026-10-15。状态：核心验收通过；人工补充项见下方。本节为 Phase 0 历史记录。

### 范围与验收

目标为检查实际主机环境、建立最小 Tauri 2 + React + TypeScript 应用、实现四页导航与明确占位状态、形成架构和数据语义决定。验收要求为真实桌面窗口可打开并导航、前端构建与 Rust 检查通过、未实现功能不产生假成功。

用户明确授权安装 MSVC Rust 工具链，安装后通过项目文件选择，保留原 GNU 全局默认。没有安装 Ollama、下载模型、创建数据库或接入 3D。

### 交付文件

- 根目录：`AGENTS.md`、`README.md`、`rust-toolchain.toml`、`.gitignore`。
- 应用：`apps/desktop/package.json`、`package-lock.json`、`tsconfig.json`、`vite.config.ts`、`index.html`、`README.md`、`.gitignore`。
- UI：`src/main.tsx`、`src/App.tsx`、`src/App.css`、`src/vite-env.d.ts`、`public/cortex.svg`，均相对应用目录。
- 桌面：`src-tauri/Cargo.toml`、`Cargo.lock`、`build.rs`、`src/main.rs`、`src/lib.rs`、`tauri.conf.json`、`capabilities/default.json`、`.gitignore`；图标为 `32x32.png`、`128x128.png`、`128x128@2x.png`、`icon.png`、`icon.ico`、`icon.icns`。
- 检查：`playwright.config.ts`、`tests/app.spec.ts`、`tests/desktop.spec.ts`。
- 文档：PRD、architecture、database-schema、security、environment、test-plan、三份 ADR、开发规范与实施计划更新，以及本记录。

官方模板的 greet 命令、opener 插件、样例资源和编辑器推荐已移除。未创建未来共享包、模型服务或空业务目录。完整提交清单可通过本阶段提交的 `git show --name-status` 查看。

### 实际执行的主要命令

以下应用命令从 `apps/desktop` 执行；初始化和根目录检查除外。

| 命令/检查 | 实际结果 |
| --- | --- |
| `node --version`、`npm --version`、`pnpm --version`、`rustc --version`、`cargo --version`、`git --version` | 已执行；具体版本见 environment.md |
| `rustup toolchain list`、`rustup target list --installed` | 初始仅 GNU，安装后 GNU 仍为 default，MSVC 为项目 active |
| `nvidia-smi --query-gpu=name,driver_version,memory.total,memory.used --format=csv,noheader` | RTX 4070 Laptop，8188 MiB，总纲硬件方向吻合 |
| Windows 注册表、vswhere、WebView2 固定目录、Ollama loopback 检查 | 编译环境已就绪；Ollama 服务不可用 |
| `rustup toolchain install stable-x86_64-pc-windows-msvc --profile minimal` | 用户授权后成功，rustc/cargo 1.99.0 |
| `npm.cmd exec --yes --package=create-tauri-app@4.7.5 -- create-tauri-app apps/desktop --template react-ts --manager npm --identifier com.cortexos.desktop --yes` | 从根目录成功生成模板，再精简配置 |
| `npm.cmd install --no-audit --no-fund` | 成功，生成 npm 锁文件 |
| `npm.cmd run typecheck` | 通过，包含 UI、配置与检查代码 |
| `npm.cmd run build` | 通过，生产资源生成 |
| `npm.cmd run tauri -- icon public/cortex.svg --output src-tauri/icons` | 成功；多余移动端/商店图标随后在核对目标目录后清理 |
| `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` | 从根目录首次通过，记录首次 1m14s；不是启动性能 |
| `cargo check --manifest-path src-tauri/Cargo.toml --locked` | 应用目录再次通过，锁文件有效 |
| `npm.cmd exec --yes --package=prettier@3.9.10 -- prettier --write 'src/**/*.{ts,tsx,css}' 'tests/*.ts' '*.ts' '*.json' 'src-tauri/*.json' 'src-tauri/capabilities/*.json'` | 格式化成功，未加入运行依赖；后续修改过的代码再次格式化 |
| `npm.cmd run tauri -- build --debug --no-bundle` | 通过，最终 exe 重新嵌入生产资源 |
| `npm.cmd run test:smoke -- --project=edge` | 初始浏览器检查 7/7 通过 |
| `$env:CORTEXOS_SMOKE_DEV='1'; npm.cmd run test:smoke -- --project=desktop` | 最终开发模式 1/1 通过，内部实际执行 `tauri dev --no-watch` |
| `npm.cmd run test:smoke` | 最终完整检查 8/8 通过，11.2s |
| `git diff --check` | 通过；提交前另检查忽略规则与凭据未进入暂存内容 |

### 实际发现与修正

1. PowerShell 的 npm 包装脚本未正确转发初始化工具参数，首轮报非终端错误；改用 `npm.cmd` 后生成成功。
2. 原生调试首次使用 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`，两次未连接成功。改为 debug-only 的已校验端口配置，并使用独立 WebView2 数据目录后，原生检查通过。release 不编译该测试入口。
3. 开发模式几次检查因 CSP 证据缺失失败：Vite 直接响应与 Tauri dev 代理后的响应不同。为 Vite 增加共用策略头，并将开发服务器策略检查与原生导航检查明确区分；生产构建仍必须从原生响应读取并核验 CSP，不能使用服务器回退掩盖失败。
4. 图标生成工具报告一份系统字体 malformed 警告，但项目 SVG 不含文字，图标文件成功生成。测试工具还报告 NO_COLOR/FORCE_COLOR 环境提示，不影响验收结果。

### 真实桌面结果

- 独立程序：`apps/desktop/src-tauri/target/debug/cortexos-desktop.exe`。
- 原生生产资源 URL：`http://tauri.localhost/#/settings`；开发 URL：`http://127.0.0.1:1420/#/settings`。
- 两种模式实际 WebView2 viewport：1440×900 CSS px，devicePixelRatio=1.25。
- 生产响应 CSP 含 `script-src 'self'`、Tauri 注入脚本哈希、`object-src 'none'`、`frame-src 'none'`；没有 unsafe-inline 脚本放宽。
- 四页原生导航通过，重载后未记录脚本或控制台错误。测试结束关闭其启动的进程树。
- 浏览器四页分别在 1440×900、1280×800、1024×768、800×600、375×812 验证，无页面级横向溢出；导航、页面标题、刷新、返回及键盘操作通过。
- 实际检查了桌面对话页，以及浏览器对话、设置和 375px 知识页截图，布局层级与操作入口清晰。最终截图副本在忽略目录 `artifacts/phase-0`，原始检查输出在 `apps/desktop/test-results`。
- 设计色计算：正文对白 16.22:1，次要文字对白 6.33:1，主按钮白字 6.36:1。输入边界进一步加深，避免只在纯白背景接近 3:1 下限。

### 限制与下一步

- Phase 0 没有模型或数据业务能力，所有相关入口已如实标注。没有真实回答、持久化或 3D 性能结果。
- Windows 150% 系统缩放、完整读屏体验、release 控制台行为、安装包和安装回归尚未验收。125% 是本机原生实测，其他 viewport 检查不冒充系统缩放测试。人工步骤见 test-plan.md。
- Ollama 不在 PATH/标准安装位置，loopback 服务不可用。Phase 1 的真实模型验收前需获得服务安装与模型下载的明确授权，或由用户提供已准备好的本机服务。
- 下一实施范围：工作区/会话 SQLite 最小迁移，Ollama 检测与流式对话、取消和错误恢复，从第一次请求记录实际执行事件。先完成真实对话与重启保存，再进入知识和图谱。

按总纲在 Phase 0 验收点停止，尚未执行 Phase 1。

## Phase 1：本地对话与保存

日期：2026-10-15 至 2026-10-16。状态：Phase 1 核心验收通过，包含真实模型与实际服务断连恢复；人工补充项见下方。

### 范围与授权

Phase 0 汇报后，用户授权继续实施，并明确允许安装官方 Ollama 与下载 qwen3:4b。本阶段实现工作区/会话、真实本地流式对话、停止、错误恢复、SQLite 保存及二维执行详情；知识编辑、文件导入、3D 和安装包仍属后续阶段。

### 实现

- `src-tauri/migrations/0001_initial.sql` 与 `src/db.rs`：版本/校验迁移、WAL、工作区复合外键、事务写入、250 ms 输出检查点、唯一活动运行、不可修改事件、独占数据目录和中断恢复。
- `src-tauri/src/models.rs`、`ollama.rs`、`commands.rs`：类型和参数校验、仅 loopback 的 reqwest、模型来源检查、NDJSON 流解析、取消、超时及按 run 的 Channel；命令通过 build.rs 与 capability 逐项授权。
- `src/studio.ts`、`Chat.tsx`、`Settings.tsx`、`App.tsx`、`App.css`：工作区选择与创建、会话列表及重命名、IME 安全发送、停止/重试、滚动保护、设置保存和执行时间线。界面沿用会话列表、阅读区、底部输入区与按需详情抽屉。
- `tests/native.ts`、`chat.spec.ts`、`ollama.spec.ts`：原生启动复用、独立数据库/浏览器配置、可控错误回归与真实模型验收分别运行。相关锁文件、文档及 ADR 0004 同步更新。

### 已执行检查

应用命令从 `apps/desktop` 执行：

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --manifest-path src-tauri/Cargo.toml` | 成功，使用项目 MSVC 的官方 rustfmt |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked` | 4/4 通过；涵盖隔离、迁移校验、恢复、事件、终态、上下文整轮预算与流分片 |
| `npm.cmd run tauri -- build --debug --no-bundle` | 通过，包含 TypeScript 与 Vite 生产构建 |
| `npm.cmd run test:smoke` | 9/9 通过，2026-10-16 最近一次 46.4 秒；7 项浏览器与 2 项真实桌面检查，含正常关窗、会话输入保护、输出上限与重试 |
| `npm.cmd run test:smoke -- --project=desktop tests/chat.spec.ts` | 补充检查 1/1 通过，39.7 秒；包含正常关窗保存与会话创建输入保护 |
| `$env:CORTEXOS_REAL_OLLAMA='1'; $env:CORTEXOS_MANAGED_OLLAMA_PID=<本任务服务 PID>; npm.cmd run test:smoke` | 最终 10/10 通过，Playwright 报告约 1.2 分钟；含 7 项浏览器、2 项原生与 1 项真实 Ollama 检查；结束清除测试变量 |

可控服务仅模拟模型接口；SQLite、IPC、原生 WebView2 与应用流程都是真实实现。完整回归覆盖停止后保存、工作区隔离、会话重命名、重试、中断恢复、离线/重连、30 秒超时、云端模型阻止及模型缺失。

### 发现与修正

1. 首次 Rust 编译发现 MutexGuard 临时借用类型不符，改用局部 guard 后通过。
2. 原生检查暴露了测试中的浮点严格比较、重复标签定位和 IPC 错误类型断言问题；分别改为近似比较、明确 textbox 角色和错误断言。
3. 流解析检查发现同一网络分片中的畸形 JSON 会丢弃前面的有效帧；改为按帧依次交付，再在错误处终止。整合检查已使用同一响应分片验证部分内容保存。
4. 上下文只纳入成功完整轮次，避免失败重试重复问题和截断后孤立的回答；小预算回归已覆盖。
5. 官方模型下载首次在约 178 MB 处遇远端超时，已续传；安装包哈希与签名核验、安装和 loopback/关闭云端检查已通过。
6. 真实模型停止检查最初读取到上一条消息，过早取消而得到空内容；明确等待第二条回答出现并有实际文字后，通过停止与重启保存检查。
7. 首轮机械验收虽通过，截图复核发现 `qwen3:4b` 当前实际为 Thinking 标签，`think:false` 仍输出分析式文字，默认 512 token 全部耗尽。该轮 2.396 秒出现文字、9.868 秒结束、512 输出 token、GPU 总用量 4246 MiB（含桌面）仅作为不合适默认模型的基线，不作为完整回答验收。已改用同系列 `qwen3:4b-instruct`；新增 `done_reason` 保存、截断提示与重试，真实模型验收要求以 `stop` 正常结束。
8. 新建会话后立即输入存在异步切换覆盖草稿的窗口；创建和加载期间短暂禁用输入并显示创建反馈，完成后恢复编辑。原生回归通过。用测试进程的 Windows `CloseMainWindow()` 验证正常关闭生成中的窗口，重启后 run 为 cancelled，部分回答存在；强制退出仍为 interrupted，二者分别验证。
9. 10 月 16 日继续时原服务和下载进程已结束，未确认分片的进度未持久化。恢复同一模型目录后重新下载；范围测速显示现有代理更快，转用官方 registry 的同一权重资源。网络会中断较大 TLS 响应，因此保留连续前缀、分段下载并核对 Content-Range；最终完整文件通过官方 SHA-256 后才交给 Ollama 登记。官方标签清单、下载与校验记录在本机 artifacts 中；没有修改系统网络配置。

### 真实模型与硬件结果

2026-10-16，真实 Windows WebView2、独立测试数据库及合成中文问题。`qwen3:4b-instruct` 的实际模型 ID 为 `0edcdef34593eac1aa2be9c7d06c432dcf81945adca5eca2f27662c18f168ba0`，4.0B、Q4_K_M，模型列表大小 2,497,293,803 字节；权重文件 SHA-256 为 `85e4a5b7b8ef0e48af0e8658f5aaab9c2324c76c1641493f4d1e25fce54b18b9`。

| 项目 | 本次实际结果 |
| --- | --- |
| UI 首段文字 / 完整回答 | 4444 ms / 5284 ms，包含界面与 IPC 等开销 |
| Ollama 总耗时 / 模型加载 | 4913 ms / 3970 ms |
| 实际输入 / 输出 | 91 / 52 token；`done_reason=stop`，完整返回三条中文建议 |
| 生成阶段速度 | 52 token / 0.816006 秒，约 63.7 token/s；不含模型加载 |
| 运行设备 | RTX 4070 Laptop，驱动 610.74，总显存 8188 MiB |
| 模型驻留 | `ollama ps` 报告 3.2 GB、100% GPU、上下文 4096 |
| GPU 总占用快照 | 4437 MiB，包含桌面和其他 GPU 使用，不能当作模型独占预算 |
| 停止与重启保存 | 第二次生成停止后保留 10 个字符；重开后完整与部分回答均一致 |
| 实际服务停止与恢复 | 停止本任务管理的服务后，应用保存 failed 并提示连接失败；恢复服务、重新检测后再次得到完整真实回答 |

这是模型未加载时一次请求的观测，磁盘及驱动缓存未清空，不是稳定基准。实际检查了 `artifacts/phase-1/native-real-chat.png` 和 `native-real-execution.png`：回答正文完整，主要操作集中于对话，执行时间线按需展开，没有将模型生成的分析文字当作执行依据。原始数值在同目录 `real-model-report.json`；测试使用独立数据库和 WebView2 配置，没有写入正式工作区。

### 限制与下一步

未验收的人工项目仍包括 Windows 150% 系统缩放、完整读屏、release 与安装包。未发送草稿仅在窗口内保存；强制退出只恢复已提交输出检查点。数据未加密，备份/导出与删除尚未提供。当前按最近完整轮次保守选择上下文，没有文件检索、知识编辑或 3D。

Phase 1 停在本阶段验收点。下一阶段先实现知识列表、详情与“回答片段 → 草案 → 用户确认 → 版本记录”的二维闭环，再连接图谱。


## UI 精简与工作台视觉调整 — 2026-10-16

范围：根据用户对小灰字、圆角和边框过多的反馈，调整现有对话、知识、图谱、设置页面。采用蓝黑顶部/主导航、浅灰蓝会话栏、白色内容画布；强调色集中在选择和主操作。减少区域框线，控件圆角为 2–3px，浮层为 4px；设置沿用开放式分组表单。

文案：移除页标题下的宣传说明、每条回答重复的无来源说明及重复保存提示。输入区快捷键/上下文规则、模型建议、隐私说明改用原生 details 按需展开。保留输入可访问标签、当前会话名称、生成状态、截断警告、错误、未保存修改与未开放标记。未改动后端、数据库或依赖。

实际命令与检查：

- `npm.cmd --prefix apps/desktop run build`：类型检查与 Vite 构建通过。
- 在 apps/desktop 执行 `npm.cmd run tauri -- build --debug --no-bundle`：独立 debug 桌面程序构建通过。
- `npm.cmd run test:smoke -- --project=edge`：7 个用例断言通过，但沙箱内结束清理挂起，手动终止；最终在正常桌面权限下运行 `npm.cmd run test:smoke -- --project=edge --output=test-results-ui`，7/7 通过，正常退出（7.5 秒）。覆盖四页、键盘、导航及 1440/1280/1024/800/375 宽度。
- `npm.cmd run test:smoke -- --project=desktop`：沙箱内首次 2 项因 CDP 超时/连接重置失败；相同命令在正常桌面权限下重跑 2/2 通过（38.3 秒）。原生 WebView2 1440×900、DPR 1.25；检查生产 CSP、导航和隔离 SQLite 的生成/停止/错误/重启恢复。聊天使用受控模型 fixture；本次未重跑真实模型性能测试。
- `git diff --check`：通过。

人工查看浏览器对话/设置、375px 对话及原生对话/错误恢复截图。截图归档在 `artifacts/ui-refresh/`，浏览器截图从临时输出目录移入 browser 子目录；原生对话和设置分别为 desktop-chat.png、desktop-settings.png。未进行 Windows 150% 缩放或完整读屏验收。

源码：App.css、App.tsx、Chat.tsx、Settings.tsx；规范与计划同步更新。本次停在现有 UI 调整验收点，Phase 2 尚未开始。


## Phase 2A：二维知识闭环 — 2026-10-16

状态：本切片验收通过。用户在 UI 调整后授权继续，依计划先交付二维知识管理，Phase 2B 的关系与 3D 尚未实现。

### 实现

- 回答可保存为知识，支持选取连续片段；未选择时取开头最多 20000 个 Unicode 字符。手动创建与来源回答创建均先保存为待确认草案。
- 知识页使用搜索列表、状态筛选、阅读详情和聚焦编辑器；可查看旧版本、对照当前生效版、原始摘录及原会话。沿用蓝黑工具栏和开放式内容布局。
- 新增不可变知识版本和审批记录；保存草案保持原生效版本。确认弹窗展示最新全文，用户明确确认后切换生效版本。并发 revision 冲突时编辑内容保留，用户读取最新内容并明确比较后才能重新保存。
- 编辑取消、Esc、正常关闭窗口提供保存/放弃/继续编辑；关闭前保存再重启可找回草案。编辑器挂载在应用根部，背景页面历史切换不会卸载未保存编辑。
- 0002 迁移前以 VACUUM INTO 创建同数据目录唯一备份；核验全部已有迁移的顺序与 checksum。来源由后端验证工作区、回答终态和原文连续片段，原始内容哈希由后端计算。
- 只增加四个业务命令及用于关闭保护的事件监听/取消监听/窗口关闭权限；无新增依赖、任意文件读取、网络或 shell 权限。没有授予强制销毁窗口权限。

主要源码：Knowledge.tsx、knowledge.rs、0002_knowledge.sql、db.rs 迁移器、commands.rs/权限注册、App/Chat 接入与语义 CSS。README、PRD、schema、架构、安全、检查计划同步更新。

### 实际检查

- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` 通过。
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --lib`：6/6 通过（最终 0.15 秒）；覆盖升级备份、历史不可变、来源/工作区隔离、草案与生效分离、审批并发、注入插入错误后的事务回滚和重启一致性，以及原有对话/流解析检查。
- `npm.cmd --prefix apps/desktop run build` 与 `npm.cmd --prefix apps/desktop run typecheck` 通过。
- 在 apps/desktop 执行 `npm.cmd run tauri -- build --debug --no-bundle` 通过，生成独立桌面程序。曾误从仓库根目录执行 npm 命令，因没有 package.json 失败，切换正确目录后通过。
- 新增 `npm.cmd run test:smoke -- --project=desktop knowledge.spec.ts`：首次因版本选择器的嵌套 label 包含选项文字导致定位等待超时，改为显式 htmlFor/id 绑定后 1/1 通过（7.4 秒）。初始 Rust 构建发现 rusqlite 不支持 usize SQL 值，改为有界 i64 迁移编号后通过。
- 最终正常桌面权限运行 `npm.cmd run test:smoke`：10/10 通过（49.1 秒）；7 个 Edge 用例、原生聊天生命周期、原生导航/CSP、原生知识闭环。知识闭环 4.5 秒，包含选区来源、审批、旧生效版保留、并发冲突比较、历史选择、来源回溯、隔离/筛选、放弃修改、正常关闭继续编辑、保存后关闭/重启和手动记录。
- 原生 WebView2 页面 1440×900、DPR 1.25；浏览器五种视口无横向溢出。知识详情/编辑额外模拟 375×812 viewport，通过并人工查看截图。不是 Windows 150% 系统缩放验收。
- `git diff --check` 通过。原生截图保存于 artifacts/phase-2a，包含草案/生效对照、确认后详情与窄窗口详情/编辑。

测试使用独立 SQLite、确定性合成回答和测试工作区；没有改写正式工作区。未重跑真实 Ollama 性能验收，本切片不涉及生成参数或模型服务变更。

### 限制与下一步

尚无关系编辑、3D、知识自动检索注入、撤销审批、通用备份恢复 UI 或删除。标题搜索在当前工作区已加载列表中完成；大量知识规模尚未基准测试。确认表示用户审核而非事实验证；原始摘录记录来源，不自动证明后续修改内容。未保存编辑在强制结束进程时仍可能丢失。完整读屏、Windows 150% 缩放和 release/安装包未验收。

下一切片为 Phase 2B：先定义关系修订事务与布局数据，再实现 7–20 节点的确定性演示图与真实知识图，复用当前检查器/编辑器；不将本切片标为整个 Phase 2 完成。


## Phase 2B：知识关系与 3D 探索 — 2026-10-16

状态：本阶段验收通过，Phase 2 完成；停在阶段门槛。用户授权继续后实现计划中的关系修订、图谱交互和视图保存。

### 已实现

- 当前工作区显示真实知识；独立的 7 节点确定性演示包含 6 种类型和 7 条方向关系，显式标记且不能写入工作区。
- 画布支持点选、双击/按钮聚焦、旋转/平移/缩放、适应全部节点；列表可通过键盘选中。按标题、类型、审核状态和本地更新日期筛选；详情保留被筛选隐藏的选中内容并说明状态。
- 复用知识详情、来源、历史、草案编辑和审核；从图谱修改保存后留在图谱。关系添加/修改/停用写入不可变历史，重复有效关系、自环、跨工作区和旧预期版本被拒绝；冲突可读取最新关系比较，自己的输入保留。
- 节点位置可在高级表单调整；相机/目标点/布局通过“保存视图”单独存储，带并发 revision。移动节点不新增知识版本，重启恢复同一视图；未保存视图在离开页面后可能丢失。
- 0003 迁移使用已有一致性备份机制。新增三个明确业务命令，没有新增系统权限。图谱代码懒加载，按需绘制，DPR 1–1.5，无自动旋转、后处理或在线素材。
- WebGL 创建失败或丢失时保留列表与知识编辑；窄窗口使用列表入口和详情覆盖层。三维执行图继续明确显示下一阶段入口。

主要变更：Graph.tsx、GraphViewport.tsx、graph-data.ts、graph.rs、0003_graph.sql、共享 KnowledgeInspector、App 接入与 CSS；依赖/锁文件、命令权限、原生/浏览器图谱检查及相关文档同步更新。临时 Prettier 3.9.10 仅用于整理当前变更代码，未加入项目依赖。

### 实际命令与结果

- npm registry 核对：three 0.186.1、@react-three/fiber 9.8.2、@react-three/drei 10.7.9 与 @types/three 0.186.0；使用 `npm.cmd install --save-exact ...` / `--save-dev --save-exact ...` 更新锁文件，安装报告 0 vulnerabilities。
- `npm.cmd --prefix apps/desktop run build`、`npm.cmd --prefix apps/desktop run typecheck` 通过。应用目录执行 `npm.cmd run tauri -- build --debug --no-bundle` 通过，生成包含生产资源的独立 debug exe。3D 块 932.70 kB，gzip 248.04 kB，Vite 的块体积提示仍保留。
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` 通过。
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --lib` 最终 8/8 通过（0.16 秒）。新增关系/视图事务与 v2→v3 升级检查：已有草案、生效版、审批记录及备份保持一致。
- 最初一次沙箱内 Edge 检查 7 个断言通过但结束清理挂起，终止后改用正常桌面权限。曾从应用目录执行根目录相对路径编辑脚本失败，回到仓库根目录后完成编辑，未产生部分代码更改。
- 第一次图谱专项 2 项失败，原因是 Html 标签的跨 React root 卸载引发 removeChild 异常；更换标签层后专项 2/2 通过（12.3 秒）。后续补验关系冲突比较与重开相机回采。
- 最终正常桌面权限 `npm.cmd run test:smoke`：12/12 通过（59.3 秒）；8 个 Edge 用例、4 个原生 WebView2 用例。既有对话、知识草案和生产 CSP 同时通过。原生内页 1440×900，DPR 1.25；375px 为视口模拟，不是 Windows 150% 缩放验收。
- `git diff --check` 通过。人工查看演示图、原生知识图和窄窗口详情截图；归档于 artifacts/phase-2b。

### 小图观测

| 项目 | 本次观测 |
| --- | --- |
| 浏览器 7 节点演示进入并出现首帧 | 651 ms |
| 原生 7 节点图谱进入并出现首帧 | 837 ms |
| 浏览器闲置 250 ms 帧计数增量 | 0 |
| 原生渲染设备 | ANGLE / NVIDIA GeForce RTX 4070 Laptop / Direct3D11 |
| 关系修订与知识版本 | 关系 4 次修订、知识 2 个版本；停用历史与确认均保留 |
| 视图恢复 | 重启后相机/目标点回采逐轴一致，位置与工作区隔离保持 |

原始 JSON 为 browser-observation.json、native-observation.json。载入计时从进入图谱页面开始，到 WebGL 首帧检查完成；缓存未清理，单次观测，不是稳定基准，也不代表整应用启动时间。数据为隔离测试工作区的合成内容，不访问正式工作区。

### 限制与下一步

当前验收小图，尚未验证 100/500/1000 节点、密集关系、真实模型生成与图谱并行负载或显存增量。关系历史查询逐边读取，保存视图最多 1000 个位置；这些上限不代表性能承诺。渲染依赖内部存在 THREE.Clock 弃用警告，未屏蔽警告。完整读屏、Windows 150% 缩放、release 与安装包仍未验证。

关系指向稳定节点，内容修订后需用户自行复查关系；知识仍未自动加入模型上下文。下一阶段 Phase 3 先将实际运行事件投影为独立执行图，再测大图及模型并行性能，不混入模型内部思维。
# 2026-10-18 产品方向纠偏审计（文档交付，待用户确认）

基线 `70ea24c`。用户要求仅审计与规划，本轮新增 `gap-analysis.md`、`cognitive-architecture-v2.md`、`graph-experience-v2.md`、`refactoring-plan.md`；保留既有实施历史，未修改运行代码、迁移、依赖、模型或用户数据库。核心发现：真实运行/事件与数据库知识图已存在，知识未进入模型上下文；执行图占位，完整上下文快照和统一实体导航缺失。建议确认后从 P0-A 知识上下文与快照切片开始。

实际检查：`git rev-parse --show-toplevel` 返回本独立仓库；`git rev-parse --short HEAD` 返回 `70ea24c`；`git status --short` 初始干净。通过 `Get-Content`、`Select-Object` 和 `rg -n` 核对 development.md、开发规范/计划、三份 SQL、Rust 数据/模型/命令/provider/知识/图服务、前端状态/页面/画布和测试入口。`rg -n 'ContextSnapshot|ExecutionStep|execution_steps|context_snapshots|retrieval_results|evaluation_cases|CREATE VIRTUAL|workflow' apps/desktop/src apps/desktop/src-tauri/src apps/desktop/src-tauri/migrations` 无匹配；结合实际调用链判断缺口，而非仅凭名称。

使用 ui-ux-pro-max、positive-spec-writing 技能。实际 `python D:/CodexData/skills/ui-ux-pro-max/scripts/search.py 'deep linking predictable back' --domain ux -n 3` 因默认 Python 不支持脚本语法失败；改为 `py -3` 同命令成功，返回 Deep Linking 和移动平台 Back Button 建议，适用范围记录在交互规范。一次包含 Bash 花括号路径的 rg 命令被 PowerShell 拒绝，随后改用逐个明确路径；一次测试路径通配符未被 rg 展开，改为搜索 tests 目录。均为只读检查，无数据变更。

文档检查：使用 `py -3 -` 内联标准库脚本，验证四文件 UTF-8 可读、相对 Markdown 链接目标存在、代码围栏配对、无行尾空白，四份通过；`git diff --check` 通过。另核对 package.json 及审计中关键符号行号。未运行 Rust/Playwright/桌面构建、原生 UI 或真实 Ollama，本轮不新增功能/性能验收成绩；现有成绩仍以以下历史记录为准。停止于审计交付，等待用户确认，不启动 P0。


# 2026-10-18 P0-A：已确认知识进入真实请求与上下文快照

用户在审计交付后明确“执行吧”，本轮按 P0-A 切片实施，停在 P0-A 门槛。未启动执行图、跨页实体导航、自动检索、工作流或评估。复用原服务/编辑器/事务和依赖；原审计四份文档同时纳入仓库。

## 变更与实现细化

- 新增 0004_context_audit：完整 request/manifest/哈希快照与有外键/工作区触发器的知识版本引用；不可变 UPDATE/DELETE 保护。旧库迁移自动一致性备份，旧 run snapshot:null。
- prepare_context_run 解析已确认 knowledge/context/decision，最多20项，固定生效版本、内容、哈希与理由，记录历史消息排除原因；同一构造对象保存后交给 provider。事务保护输入、输出占位、运行、快照和事件；写入失败回滚且不请求模型。
- preview_context 为只读命令（业务命令总数19）；start_chat 增加可选 contextSelection；get_run 复用返回 snapshot。UI 折叠选择/预览、版本变化保留问题、查看本次知识正文与完整消息快照。选择按当前进程工作区/会话保留，重启需重选；运行依据永久保存。
- 模型检查开始/完成、请求尝试、收到响应分别追加事实事件；旧 model_called 保留，只调整解释文案。没有添加虚构检索/验证事件。执行图仍占位。
- 与最初方案的细化：同步校验和预算失败留在发送前；运行与上下文原子提交，未提前保存空运行。没有单独 get_context_snapshot 命令、通用 context item 多态框架或模型权重固定；详见 ADR 0006。接收/准备事件记录持久化时间，不当作组装耗时。

## 实际命令与结果

- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml`，最终 `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`：通过。
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --lib`：最终 **10/10**，0.22秒。新增测试覆盖预览不写库、v1生效/v2草案、审批并发、跨工作区/重复/超预算、请求快照及哈希一致、不可变、写入失败回滚、重启、v3迁移备份和外键完整性。8项历史测试继续通过。
- apps/desktop 中 `npm.cmd run tauri -- build --debug --no-bundle`：独立桌面构建成功。后续纯格式化后 `npm.cmd run build` 再次通过，产物哈希与原生测试版本一致：index-B4W8iVML.js，GraphViewport-xvJ1wQ5P.js。没有新增依赖；保留约932.70kB图模块告警。
- `npx.cmd playwright test context.spec.ts --project desktop`：原生新闭环 **1/1**，7.1秒；HTTP捕获与快照等值、审批冲突保留输入、v2实际使用、模型检查失败不发送chat、重启原快照一致。
- `npx.cmd playwright test`：浏览器/原生回归 **13/13**，约1.1分钟。包含5种窗口尺寸、键盘/导航、图降级、聊天取消/异常/关闭、知识和关系修订。使用独立测试目录，正常桌面权限执行，不读取正式工作区数据。原生测试窗口1440×900、DPR1.25。
- Ollama 初始未运行；默认沙箱执行已安装程序被拒绝访问，提升至正常桌面权限后按既有授权使用 `Start-Process -WindowStyle Hidden` 启动官方 serve（PID8220），仅绑定127.0.0.1，进程环境禁用cloud且单生成，未改全局环境、未下载模型。`Invoke-RestMethod` 检查0.40.4与既有模型。
- `$env:CORTEXOS_REAL_CONTEXT='1'; npx.cmd playwright test --project real-context`（在 PowerShell 中分行执行）：真实本地模型 **1/1**，9.1秒。qwen3:4b-instruct 完成；真实回答为“验收项目代号为青鹭-742，发布时间是周四。”，使用知识v1且重启后快照一致。观察wallMs4124包含UI检查/截图开销，不当作稳定生成性能基准。服务保持运行供用户使用。
- `npm.cmd exec --yes --package=prettier@3.9.10 -- prettier --write src/ContextPicker.tsx src/studio.ts src/Chat.tsx tests/context.spec.ts tests/context-real.spec.ts playwright.config.ts`：格式化成功，未增加项目依赖。`git diff --check`：通过。

## 调试中实际失败与恢复

Python默认cp950读取中文失败后改用 `py -3 -X utf8 -`；首次 Rust 测试暴露旧 PreparedRun.messages 引用，改为读取 request.messages。拆分 inspect 时格式化后的签名未匹配导致重复chat，修正后通过。新增迁移测试的 usize SQL参数改为i64；旧库fixture需事务满足延迟外键，补事务后通过。前端构建发现原图测试 HTMLElement 缺少 getContext 类型，明确为 HTMLCanvasElement 后通过。一次在根目录调用npm因无package.json失败，改从apps/desktop执行。上述失败均已恢复，不计为通过。

## 证据与限制

已查看原生模拟与真实截图：`artifacts/p0-a/context-snapshot.png`、`native-real-context.png`；观察JSON为 `artifacts/p0-a/real-context-observation.json`。这些是隔离测试工作区，不是用户项目知识。当前provider检查可能不提供digest，快照记录精确HTTP输入但不能承诺模型权重/回答精确重现。暂无自动检索、答案引用验证或执行图。Windows150%缩放、完整屏幕阅读器、公开安装包和新面板大知识列表性能尚未验证。


提交前发现新迁移工作区为CRLF，而Git索引为LF；发布前将0004统一为LF（尚未访问正式用户库）。通过标准库脚本核对工作区与暂存区字节相同，SHA-256为1849427505789c35f7358616dce145924ee0420b7c24b2874f543c3c810d057d。重新执行Rust测试10/10、桌面构建成功；最终二进制用 `$env:CORTEXOS_REAL_CONTEXT='1'` 后运行 `npx.cmd playwright test context.spec.ts context-real.spec.ts`，原生模拟与真实模型2/2通过（11.1秒）。最终真实观察wallMs1033（已热模型，仍包含UI检查开销），回答同上；artifacts/p0-a覆盖为最终截图和JSON，不把两次耗时当性能对比。测试之外未使用新迁移打开正式数据库。


# 2026-10-19 P0-B：真实执行图与跨页实体定位

按用户“继续”实施 P0-B，完成后停在 P0 门槛。复用现有事件、快照、消息、知识检查器和三维画布，无新增依赖。执行过程是应用可观测事实，不代表隐藏思维链。

## 变更

- 0005_execution_views 保存运行级相机/目标点及修订令牌；与知识布局独立。事件位置确定性投影，暂不增加位置编辑表。新增 list_runs/load_execution_graph/save_execution_view，共22个业务命令，无系统能力扩张。
- ExecutionTrace/execution-data 从真实事件、请求快照和输出生成图；列表与画布共享选择、运行分页、生成中刷新、终态详情、WebGL降级及离线读取。顺序、上下文纳入、输出保存三类关系有明确解释。
- navigation 统一工作区/会话/运行/消息/节点/版本定位。聊天打开执行轨迹；知识跳转固定当次版本；原消息滚动并取得焦点；返回原运行节点。视角草稿按工作区/运行在当前进程保留，重启仅恢复显式保存值。
- README、架构、schema、权限、交互规范、计划及测试文档同步；ADR0007记录事件投影和固定布局的实现细化。

## 实际验证

- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`：通过。
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --lib`：11/11，最终0.23秒。覆盖视角并发、无效坐标、跨工作区拒绝、分页、知识视角独立、完成/失败/取消及重启恢复中断读取、外键完整性；既有10项继续通过。
- apps/desktop 中 `npm.cmd run tauri -- build --debug --no-bundle`：最终构建成功。`npm.cmd run build` 在最后纯空白格式整理后再次通过，产物与原生内嵌版本一致：index-5WTXfqqx.js、GraphViewport-CFYPqqpe.js、index-Cic6bEcr.css。保留933kB图模块告警和 Three.Clock 弃用提示，未将其当作性能达标证据。
- `npx.cmd playwright test trace.spec.ts --project desktop`：原生专项1/1，15.4秒（用例11.6秒）。
- 完整检查先设置 `$env:CORTEXOS_REAL_CONTEXT='1'`，再运行 `npx.cmd playwright test`。首轮14通过/1失败：知识版本切换使历史面板折叠；修复后重新运行完整检查，最终结果见本节末尾。
- 使用隔离测试数据库、正常桌面权限和原生WebView2，默认1440×900/DPR1.25；浏览器另检验5种尺寸。真实模型为既有本地 Ollama 0.40.4 / qwen3:4b-instruct，未安装新软件或下载模型。

## 调试与修复

消息定位最初被页面主区域焦点覆盖；调整页面聚焦并在实体定位时保留滚动位置。懒加载画布未就绪时“适应”操作失效；增加就绪状态和按钮门控。旧URL曾在创建新工作区后触发回跳；收窄导航效果依赖并清除旧实体定位。首次专项的卡住测试由 Ctrl-C 终止后修复，未计为通过。完整回归发现切换历史版本触发详情卸载、折叠选择器；分离实体读取和版本选择后，既有连续版本切换检查通过。创建分支首次受沙箱.git写入限制，按既有提交授权提升后成功。

## 证据与边界

`artifacts/p0-b/` 保存实际执行图、真实模型图、375px WebGL降级截图及真实调用观察JSON。已查看截图；窄屏保留列表、详情和原消息操作。原生专项验证知识v1被后续v2替代后仍定位v1、消息焦点、返回节点、保存视角、实时终态、跨工作区隔离，以及关闭模拟模型服务后重启仍读相同事件和视角。失败/取消/恢复中断读取由Rust测试覆盖，未声称每种终态均有独立原生截图。

自动检索、来源文件授权导入、任意工作流依赖图、Decision Lab、评估、LoRA尚未实现。100/500/1000节点性能、模型并行GPU基准、Windows150%缩放、完整读屏、安装包仍待专项验证。当前模型按标签使用，输入快照不保证权重或逐字回答重现。当前停在P0验收，下一切片为P1-A。

最终完整回归 **15/15通过，1.2分钟**：8项Edge浏览器、6项原生桌面、1项真实Ollama（4.3秒）。最终观察回答为“验收项目代号为青鹭-742，发布时间是周四。”，使用知识v1，保存7个真实事件，重启快照一致。观察wallMs=1031包含UI检查开销，不作为模型性能基准。artifacts/p0-b已更新为最终测试产物。

提交前 `git diff --cached --check` 通过；0005工作区与索引逐字节相等、均为LF，SHA-256为730b644b5b5b8174b7b11712e148ef7e9c8ca61a69808d27484a8505668edb83。artifacts沿用仓库忽略规则，截图和观察JSON作为本机验收材料保留；测试源码与记录入库。Gittoken.txt未被跟踪。


# 2026-10-19 参考图驱动的AI对话UI重构

用户明确要求按参考图重构对话页及必要共享布局，完成后停止。实际文件是docs/design/reference/ai-chat.jpg（4797×2763），非提示中的PNG。用内置view_image读取并分析；本机未找到see技能或脚本，如实说明后使用内置工具。未读取另一张document-history.jpg内容，也未变更知识引擎/数据库/3D语义。

## 实现

拆分AppShell、TopBar、Sidebar、WorkspaceSwitcher、ConversationList、ChatMessage、ChatComposer、ModelStatus、EmptyState、CognitiveInspector；TopBar与AppShell同文件，其余按行为独立。保留studio作为状态与服务接口，原消息操作、IME防误发送、取消/错误/草稿、知识保存和执行快照沿用。浅色设计Tokens限定在reference-chat作用域，其余页面保留现状。参考品牌与未实现功能替换为CortexOS及真实入口；全部使用HTML/CSS/SVG和现有lucide，没有整图背景或新依赖。

新增ui-spec、component-map、visual-review，同步规范和计划覆盖关系。实际文件清单见component-map。用户在工作中自行修改的AGENTS.md不回滚、不纳入UI提交。

## 实际命令与验证

- apps/desktop中 `npm.cmd run typecheck` 首次发现提取后的modelReady未使用；将其用于模型设置入口的真实状态提示后通过。
- `npm.cmd run build` 通过；`npm.cmd run tauri -- build --debug --no-bundle` 首次因用户运行的exe被占用而失败。未强制结束进程，用户确认“关了”后构建成功。
- `npm.cmd exec --yes --package=prettier@3.9.10 -- prettier --write ...` 整理本轮组件、CSS、测试与配置，未修改项目依赖或锁文件。
- `npx.cmd playwright test --project edge` 初次6/8：两项窄屏测试仍假设导航常驻。测试改为先打开实际侧栏抽屉后导航，后续完整回归通过。
- `npx.cmd playwright test chat-design.spec.ts --project desktop` 原生专项1/1通过，10.3秒。检查1366×768、1920×1080空态/消息、流式停止入口、真实执行检查器、离线草稿、375px抽屉及Escape焦点。
- 先设置 `$env:CORTEXOS_REAL_CONTEXT='1'`，再运行 `npx.cmd playwright test`：完整 **16/16通过，1.6分钟**（8项Edge、7项原生桌面、1项真实Ollama）。真实模型检查17.8秒；运行与快照链路继续通过，观察JSON保留在artifacts/ui-chat。取消、异常、重启持久化、知识审核与图谱定位沿用既有测试。
- 在上述回归后补充窄屏抽屉只挂载一份Sidebar，避免重复工作区label/id；再次构建成功。最终产物index-DuLV9sH4.js、index-DsGy0J-I.css、GraphViewport-ye0uV-Ea.js。图模块933kB警告和既有Three.Clock弃用提示仍保留，未声称解决图谱性能。
- 最终增量检查命令 `npx.cmd playwright test app.spec.ts chat-design.spec.ts`，结果记录在下方。

## 截图与限制

实际截图位于artifacts/ui-chat，均为独立测试工作区。使用Windows System.Drawing将参考与实际截图等比例缩小并排，未安装Pillow（尝试导入后提示缺失）。人工查看参考、空态、对话、窄屏与对比图，调整欢迎区垂直位置和重复焦点框。未生成虚构相似度分数。

原生默认DPR1.25；通过Playwright设置WebView2 CSS视口，不等于更改Windows显示缩放。未完成150%/200%系统缩放、完整读屏或长会话性能专项。参考没有对话/加载状态，这些按产品状态补充；视觉差异与下一轮建议见visual-review。本轮完成后等待用户确认，不进入P1或重做其他模块。

最终增量检查 **8/8通过，18.9秒**（7项浏览器导航/尺寸，1项原生视觉与行为专项8.4秒）；工作区选择器唯一性断言通过。最终截图覆盖至artifacts/ui-chat。完整回归后仅追加了抽屉挂载修正，使用此针对性回归验证，未把前一轮结果冒称为最终二进制全量重跑。


# 2026-10-19 P1-A 生效知识自动检索

用户确认三项推荐：进入P1-A、自动检索默认开启且按会话关闭、优先中文夹杂英文术语和代码。本轮保留对话设计，交付审核知识→索引→实际检索→请求快照→执行图的完整闭环，停在P1-A验收点。

## 实现与文件

新增 migrations/0006_knowledge_retrieval.sql、src-tauri/src/retrieval.rs、components/RetrievalSummary.tsx、tests/retrieval.spec.ts、ADR0008。后端 db/knowledge/models/commands 与命令能力声明接入索引、偏好、预算和真实事件；前端 studio、Chat、ContextPicker、ChatComposer、CognitiveInspector、ExecutionTrace、execution-data 和局部CSS接入实际状态与候选说明。既有上下文、聊天、Ollama和执行图测试更新默认检索语义；README、架构、schema、规范、计划、权限和测试文档同步。

FTS5使用中文相邻双字与英文数字词项、BM25排序；仅检索当前工作区已生效的knowledge/context/decision版本。最多记录20候选、自动纳入5项，使用手动资料后剩余预算的一半，历史使用余量。候选记录版本、分数和纳入/排除原因。审核更新与索引原子提交；草案不覆盖生效内容。schemaVersion3保存当次检索和完整请求，实际检索开始/完成事件进入既有执行图。新旧会话默认开启，独立偏好重启保留；关闭不清除已有会话历史。

## 实际验证

- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` 通过。
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --lib` 最终14/14，0.30秒。包括8条固定混合语言查询、v5迁移回填、候选/预算上限、版本审核、跨工作区隔离、偏好恢复、事务失败回滚、不可变记录和外键检查。初轮旧断言仍预期默认不检索，修正后通过。
- apps/desktop中 `npm.cmd run typecheck`、`npm.cmd run build`、`npm.cmd run tauri -- build --debug --no-bundle` 成功。最终内嵌资源index-BmqIDbuv.js、index-Czr1-CGQ.css、GraphViewport-CzpYtZik.js。未新增依赖、系统安装或模型下载。
- 设置 `$env:CORTEXOS_REAL_CONTEXT='1'` 后 `npx.cmd playwright test`：初轮16通过/1失败，发现异步保存时复选框短暂恢复旧值；增加按会话隔离的即时反馈，失败恢复并保留草稿。重建后二次完整回归 **17/17通过，1.5分钟**：8项Edge、8项原生桌面、1项真实Ollama（4.8秒）。真实模型自动获得已审核知识，实际请求与快照相同。
- `npx.cmd playwright test retrieval.spec.ts --project desktop`：修复后专项1/1；全量通过后补充等待WebGL就绪、适应图和展开原因再截图，再次专项 **1/1，7.0秒**（用例4.4秒）。最后只修改测试截图等待，未改变运行代码或二进制。
- 隔离测试数据，原生WebView2默认1440×900/DPR1.25，对话专项覆盖1366×768及1920×1080。测试验证首次开关操作保留草稿、预览不调用模型、v1到v2审核影响下一请求、无命中、关闭不生成检索事件、重启偏好与旧快照不变、新会话重新默认开启。

## 证据、限制与停止点

artifacts/p1-a保留实际检索输入条、执行图截图、真实Ollama截图及观察JSON，沿用忽略规则作为本机证据。已查看检索输入条与就绪执行图；最初截图早于画布就绪，已用专项截图替换。执行图仍有相邻标签重叠的既有视觉限制，后续可单独优化；不影响事件列表和详情查阅。

本轮为词项检索，无语义向量、同义词或单字中文召回；8条固定样例不代表通用检索质量。BM25共享索引统计但候选按工作区严格过滤。上下文准备沿用原子提交边界，准备失败不调用模型且保留草稿，但不持久化独立失败运行。预览只读，发送重新读取有效版本。图模块约933kB及Three.Clock弃用提示仍存在；大图性能、150%系统缩放、完整读屏、安装包尚未验证。P1-B文件授权导入、Decision Lab、评估及LoRA未实现。本轮完成后停止，下一阶段先澄清用户决策。


# 2026-10-20 知识页面参考布局

用户提供document-history.jpg，确认仅重排知识页；随后明确以聊天UI为底层、不要大改。按后续指示复用聊天侧栏、Tokens与顶部状态，保留文字导航，不采用最初讨论的独立窄导航。参考仅用于知识列表在上、版本时间线在下、正文在右的排布。

## 实现

修改App.tsx、Knowledge.tsx、components/AppShell.tsx、components/Sidebar.tsx；新增knowledge-design.css。版本历史使用原有详情数据和React portal，保持单份读取/审核逻辑，点击时间线可切换版本，原下拉选择仍保留。搜索/状态筛选、新建/编辑、未保存草稿保护、并发冲突、审核、生效版本对照、来源摘录/哈希、原会话、图谱/执行定位继续使用原接口。图谱检查器保留原排布；无数据库、Rust、权限或依赖修改。共享导航在知识页正确高亮并保留工作区切换目标；窄屏沿用聊天抽屉，历史位于正文之后。

设计文档design/knowledge-layout.md记录用户纠正后的范围、估算尺寸和与参考的取舍；implementation-plan同步此次独立UI切片。参考没有转换为页面背景，也没有添加虚构的导入、恢复或行级diff。

## 实际检查

- `npm.cmd run typecheck`误从仓库根执行一次，因无package.json失败；改在apps/desktop执行后通过。
- apps/desktop中 `npm.cmd run tauri -- build --debug --no-bundle`成功，包含类型检查和Vite构建。
- `npx.cmd playwright test`：完整 **16/16通过，1.4分钟**，8项Edge＋8项原生桌面。涵盖聊天取消/异常/重启、知识来源/审核/并发/保存、检索偏好和快照、知识/执行图跨页定位。此轮未设置真实Ollama检查开关，模型流程使用本地HTTP测试服务，未声称重新完成真实模型测试。
- `npm.cmd exec --offline --package=prettier@3.9.10 -- prettier --write ...`因缓存缺失失败；同版本普通exec成功整理4个组件/CSS文件，未更改依赖。格式整理后 `npm.cmd run build`通过，产物与原生构建相同：index-DIKCKM0b.js、index-Bt6XidXv.css、GraphViewport-7l3gGUX7.js。
- 在原知识集成测试添加侧栏时间线点击、1366×768/1920×1080截图与当前导航断言；窄屏沿用375×812且补充历史版本切换。专项最终结果记录于下方。

## 截图与边界

artifacts/ui-knowledge保存原生截图（本机忽略文件），已检查1366px和375px截图。最初两尺寸截图保留了测试点击旧版本后的侧栏滚动位置，补充截图前回到侧栏顶部以显示完整标题。测试使用隔离数据库和WebView2，DPR1.25；未改用户真实数据。参考的紫色、窄图标栏、Mac标题栏不复制，服从用户明确要求的聊天UI底层。正文继续纯文本；现有生效版本对照保留，未增加富文本和行级差异算法。图模块约933kB和Three.Clock弃用告警仍存在，未进行图性能、系统150%缩放、完整读屏或安装包验证。本轮完成后停在布局验收点，不进入P1-B。

最终专项命令 npx.cmd playwright test knowledge.spec.ts --project desktop：1/1通过，7.4秒（用例4.9秒）。包含375px版本切换，最终截图已覆盖至artifacts/ui-knowledge。


# 2026-10-20 紧凑导航、知识阅读与历史抽屉

用户明确要求解决双侧栏臃肿、历史位置不合理与正文空旷；随后要求主界面不要小灰字说明。先检查共享外壳、知识组件、样式和原有功能，给出文件方案，按导航→列表→正文→历史推进。用户更新AGENTS要求非必要不跑单元测试；本轮没有Rust/单元测试，已运行的是此次任务明确要求的类型检查、构建及真实界面集成检查。

## 实际修改

- components/AppShell.tsx统一56px可展开导航，工作区/模型状态在顶部；ConversationList保留聊天会话和新建入口。新增components/NavigationIcon.tsx原创几何SVG与compact-shell.css，支持Tooltip和键盘聚焦。
- Knowledge.tsx移除历史portal，列表260px可收起；正文保留编辑、审核、对照、来源、图谱定位。历史用原生非模态dialog右侧抽屉呈现，Escape/关闭恢复触发焦点，切换实体关闭旧抽屉。
- knowledge-design.css限定正文860px，桌面打开抽屉时预留380px；较窄窗口覆盖显示。类型/版本/状态合并一行14px清晰文字，取消正文小灰字说明；来源解释按需展开，状态色降噪。App.tsx引入样式。
- 更新app.spec.ts、chat-design.spec.ts、knowledge.spec.ts；新增knowledge-layout.spec.ts及Playwright配置入口。设计规范与实施计划同步。未改数据库、Rust、模型调用、知识引擎、权限或依赖。

## 分阶段实际检查

- 第一阶段 `npm.cmd run typecheck`通过；`npx.cmd playwright test app.spec.ts --project edge` **7/7，9.6秒**。实际浏览器导航、键盘与375–1440px布局验证。
- 第二阶段同两命令通过，浏览器 **7/7，8.9秒**。列表收起布局接入，历史移出列表。
- 第三阶段 `npm.cmd run tauri -- build --debug --no-bundle`成功；`npx.cmd playwright test knowledge-layout.spec.ts --project desktop`首轮失败：程序聚焦导航时Tooltip未显示。将提示适用状态补充到focus，随后完整原生检查通过。
- `npm.cmd exec --yes --package=prettier@3.9.10 -- prettier --write ...`整理本轮新增/修改组件、样式和布局测试，无依赖变更。
- 第四阶段重建成功；`npx.cmd playwright test` **17/17，1.4分钟**：8项Edge、9项原生WebView2。包含三个目标尺寸、56px导航展开、260px列表收起、正文宽度、历史打开/Escape焦点，及原聊天、审核、版本对照、来源、草稿保护、并发冲突、重启、检索、图谱定位回归。模型相关测试使用本地HTTP测试服务；未重跑真实Ollama专项。
- 人工查看截图发现抽屉遮正文，修正1200px以上预留抽屉空间并移除重复历史标题；追加用户“去小灰字”要求后再次构建成功。最终原生资源index-CtdGXdjX.js、index-BIHhB7U0.css、GraphViewport-By-6SJeV.js。后续只运行必要布局专项，不重复全量检查。

## 证据与限制

截图保存在忽略目录artifacts/ui-knowledge-compact，是真实隔离测试数据库中显示的知识内容，非静态页面背景。视口1366×768、1600×900、1920×1080，原生DPR1.25。此前375px浏览器及知识原生流程通过。图标为手写SVG，不使用emoji。导航展开仅保留当前进程状态，未新增偏好数据库；正文仍为纯文本，版本对照沿用生效版内容，不新增行级diff。未验证系统150%缩放、完整读屏、安装包或图性能；既有933kB图模块和Three.Clock告警仍保留。完成本UI切片后停止。

最终布局专项 npx.cmd playwright test knowledge-layout.spec.ts --project desktop：1/1通过，5.3秒（用例2.9秒）。三个目标尺寸均验证正文右边界不超出抽屉左边界，最终截图已覆盖。全量17项在最后排版修正前通过，未将其冒称为最终二进制全量重跑。


# 2026-10-20 P1-B1 授权文件来源与知识证据

用户要求按计划继续，确认三项推荐：明确选择的UTF-8 TXT/Markdown、单文件2MiB；本地版本快照与手动更新；停用未来使用并保留历史。P1-B拆为B1来源证据闭环与B2回答引用解析，本轮交付B1后停止。权限设计先记录在ADR0009，沿用紧凑UI且主界面不增加小灰字说明。

## 实现与修改文件

新增src-tauri/migrations/0007_source_evidence.sql、src-tauri/src/sources.rs、前端SourceBrowser.tsx/source-browser.css、tests/sources.spec.ts、ADR0009。修改commands/lib/build/capabilities注册7个受限来源命令，Cargo.toml/Cargo.lock加入官方tauri-plugin-dialog（2.8.3，传递依赖rfd/tauri-plugin-fs；前端没有fs或dialog插件权限）。未安装系统软件或新增npm依赖。

knowledge.rs复用原事务保存知识版本和固定来源/行链接，后续修订继承起源；db/retrieval对显式和自动纳入检查来源启用，快照知识引用携带真实sourceEvidence。SourceBrowser提供预览/导入、快照版本、固定行范围、200行分页、建立草案和停用；Knowledge.tsx复用编辑/审核并提供原文入口，ContextPicker隐藏已停用来源项，App.tsx引入样式。native测试辅助只在隔离目录使用debug选择替身，另提供本测试进程PID用于真正原生选择器验收。README、architecture、schema、security、test-plan、implementation-plan、refactoring-plan同步。

票据一次性/10分钟过期，取消即作废；提交不静默重读文件，原文改变也保存预览内容。重新选择同路径且字节变化新增来源版本；未变化不重复建版本；停用来源需重新明确选择导入才能恢复。停用与相关FTS变更同事务，旧会话内容与旧运行快照保留。来源内容仅作为数据，审核不等于事实验证。

## 实际检查与修复

- `cargo add tauri-plugin-dialog@2 --manifest-path apps/desktop/src-tauri/Cargo.toml`完成，Cargo.lock新增14个传递包；既有版本未整体升级。
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml`及`cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked`通过。中间一次检查发现debug路径变量替换遗漏，修正后通过；移除未使用import。
- `npm.cmd --prefix apps/desktop run typecheck`通过；后续apps/desktop中`npm.cmd run tauri -- build --debug --no-bundle`通过。首轮构建因测试replaceAll超出项目目标库失败，改为兼容replace后成功。最终资源index-QnfIlpq5.js、index-Ca-1QCrO.css、GraphViewport-Om0OCpjN.js。
- `npm.cmd exec --yes --package=prettier@3.9.10 -- prettier --write ...`整理本轮组件/样式/测试，无依赖改动。
- `npx.cmd playwright test sources.spec.ts --project desktop`初轮2项失败，均为Playwright将IPC拒绝包装为Error，字符串包含断言不适用；改为toThrow后 **2/2通过，9.3秒**。覆盖预览不落库、预览后原文件改变仍保存预览、来源v2与知识源v1独立、知识修订继承、审核/实际HTTP请求与快照等值、跨工作区/票据重放/取消/通用fs拒绝、停用自动/手动拒绝、重启/重新授权，以及v6迁移、非UTF-8/超限/空文件/敏感名称/重解析路径拒绝。
- 必要相关回归 `npx.cmd playwright test sources.spec.ts context.spec.ts knowledge.spec.ts retrieval.spec.ts --project desktop`：前5项通过，30.2秒整轮中真正原生选择器验收脚本失败。应用选择窗口已打开；本机UIAutomation把文件名Edit暴露为Pane且不提供ValuePattern。脚本几次适配/检查控件元数据后，改为按测试进程PID定位窗口/原生Edit句柄，以Win32设置文件名并向该对话框发送IDOK；不操作其他进程，不读取桌面文件列表内容，不更改剪贴板。
- 最终 `npx.cmd playwright test sources.spec.ts --project desktop -g "Windows native picker"` **1/1通过，4.7秒**（用例2.5秒）。真正通过Windows文件选择器选择项目隔离目录文件→原文预览→确认导入；1366×768/1920×1080无横向溢出并截图。最终只修改测试适配，运行代码/二进制与上述5项通过版本一致，因此本切片6项相关验收均通过；没有声称全项目19项全量重跑。

遵循用户新规范，未运行Rust或其他单元测试。v6夹具升级后PRAGMA integrity_check=ok、foreign_key_check为空、迁移版本7；旧知识内容保持。未新增系统权限或访问项目根目录真实令牌，敏感名称验证仅用隔离合成夹具。

## 证据、限制与停止点

artifacts/p1-b1保留真正原生选择后显示的来源窗口截图，已查看1366px；最终来源专项重新生成并保留了来源版本/证据截图。数据库/索引检查和运行快照核对见可重跑测试源。Windows原生WebView2，默认DPR1.25；未验证系统150%缩放、完整读屏、大文件长期性能或安装包。未重跑真实Ollama，模型输入由本地HTTP测试服务捕获；原生文件选择使用真实系统对话框。

仅支持本机普通UTF-8文本/Markdown，不支持PDF/Office、目录扫描/监控或网络路径。原文快照不自动全文检索，只有从片段建立并审核的知识进入P1-A；正文仍纯文本。撤销不抹去历史内容，永久删除尚无界面。源路径仅本地留存；固定行范围基于BOM/CRLF规范化快照。历史回放不重新读取原文件。P1-B2回答引用解析、片段引用校验和点击引用定位未实现，本轮停止在B1验收。


最终代码审查将commit_source改为数据库任务前原子取出票据，失败时在未被新选择覆盖的情况下恢复重试；同时提交只能成功一次。一次辅助修改脚本因工作目录下相对路径错误失败，改用绝对路径后修正并重新构建。最终 `npx.cmd playwright test sources.spec.ts --project desktop` **3/3通过，12.6秒**，含并发提交断言及真实选择器；截图已覆盖至artifacts/p1-b1。此前context/knowledge/retrieval三项原生回归通过，最终后端改动仅票据提交逻辑，使用来源专项复查，无单元/全量重跑。


# 2026-10-20 全界面通俗用词与准确性检查

用户要求不用“原生来源”等晦涩词，并自查其他界面；严谨、正确、完整优先。本轮检查聊天、知识、资料文件、图谱、执行记录、设置及未来错误提示，做展示文案调整。保持内部API/字段/事件ID、原始数据、历史记录、参数单位与数值边界。

主要替换：来源入口→资料文件；来源快照→保存的文字副本；草案→草稿；审核并确认→检查并确认；上下文预览→查看本次会用的资料；候选/纳入/预算→找到的知识/选用/容量上限；运行审计→处理记录/详细记录；内容哈希→内容校验码（SHA-256）；适应/聚焦→显示全部节点/移到选中节点。设置参数加通俗名称并保留Token/温度；说明Token不等于字数、温度不代表正确性。预览只说选用，不冒称模型已使用；用户确认不等于事实核实；停用后历史内容仍可能保留。源文件保存为规范化文字副本，不声称复制文件属性。详细规则新增design/plain-language.md并加入development-standards，README使用说明同步。

正确性修复：设置页“尚未授权任何来源”和“文件导入开放前不会读取项目资料”已过时，改为当前的主动选文件政策。模型仅记名称的限制改为不能确认以后模型文件是否相同，且同一模型也可能回答不同。未知量化仍标明压缩方式未知，不伪造模型信息。图谱关系及执行顺序不改变含义；节点位置不能证明因果或可信度的说明保留。必要安全限制、UTF-8、2MiB、GGUF、Token、SHA-256保留并改用可操作的说明。

修改前端各页面/组件及execution-data/graph-data的显示文本，Rust仅改错误文案与系统文件选择窗口标题；不改数据库、权限、模型请求、状态值或数值校验。知识标题栏允许换行，防止“资料文件”较长按钮挤压窄窗口。已有界面测试选择器同步到新名称，本轮没有增加或运行单元/集成/界面测试。

实际验证：`npm.cmd --prefix apps/desktop run typecheck`通过；固定版本Prettier整理修改组件；`cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml`完成。`npm.cmd run tauri -- build --debug --no-bundle`包含类型检查与前端构建，最终结果记录于下方。按代码逐项核对旧字段/事件ID/参数范围与新显示名称，不将构建通过当作界面实测。既有图模块约933kB告警保留；本轮未生成新截图，先前截图的用词不代表最新版本。

桌面构建成功：index-CU-C3x5Z.js、index-B2jBWQkw.css、GraphViewport-BdwdcTdd.js。`git diff --check`通过；未运行测试、未新增截图。

# 2026-10-20 P1-B2 回答引用与固定文字定位

用户要求继续推进；按已定 P1-B1→B2 顺序完成引用切片，停止在 B2 门槛。先检查总纲、开发规范、实施/纠偏计划、数据库终态保存/恢复、上下文组装、消息读取、聊天及来源窗口。采用 ADR0010：请求内资料编号 `[资料1]` 或 `[资料1:2-3]`，后者是本次知识正文的1基闭区间，不是原文件行号。知识编号及格式说明真实进入发送请求，其完整字节开销使用原容量计算；不改变模型服务接口或权限。

## 实际修改

- 新增 `src-tauri/src/citations.rs`：从固定 manifest 解析实际回答标记，检查编号及行范围，保留原文、无效原因及 UTF-16 位置；扫描残缺标记有128字符边界，避免逐标记扫描全部剩余输出。
- 修改 Rust `db.rs`、`models.rs`、`lib.rs`：manifest 升至4，保存 citationProtocol/citationId；统一 finish 事务追加实际 citations_checked 与回答/终态，取消、失败和启动恢复复用；load_messages 返回已保存结果。已有 SQL 迁移、数据库版本7及锁文件均无改动，无新增命令/能力。
- 新增 `components/AnswerContent.tsx`，复用 `ChatMessage`、SourceBrowser（onDraft 可选，知识页原回调保留）、知识版本导航及聊天样式。编号按钮打开固定全文及高亮行，错误/缺失引用持续提示，来源起源和说明按需展开。复制/保存知识仍用原始消息文字。导航/App 支持从知识版本返回原回答并恢复焦点；studio 增加可选引用类型。
- `CognitiveInspector.tsx`、`execution-data.ts` 展示真实“检查引用位置”事件，明确没有核实回答内容。不存在的编号不回退到未选知识、其他运行或最新版本。来源起源与知识正文分别展示，避免知识修订被误解为文件也已修改。
- 新增 `tests/citations.spec.ts` 一条必要原生整链路场景并加入 Playwright 配置；更新 README、ADR0010、architecture、database-schema、implementation-plan、refactoring-plan、test-plan 和本日志。保留用户 AGENTS.md、desktop.spec.ts 行尾状态以及 reference 文件，不纳入本次提交。

## 实际命令及结果

- `npm.cmd run typecheck`（apps/desktop）：通过；两次桌面构建内也执行类型检查。
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml`完成。随后的 `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked` 初次发现 Message 的 Value 类型缺少限定名，改为 serde_json::Value 后通过。恢复与本任务无关的 ollama.rs 格式变化。
- 第一次 apply_patch 因 manifest 的整行上下文不匹配失败，未写入部分文件；修正匹配后成功。窄范围读取曾有错误路径/PowerShell通配符，随后按实际文件读取，不把读取失败当实现缺陷。
- `npm.cmd exec --offline --package=prettier@3.9.10 -- prettier --write ...` 因离线缓存响应缺失失败；正常 `npm.cmd exec --yes --package=prettier@3.9.10 -- prettier --write ...` 完成（AnswerContent、ChatMessage、CognitiveInspector、SourceBrowser、navigation、App、studio、execution-data、chat-design.css、citations.spec.ts、playwright.config.ts；部分未变）。未新增依赖。
- `npm.cmd run tauri -- build --debug --no-bundle`两次成功，最终资源 index-BIsfhK4W.js、index-BTe-S12r.css、GraphViewport-7-EKIW-4.js；最终二进制位于 src-tauri/target/debug/cortexos-desktop.exe。
- `npx.cmd playwright test citations.spec.ts --project desktop --output ../../artifacts/p1-b2` 首轮 **1/1通过，6.7秒**；补上实际引用检查事件在3D图中的入口和说明，最终二进制复查 **1/1通过，5.9秒**（用例3.8秒）。未跑单元测试或全量回归。
- `git diff --check`通过；`git rev-parse --show-toplevel`确认 D:/vibecoding/CortexOS。提交/推送见本轮最终记录。

该场景捕获真实 HTTP 请求并与保存请求逐项比较；验证全文及指定行、重复同文行、中文/emoji位置、未选编号/越界/错误起点/残缺标记、无引用、生成时未检查、停止后部分引用、知识版本跳转与返回焦点、实际图节点、知识v2/文件v2/停用后的历史v1、跨工作区拒绝、正常关闭重启。1366×768、1600×900、1920×1080无页面横向溢出，抽屉范围及Escape恢复焦点通过。artifacts/p1-b2有三个真实原生截图（DPR1.25），已查看1366和1920；非静态背景。

## 限制及停止点

本轮模型由可控本地 HTTP 服务返回合成回答，未重跑真实 Ollama；不能据此保证模型总会标注引用。位置检查不验证论断真伪、引文是否支持论断或模型是否遵守文件里的指令；权限边界保持由Rust控制。无本协议的旧运行直接保留原样，失败/启动恢复共用 finish 路径做代码检查，未新增故障重启或旧迁移实测。系统150%缩放、完整读屏、安装包及性能基准未验收；既有约933kB图模块告警仍在。

当前 P1-B 约定的文件→确认知识→真实请求→引用→历史资料闭环已交付。文件仍不直接全文检索，不支持PDF/Office、目录监控或永久删除；P1-C图交互与性能、P2决策/比较/评估保持待实施。完成本切片后停止。

仓库操作：已确认独立根目录后 `git switch -c codex/answer-citations`，显式 `git add -- ...` 本轮23个文件，`git diff --cached --check`通过。使用 `git commit -m "feat: resolve answer citations against saved run inputs"` 提交，正常 `git push -u origin codex/answer-citations` 复用仓库凭据；最终结果以本轮工具输出及汇报为准。补看最终1600px截图，三个目标尺寸均已查看。

## 2026-10-20 参考图 1–8 应用与控件比例修正

用户要求把新版参考图应用到真实界面，同时补回必要边框、减少小灰字/间隔点说明、检查全部主要按钮与文字比例。范围只含既有 UI，停止在本轮适配门槛；不推进 P1-C/P2。

### 交付

- 新增 src/workspace-design.css 统一现有语义 Tokens、留白、字号、控件边框和连续圆角，App.tsx 最后引入。保留原有专用布局作为基础，不新增库。导航64px、聊天列表224px、知识列表240px；主正文限宽，窄屏单列。
- 按钮按文字定宽，常规高度36px、内边距7px 14px。确认/保存/资料/图谱控件复用同一规则，禁用主按钮回到中性色；输入框、搜索、顶栏、侧栏、画布分区和弹窗保留边界，列表与正文不增加卡片。
- Settings 按模型连接/本地数据/关于分类显示，参数保留原 details，DOM 与 settingsDraft 保留。Graph 的关系列表按需展开，节点列表默认收起，所有尺寸保留入口与 WebGL 降级。
- 知识状态拆为独立文字片段。ContextPicker、ExecutionTrace、Graph、Knowledge、SourceBrowser、AnswerContent、CognitiveInspector、RetrievalSummary 简化界面元信息分隔符；常驻界面仅保留窗口标题中的间隔点。预算与执行解释收起，错误、未保存、停用、版本变化及实际事件保留。
- 不改变 Rust、数据库、权限、依赖、锁文件或引用/审核/草稿业务逻辑。保留用户原有 AGENTS.md、desktop.spec.ts 改动与 reference 图片。

### 实际命令与结果

- git rev-parse --show-toplevel：确认 D:/vibecoding/CortexOS；未暂存、提交或推送。
- UI/UX 数据检索首次 python 指向旧解释器产生 SyntaxError；改用 py -3 执行 skills/ui-ux-pro-max/scripts/search.py，先查 productivity whitespace progressive disclosure（结果不匹配），再查 whitespace spacing hierarchy（使用文本限宽/间距指导）。未持久化模板。
- npm.cmd run build（apps/desktop）两次通过，包含 tsc --noEmit；npm.cmd run tauri -- build --debug --no-bundle 两次通过。最终 CSS index-BS6JJ2mu.css、JS index-C4Gh3LJj.js；最后仅排版格式修改后的构建资源哈希与原生检查版本一致。已有约933kB图模块体积告警保留。
- npm.cmd exec --offline --package=prettier@3.9.10 -- prettier --write 指定变更文件：ENOTCACHED，未联网安装或新增依赖，改为手动整理 TSX。
- node artifacts/ui-reference-implementation/review.cjs：默认沙箱原生进程未打开可用 CDP 端口，终止该次检查后用 require_escalated 运行同一隔离脚本；首次因按钮定位重复停止，收窄到工作区 dialog 后得到22张截图。补充加载完成的3D/确认/导入检查时，测试来源文件不在专用数据目录，原有权限校验正确拒绝；将夹具移入对应 data-时间戳 后最终完成。
- 最终视觉脚本输出 errors=[]、captures=23、overflow=[]、smoothCorners=true。使用独立 data-时间戳 与 webview-时间戳 目录，本地 HTTP 返回标注在“视觉验收（隔离数据）”工作区中的合成回答；没有访问日常用户工作区或真实 Ollama。
- git diff --check 通过，只有现有 CRLF/LF 提示。未运行单元测试、Playwright 测试套件或 Rust 测试。

### 原生 WebView2 实际检查

- 对话与知识：1366×768、1600×900、1920×1080、800×600、375×812；设置：1366×768、1600×900、800×600、375×812。页面和主内容区均无横向溢出，必要操作可通过滚动访问。375px 是 CDP 设置的内容视口，不代表原生外窗低于800px最小宽度。
- 实际操作：创建隔离工作区、发送/完成合成回答、打开处理记录、进入执行轨迹并等待 WebGL、选择真实事件、查看知识/历史、关闭历史、打开/取消草稿编辑、确认知识版本、预览并确认导入 UTF-8 文件、查看带行号副本、选择图谱节点、展开关系并打开/取消关系编辑、切换设置分类、展开参数。
- 1600px实测：检查并确认124×36、保存草稿86×36、确认此版本100×36、保存关系86×36、保存设置86×36。36px不牺牲项目要求的32px最低点击区域。
- 已查看对话、知识、设置、窄屏、知识编辑/确认、处理记录、执行图、知识图、文件内容和关系编辑截图；最终页面与指标保存在 artifacts/ui-reference-implementation，含 review.cjs 和 metrics.json。

### 限制与停止点

此次属于视觉适配，不替代既有持久化、取消竞争、引用、冲突、权限等完整回归；这些路径仅核对原逻辑未被改写。未重跑真实 Ollama、系统150%缩放、完整读屏、故障恢复或安装包测试。图谱仍使用现有真实几何和图数据，不复制参考图中的虚构节点/样式标签。完成本轮UI验收并停止。

## 2026-10-20 留白密度回调

用户反馈上一轮留白过大。本轮直接调整 workspace-design.css 原有规则：页面横向边距上限72→40px，对话最大宽760→960px，知识外层840→1040px；消息间距44→28px，正文行高1.95→1.75；收紧列表、表单、弹窗与设置分组间距，保留36px按钮、边框及响应式规则。同步更新设计文档。未修改组件功能逻辑。

实际执行 npm.cmd run build（apps/desktop）：类型检查和 Vite 构建通过，原有933kB图模块告警保留。核对共享样式和窄屏覆盖规则；本轮未重跑截图、原生桌面构建或单元测试，上轮截图不代表此次密度调整后的画面。

# 2026-10-20 P1-C1 图谱直接关联、键盘操作与绘制基线

用户说明刚微调UI，要求按当前规范继续。核对总纲、开发规范、实施/纠偏计划、reference-ui-implementation.md及现有图谱/数据库/组件。先将用户这批UI代码和对应说明作为独立基线提交860045b（15文件），不覆盖其规范：64px导航、紫色主操作、默认收起节点列表、按需展开说明、36px内容定宽控件。AGENTS.md、desktop.spec.ts行尾状态和新增参考图保留，不纳入提交。继续在codex/graph-neighborhood分支实现ADR0011约定的P1-C1；本轮不宣称整个P1-C完成。

## 实际修改文件及行为

- Graph.tsx：当前有效一跳关联范围（含中心最多50个，每次增加50个）、返回全部节点、节点与关系列表50项显示分页；保留搜索/类型/状态/日期，显式临时显示隐藏节点；关系两端按钮及用户记录/未登记独立证明的准确说明，保留修订历史和全部编辑能力。sessionBrowse按工作区/演示分别保留搜索/筛选/中心/展开量，不改保存视图语义。节点选择后焦点进入详情，清除/分页/收起后焦点有明确去向。
- graph-data.ts：relatedNodeIds只用最新有效关系，双向收集一跳且去重，排除停用和间接两跳节点。GraphViewport.tsx：从canvas继承Tokens，使最新UI背景覆盖生效；记录真实上一帧绘制提交计数，不增加模型事件、特效或持续绘制。
- workspace-design.css仅增加6条局部规则：关联范围操作、关系端点、隐藏节点提示、详情键盘焦点，以及窄屏降级列表/详情层级。保留用户原文件格式与既有布局。
- 新增tests/graph-neighborhood.spec.ts、graph-fixture.ts、graph-baseline.spec.ts；修改Playwright配置，基线仅在CORTEXOS_GRAPH_BASELINE=1时启用独立项目。graph.spec.ts同步新版标点和列表/关系折叠入口，保留原关系冲突、知识编辑确认、保存视角、停用、重启与相机精度断言。
- 新增ADR0011、graph-baseline.md；更新README、architecture、development-standards、graph-experience-v2、implementation-plan、refactoring-plan、test-plan及本日志。Rust、迁移、权限、模型调用、依赖和锁文件均无改动。

## 实际命令与验证

- 多次 `npm.cmd run typecheck`（apps/desktop）通过；每次桌面构建也包括类型检查与前端构建。
- 使用已应用的ponytail与ui-ux-pro-max约束复用现有实现。默认 `python D:/CodexData/skills/ui-ux-pro-max/scripts/search.py "focus restoration hidden navigation" --domain ux` 无法解析技能脚本，改用 `py -3 ...` 成功。键盘导航结果适用焦点可见和操作顺序，不将一般布局结果当作专项验证证据。
- `npm.cmd exec --yes --package=prettier@3.9.10 -- prettier --write ...`整理本轮Graph、GraphViewport、graph-data、测试及配置；格式化曾展开用户CSS的整文件格式，随后恢复原格式，只保留新增规则。两次apply_patch因格式上下文不匹配失败且未写部分文件，按实际行修正后成功。
- `npm.cmd run tauri -- build --debug --no-bundle`共4次成功，用于初版、降级层级、清除焦点、统一分页焦点。最终资源index-C7OFpWxZ.js、index-DYDSe7mD.css、GraphViewport-BhRr93gR.js；二进制src-tauri/target/debug/cortexos-desktop.exe。既有约933kB图模块告警保留。
- `npx.cmd playwright test graph-neighborhood.spec.ts --project desktop --output ../../artifacts/p1-c1` 首轮 **1/1，7.0秒**。随后增加375px降级及原graph.spec.ts必要回归，命令 `npx.cmd playwright test graph-neighborhood.spec.ts graph.spec.ts --project desktop --output ../../artifacts/p1-c1` 首次新场景通过、旧场景失败（21.2秒）：清除按钮移除后焦点落到根页面，Escape未到达列表，下一次列表按钮操作反向关闭。定位后修正真实焦点行为；第二轮 **2/2，13.6秒**。统一分页焦点后最终二进制同两场景 **2/2，13.3秒**（4.9/6.1秒）。没有删原有业务断言来换取通过。
- 临时进程环境 `CORTEXOS_GRAPH_BASELINE=1`，执行 `npx.cmd playwright test graph-baseline.spec.ts --project graph-baseline --output ../../artifacts/p1-c1-baseline`，finally移除该进程环境变量：**1/1，54.8秒**（场景52.5秒）。真实原生绘制、隔离合成数据库，无模型生成。100/1000、500/1000、1000/5000节点/连线：读取往返中位数133.9/181.2/707.7ms，绘制提交1101/1501/6001，350ms空闲均0新增绘制。详见graph-baseline.md，不能把脚本绘制率当屏幕FPS或把JS堆当显存。
- `git diff --check`通过；`git rev-parse --show-toplevel`确认D:/vibecoding/CortexOS。没有运行单元或全量测试，没有安装软件/新增依赖。

最终原生交互覆盖：58节点和有效/停用/两跳关系；50项范围/列表/关系分页及焦点；隐藏/临时显示、关系两端、跨页范围保留、工作区隔离、WebGL降级、浏览前后完整GraphData相等；相关旧场景覆盖知识v2确认、关系4版/停用、冲突保留与读取比较、独立视角保存/相机精度及关闭重启。1366×768、1600×900、1920×1080无页面横向溢出，375px内容视口降级仍能操作。已查看三个尺寸及375px真实截图，位于artifacts/p1-c1；375px是CDP内容视口，不代表原生外窗小于800px最低宽度。

## 停止点与后续

完成P1-C1后停止。全图仍由原API一次读取，显示分页不等于数据库分页；多中心范围合并/收起、数据库/历史读取规模和批量绘制在P1-C2继续。基线在最终焦点/降级小修前采集，最终绘制实现与基线相同，未重复基线测量。此轮未测真实模型共存、完整CPU/GPU占用/进程内存/显存、系统150%缩放、完整读屏、长期性能或安装包。不会据一次原生合成首画面小于2秒就宣称达标。后续按基线优先测量节点/箭头独立mesh的绘制开销，再优化并同场景对照。

仓库按显式文件范围提交本轮变更，并使用配置凭据正常推送codex/graph-neighborhood；最终提交与推送结果见本轮工具输出和汇报。未读取Gittoken.txt，用户参考图和其他改动保持原样。

## 2026-10-21 V3 图谱视觉与相机切片

### 范围和永久规范

按用户先计划后实施的要求，读取本地 ai-chat.jpg（4797×2763），输出计划后实施。新增 design/ui-direction-v3.md 永久规范并由 AGENTS.md、development-standards.md 和 implementation-plan.md 引用。规定原始AI Chat视觉优先、统一Tokens、适中留白、必要边框、按需信息、图谱交互与每轮实际截图验收。先图谱后其他页面，停在图谱视觉确认门槛。

新增 design-tokens.css 和 graph-design.css，V3主题仅在图谱外壳启用；其余页面保持此前样式。图谱筛选默认折叠且显示启用数量，知识检查器和执行详情按选择显示浮层，保持画布尺寸。低饱和节点保留类型形状和细腻高光，细半透明方向连线与页面浅灰蓝背景一致。复用已有公共控件和知识编辑器。

新增 GraphCamera 作为知识/执行图唯一相机控制器。点击聚焦保存探索镜头，420ms可中断动画；Esc/关闭恢复，空白拖动立即恢复后接续旋转，右键平移、滚轮缩放，尊重减少动态效果，编辑器Esc与相机隔离。聚焦不改节点位置或邻接数据。

保留本轮之前未提交的GraphInstances批量绘制、Rust关系读取、测试/配置改动和ADR0012；仅在相关前端上增量适配。既有Benchmark已由 CORTEXOS_GRAPH_BASELINE / CORTEXOS_REAL_GRAPH 开关进入独立测试项目，未删除数据、报告或普通视图功能，也未新增常驻性能面板。

### 实际命令与结果

- py -3 D:/CodexData/skills/ui-ux-pro-max/scripts/search.py 'personal workspace minimal calm' --design-system -p CortexOS：结果偏营销/健康产品，未采用；随后 'motion interruption reduced motion' --domain ux 返回匹配的动态效果与可访问性规则。外部参考搜索工具返回错误，产品交互只按用户提供的要求与本地参考落实。
- npm.cmd run build（apps/desktop）：类型检查与Vite构建通过。一次从仓库根运行同名命令因无package.json失败，随后在正确目录通过桌面构建执行前端检查。
- npm.cmd run tauri -- build --debug --no-bundle（apps/desktop）：两次通过，包含前端类型检查/构建。保留已有图模块约937kB告警。
- node artifacts/ui-v3/review.cjs：默认沙箱未提供可用调试连接，停止后以 require_escalated 运行隔离原生脚本。首轮脚本滚轮时指针停在保存按钮，修正为先移回画布；第二轮执行视图未指定回答记录，修正为选择实际run ID。第三轮成功，errors=[]，11张截图，无页面横向溢出。
- git diff --check：通过（正常CRLF/LF提示）。未运行单元测试、整套回归或Benchmark；未连接真实Ollama，使用本地HTTP合成回答生成真实应用事件。

### 原生 WebView2 验收

artifacts/ui-v3/review.cjs 使用时间戳隔离数据与WebView目录。实际检查：7个不同类型知识节点、6条关系；默认收起筛选/检查器；筛选和重置；列表选择；真实实例节点鼠标点击；聚焦和Esc恢复（比较保存的camera/target数值）；空白拖拽退出并继续旋转；右键平移；滚轮缩放；飞行动画中断后保持停止；减少动态效果；知识编辑弹窗Esc保持图谱聚焦；执行图选择与Esc退出。所有断言通过。

截图：1600×900默认/筛选/聚焦/编辑/真实节点选择/减少动态效果/执行概览/执行选择，加1366×768、800×600、375×812概览，共11张。已对照原图查看默认、聚焦、筛选、编辑、375px和执行选择截图。375px是CDP内容视口，不代表原生外窗支持低于800px最小宽度。指标与相机快照在 artifacts/ui-v3/metrics.json。

### 停止点

图谱V3切片完成，等待用户视觉确认后同步知识、对话和设置。未验证150%系统缩放、完整读屏、安装包、真实模型共存和大图新材质性能；既有Benchmark历史不代表本轮成绩。未改变数据模型或业务命令。最后仅整理GraphCamera排版，再构建核对产物与原生截图版本一致。
`npm.cmd run build` 最终排版后通过，产物仍为 index-BFsZvh0n.css、index-BtkOPcSH.js、GraphViewport-DKirpM0-.js，与原生截图检查版本一致。

## 2026-10-21 V3 剩余页面全局同步完成

### 交付

用户要求将V3剩余范围推进完。本轮复核原始 ai-chat.jpg 和V3永久规范后，输出范围与验收计划，将 AppShell 的V3从图谱专用改为全局默认。新增 workspace-v3.css，提取原图谱文件中的导航/顶栏共享规则，统一对话、知识、设置及编辑/确认/资料窗口、处理记录和版本抽屉。全局Tokens增加面板/控件/输入区圆角、内容宽度、页面边距及表面/强调变体；各页继续复用已有公共组件。

对话保留960px开放阅读区和输入框，发送及复选框采用克制紫色；知识保留240px侧栏、1040px正文外层、版本历史与来源，标题层级与必要边框统一；设置分类采用柔和分段外观，参数/说明仍按需展开。补齐筛选与版本下拉、表单和搜索的12px圆角，浮层统一20px与轻阴影。保留此前收紧的间距，不增加装饰卡片或常驻说明。

本轮产品逻辑只改共享外壳类名和样式导入；未改业务状态、命令、数据库、依赖、锁文件或相机行为。保留此前所有未提交的图谱、Rust、测试及配置改动。永久规范 design/ui-direction-v3.md、AGENTS.md、development-standards.md 和 implementation-plan.md 已更新为全局V3，取消已结束的图谱先行临时门槛。

### 实际命令与结果

- npm.cmd run build（apps/desktop）：tsc --noEmit、Vite构建通过。
- npm.cmd run tauri -- build --debug --no-bundle（apps/desktop）：两次通过，包含前端构建。最终资源 index-zlxuMGtx.css、index-7L8CMbIC.js、GraphViewport-H66BL5qP.js。已有约937kB图模块体积告警保留。
- node artifacts/ui-v3-complete/review.cjs：使用 require_escalated 运行独立原生WebView2检查。首轮29张截图通过，检查后补齐知识筛选下拉圆角和复选框强调色；针对最终样式增加窄窗口弹窗与弹窗横向溢出检查，最终 errors=[]、captures=33、overflow=[]、smoothCorners=true，所有页面V3 Token断言通过。
- git diff --check：通过，仅现有CRLF/LF提示。未运行单元测试、整套测试或Benchmark。

### 实际原生操作与截图

脚本使用 artifacts/ui-v3-complete 中独立时间戳数据库、来源文件和WebView目录、本地HTTP合成模型响应，工作区标注为“V3 全局验收（隔离数据）”；没有使用日常工作区或真实Ollama。最终退出由脚本启动的进程。

对话/知识：1366×768、1600×900、1920×1080、800×600、375×812；设置：1366×768、1600×900、800×600、375×812。知识编辑/确认、资料、处理记录额外检查375×812。375px为CDP内容视口，原生窗口最低宽度仍为800px。每次截图断言主文档、main和打开的dialog无横向溢出，并核对全局背景#FAFAF9、强调#625CAC。

实际操作：创建工作区和合成知识，发送并完成回答；打开处理记录，进入执行图并选择事件；查看知识与版本历史；修改标题→取消→继续编辑（内容保留）→填写修改说明→保存新草稿→审核并确认；手动选用已确认知识并预览资料；预览并确认导入UTF-8文件、查看行号；进入知识图谱、展开关系、打开/取消关系编辑；切换设置分类、展开参数、修改温度→跨分类返回（草稿保留）→保存设置；通过本地503响应触发连接错误、再恢复并重试成功。

已对照本地参考查看桌面对话/空态/知识/设置/版本/资料/上下文预览和375px对话、知识、设置、编辑、确认、资料及处理记录截图。最终33张PNG、metrics.json和review.cjs位于 artifacts/ui-v3-complete。

### 验证边界与停止点

V3全局界面同步完成，停止在本轮视觉交付门槛。保留现有业务能力；本次视觉与关键操作检查不替代完整业务回归、真实模型性能共存、150%系统缩放、完整读屏、安装包或大图Benchmark。未宣称新的性能成绩。

## 2026-10-21 对话气泡比例与输入区密度修正

用户指出短发言与边框比例失衡、输入框和整体对话留白过大。根因是用户article作为块占据88%宽度，消息滚动区flex:1强撑剩余高度，空态100px最小高度及输入区旧textarea最小高度叠加。

本轮在 workspace-v3.css 修正：用户气泡width:fit-content、上限min(85%,720px)、内边距8px 12px；短对话message-scroll改为flex:0 1 auto，长历史仍缩入可用空间滚动；阅读区宽度由960→1120px，标题/消息上下边距缩小、末条消息外边距归零、欢迎区不按视口强撑高度。ChatComposer仅将rows由2→1，默认26px文本区，支持原生field-sizing:content时随输入增长，上限160px，保留换行/滚动与手动调整回退；输入框内边距12px 16px，发送36px。更新永久V3规范与实施计划，其他页面与业务逻辑保持原实现。

实际执行 npm.cmd run tauri -- build --debug --no-bundle（apps/desktop）两次通过，包含tsc和Vite构建。首次截图发现空态最小高度与末条消息外边距仍制造空隙，修正后完成最终同一原生脚本复验。最终资源index-CV49kMPD.css、index-8mdNDOEO.js、GraphViewport-DQ6j45uE.js；已有约937kB图模块告警保留。

node artifacts/chat-density-v3/review.cjs 通过require_escalated运行两次；使用专用时间戳隔离数据库、WebView目录和本地合成模型响应。最终errors=[]、12张截图：空态1600，短对话1366×768/1600×900/1920×1080/800×600/375×812，多行草稿1600及375，长问题375，长历史1600/800/375。所有视口主文档和main无横向溢出。375为CDP内容视口，非原生窗口最小宽度。

原生实测：短句“你好”气泡约59.6×45.65px，单行输入框约95.6px高，12行草稿的文本区增长至160px上限；短对话输入框直接跟随消息容器。检查Shift+Enter插入换行、Enter发送、长中文/无空格URL换行、6轮历史滚动、回到最新内容均通过，field-sizing支持检测为true。已查看最终空态、短对话桌面和窄屏、长历史截图。证据在artifacts/chat-density-v3（含metrics.json、review.cjs）。

git diff --check通过，未运行单元测试、完整业务回归或真实模型性能测试。本轮仅修正对话比例与密度，停止在该视觉验收点。

## 2026-10-21 连续圆角与输入区底边修正

用户再次明确需要苹果式圆润、无突兀转折的圆角，且输入框下方留白过大。本轮检查发现旧规则分散且squircle配小半径视觉偏方，上一轮短对话输入区随消息排布导致其下方留下较大窗口余量。按用户最新指示覆盖该布局选择。

统一Token为 --v3-corner-shape:superellipse(1.4)，控件半径16px、面板28px、输入框32px、图标按钮14px、用户气泡18px；曲线从单一共享规则应用到按钮/表单/搜索/筛选/版本/图谱检查器与浮层/抽屉等。清理workspace-design.css和graph-design.css中旧squircle规则，workspace-v3.css集中管理。圆形发送按钮保留圆形。此为Apple连续圆角视觉适配，不声称与原生CALayer曲线逐点相同；旧WebView保留常规圆角回退。

对话消息区恢复使用剩余可用空间滚动，输入区靠底部，桌面12px、375px内容视口8px；空态输入区同样靠底部。辅助操作与快捷键入口桌面合为同一行，距输入框6px，窄屏换行；展开资料选择或说明仍可操作。保留之前消息气泡按文字定宽、约96px紧凑输入框、多行自动增长和发送逻辑。更新永久规范与实施计划。

实际命令：npm.cmd run tauri -- build --debug --no-bundle（apps/desktop）通过，含类型检查和Vite构建；最终资源index-BWBBl2ox.css、index-B6v0lYxL.js、GraphViewport-CKPrHaMj.js。原有约937kB模块告警保留。

node artifacts/continuous-corners-v3/review.cjs（require_escalated）通过：errors=[]，35张页面截图及1张输入框局部截图；检测当前WebView2支持superellipse(1.4)，所有被检查的可见按钮/表单/输入框/浮层等计算样式均为同一曲线；输入区底边均为7–13px容差内（桌面12、窄屏8）。对话、知识、设置与编辑/审核/资料/处理记录弹窗、图谱及执行图，1366/1600/1920/800/375相关视口均无主文档、main或dialog横向溢出；额外检查375px资料选择展开和桌面说明展开。继续检查了草稿、审核、文件导入、设置保存及连接失败恢复。

node artifacts/continuous-corners-v3/check-density.cjs（require_escalated）通过：errors=[]，另12张对话密度截图；短气泡、单行紧凑高度、多行增长上限160px、底边、长中文/无空格URL换行、6轮历史滚动、回到最新内容、Shift+Enter/Enter均通过。脚本使用各自时间戳隔离数据库、WebView目录和本地合成模型响应，结束后关闭拥有的应用进程。375px为CDP内容视口，非原生窗口最小宽度。

已查看输入框局部、桌面对话/设置/编辑、375px展开资料截图；截图与两组metrics在artifacts/continuous-corners-v3。外部文档核对使用MDN superellipse和Apple连续圆角官方文档，引用已写入永久规范。git diff --check通过；未运行单元测试、完整业务回归或性能Benchmark。本轮停止在圆角与底边视觉验收点。

## 2026-10-21 移除系统标题栏与空输入框高度调整

用户明确白条是Windows系统标题栏，并追加要求空输入框默认高度稍提高。本轮tauri.conf.json设置decorations:false、启动背景#FAFAF9，复用原64px顶栏而不增加另一条标题区。新增WindowControls.tsx：空白区域左键调用startDragging、双击toggleMaximize，右侧36px最小化/最大化或还原/关闭按钮，监听原生resize同步还原图标，浏览器不挂载原生操作。操作错误沿用studio.reportError，close沿用原关闭路径和草稿监听。workspace-v3.css设置窄视口控件布局。只为main增加allow-minimize、allow-toggle-maximize、allow-is-maximized、allow-start-dragging四项权限，复用既有close/event授权，需求与范围已写入security.md和永久UI规范；没有启用core默认权限包或文件/shell能力。

输入文本区最小高度26→42px，空输入框整体约95.6→111.6px，输入后保持稳定的最小高度，多行上限仍160px；底边仍桌面12px、窄内容视口8px。永久V3规范和实施计划已同步。

实际执行 npm.cmd run tauri -- build --debug --no-bundle（apps/desktop）三次通过，含tsc --noEmit和Vite构建。最终资源index-BvLt9UEy.css、index-BYNcGG0Y.js、GraphViewport-BKzRUZoX.js；已有约937kB图模块告警保留。git diff --check通过，未运行单元测试或完整业务/性能回归。

原生验收脚本 artifacts/frameless-window/review.cjs 和 native-window.ps1 通过require_escalated运行，只操作本次启动的隔离进程及经PID校验的HWND。初版辅助脚本生成的here-string嵌套未写出文件，改为独立文件后继续；最小化后Get-Process.MainWindowHandle误选18px辅助窗口，改为保持最初主窗口句柄并校验拥有PID，排除验收脚本误判。最终errors=[]：Win32测量正常窗口client顶部与外框顶部差1px，确认系统标题栏已移除；真实最小化IsIconic=true，最大化IsZoomed=true，还原后false；双击处理调用和拖动API成功；1600/800/375内容视口无横向溢出，窗口按钮可达；未保存知识编辑下发送原生WM_CLOSE弹出保护，继续编辑保持标题，放弃草稿后自定义关闭按钮能正常退出进程。

使用PrintWindow保存真实完整窗口截图native-window.png（包含原生边缘），另有3张客户区截图client-1600/800/375.png，已查看完整窗口和375布局。实际缩放环境中客户区1800×1125物理像素，对应1440×900逻辑尺寸，记录原生数值，未把CDP模拟当真实外窗缩放。375px仅内容视口，外窗最小宽度仍800px。

真实鼠标拖动自动模拟未通过：Win32鼠标注入在管理环境没有产生任何浏览器mousedown事件，窗口位置未改变。该项记录nativeDragVerified=false、nativeMouseEvents=[]；另通过派发真实组件事件调用原生startDragging确认权限/API通路，无错误。不能宣称真实鼠标拖动已验收；边缘缩放和Windows吸附也未专项验证。未增加为测试而放宽的应用权限。

artifacts/frameless-window/check-composer.cjs 对最终构建通过，errors=[]，12张截图覆盖1366/1600/1920/800/375相关视口；约112px默认输入框、文字定宽气泡、42–160px文本区、底边、长文字/URL换行、6轮历史滚动、回到最新内容、Shift+Enter/Enter均正常。截图和两组metrics保存在artifacts/frameless-window。使用时间戳专用数据库、WebView目录和本地合成模型响应，没有使用日常数据；脚本退出后无测试应用进程残留。

窗口配置与能力用法核对Tauri官方Window Customization及本地2.12.4 API/Schema，参考链接已写入永久规范。本轮完成系统标题栏移除和输入框高度调整，停在该交付点；保留真实拖动的自动验收限制，不宣称全套窗口交互或完整回归通过。

## 2026-10-21 P1-C2a 批量绘制、等值读取与真实模型共存

### 范围与实现

用户要求继续按计划开发，并在本轮期间进一步微调UI。已复读development、development-standards、implementation-plan和最新永久V3，保留用户完成的全局Tokens/控件、GraphCamera、收起筛选/按需详情、无系统标题栏与约112px输入区；不重复设计页面。上述V3工作及独立验证保留在前面的日志，不能算作本轮新性能证据。

本轮GraphInstances按类型批量绘制节点、统一箭头实例；筛选/移动更新矩阵和包围球，点击仍映射真实实体UUID。GraphViewport保持标签最多7项、按需绘制、DPR限制及知识/执行两种事实来源。适配V3后由GraphCamera管理单击聚焦与返回探索镜头，实例标记graphNode供空白拖动判断。原生专项发现列表Esc还会被相机处理，在Graph列表处理处增加preventDefault，恢复“先关闭列表、详情仍在”的原键盘路径。

graph.rs将每条关系一次历史查询改为一次限定工作区的LEFT JOIN，返回ID/字段类型/完整历史/版本顺序保持不变。无数据库迁移、依赖、锁文件或模型调用修改；本性能切片无新增能力。已授权的四项窗口权限保留为用户最新V3外壳基线。

实现文件：GraphInstances.tsx、GraphViewport.tsx、Graph.tsx、src-tauri/src/graph.rs。同时纳入用户最新GraphCamera.tsx、App/ExecutionTrace/AppShell/ChatComposer/WindowControls、design-tokens/graph-design/workspace-v3及旧workspace样式清理、Tauri窗口配置/能力及V3规范。验证文件为playwright.config、graph-baseline/graph-instances/graph-model/graph-neighborhood/graph/trace专项。文档为ADR0012、graph-performance-p1-c2a、architecture、test-plan、implementation-plan、refactoring-plan与本日志；用户更新的AGENTS/development-standards/security/V3永久规范一并保留。原始参考图片和未改动内容的desktop.spec.ts不混入性能提交。

### 实际命令与结果

命令从独立仓库执行；npm/Playwright命令目录为apps/desktop，Rust与git命令目录为仓库根。下列均实际执行，未运行单元或整套测试。

- npm.cmd run typecheck：多次通过，最终V3及测试调整后再次通过。首次编辑曾出现TS1109（条件索引末尾多余逗号），修正后通过。
- rustfmt --edition 2024 apps/desktop/src-tauri/src/graph.rs；cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked：通过。未对无关Rust文件全局格式化。
- npm.cmd exec --yes --package=prettier@3.9.10 -- prettier --write：本轮仅整理GraphInstances/GraphViewport及新增/修改的性能专项，没有加入项目依赖。
- npm.cmd run tauri -- build --debug --no-bundle：性能版与最终V3/Esc修复版均通过，包含类型检查/Vite。最终Rust编译36.79s，资源index-BvLt9UEy.css、index-P4_8NBWo.js、GraphViewport-CgpVKyGZ.js；已有约937kB图模块告警保留。
- CORTEXOS_GRAPH_BASELINE=1、CORTEXOS_GRAPH_FIXTURE_DIR=仓库artifacts/p1-c2a-fixture，分别以before-7187f83/after-batched-render-and-query标记执行npx.cmd playwright test graph-baseline.spec.ts --project graph-baseline --output ../../artifacts/p1-c2a-before（及p1-c2a-after）：各1条通过，总50.7s/12.1s。同一库的完整GraphData哈希三规模全部相等。
- npx.cmd playwright test graph-instances.spec.ts graph-neighborhood.spec.ts graph.spec.ts trace.spec.ts --project desktop --output ../../artifacts/p1-c2a-checks：3通过、trace因旧选择器失败；trace两次针对性复核，先修隐藏版本文字、再修旧“定位原消息”按钮名，最终p1-c2a-trace为1条通过/10.4s，原有业务断言保留。
- CORTEXOS_REAL_GRAPH=1执行graph-model.spec.ts --project real-graph，输出p1-c2a-model：1条通过，但模型很快拒绝软件话题，不能作为持续共存证据。改用独立数字表格问题并要求交互前后均streaming，输出p1-c2a-model-long：1条通过/47.7s，2048 Token、模型总耗时约32.4s；此为V3更新前结果。
- 最新UI先执行上述四个desktop专项，输出p1-c2a-v3-checks：3通过，关联专项因列表Esc同时退出聚焦失败；修复后重新桌面构建。
- CORTEXOS_GRAPH_BASELINE=1、CORTEXOS_REAL_GRAPH=1、同一fixture目录及final-v3-batched标记，执行npx.cmd playwright test graph-instances.spec.ts graph-neighborhood.spec.ts graph.spec.ts trace.spec.ts graph-baseline.spec.ts graph-model.spec.ts --project desktop --project graph-baseline --project real-graph --output ../../artifacts/p1-c2a-v3-final：4个交互专项与1个三规模基线通过；2048 Token真实模型完成等待150s超时，结果5通过/1失败，总3.7min，保留失败报告。不能按PowerShell包装器返回码代替Playwright失败结果。
- 将共存专项默认上限设为产品起始512 Token，保留CORTEXOS_GRAPH_OUTPUT_LIMIT=2048独立压力入口。CORTEXOS_REAL_GRAPH=1、同一fixture执行npx.cmd playwright test graph-model.spec.ts --project real-graph --output ../../artifacts/p1-c2a-v3-model-default：1条通过/28.5s（用例24.5s），保留终态、配置、实际事件及数据哈希断言。没有以此覆盖长回答失败。
- py -3 -X utf8只读汇总JSON，断言before/after/final-v3三版本每个规模完整GraphData哈希一致，通过。为排查长回答读取隔离SQLite，曾误用不存在的finish_reason/事件created_at列，失败后改按实际messages结构读取，未修改数据库。默认py编码及缺少PIL的初期检查失败后使用UTF-8和标准库读取图片信息，未安装库。
- 长回答等待排查中，默认沙箱拒绝CIM读取；require_escalated后只取得本次应用子进程的调试端口。随后只读CDP连接时测试已退出，ECONNREFUSED，未据此声称读取了运行中的界面。
- git rev-parse --show-toplevel为D:/vibecoding/CortexOS；git check-ignore Gittoken.txt确认忽略；credential.helper为manager。git diff --check通过，仅现有换行提示；未读取令牌正文。

### 实测结论与截图

V3更新前同条件比较：1000节点/5000关系上一帧绘制提交6001→4，打开至首画面600.8→240.2ms；读取往返585.3→573.0ms，不能宣称SQL延迟显著改善。最终V3画布高度626→670，单列观测：同规模打开346.2ms、读取963.0ms、提交4、350ms空闲新增0；不把视觉和设备负载差异归为优化收益。完整表格、方法与限制见graph-performance-p1-c2a.md。

最终512场景在图谱拖动及筛选前后仍在生成，输出增长，最终completed/length、512 Token、862字符，模型总耗时约15.1s；图谱/历史哈希不变。设备GPU生成中93–96%，显存5897–6155MiB，结束采样7396MiB/8188MiB；这是整块设备，非本应用独占。2048场景未在150s内结束，磁盘已有2253字符，加载前设备4568MiB、加载后7576MiB、交互GPU98–99%；根因未证明，保留高负载长回答限制。未清理其他应用、改变GPU设置或静默降低用户实际配置。

已对照本地AI Chat参考并查看最终原生截图：8节点形状、大图、1366×768/1600×900/1920×1080关联与详情、375px知识图、执行图、默认真实模型并行画布。三桌面尺寸无横向溢出；375px为CDP内容视口，非实际外窗最小尺寸。原有编辑/审核、四个关系修订、视角与重启、知识版本/消息导航、WebGL降级和键盘焦点专项均通过。截图只证明实际检查范围；未重新跑全部来源/聊天业务。独立证据目录见上述artifacts路径，没有以静态图片替代界面。

### 交付门槛与剩余事项

本切片完成批量绘制和默认模型设置下的可用闭环，停止在P1-C2a。高负载2048 Token场景仍未通过，完整性能验收尚未完成；后续需拆分模型响应、流式刷新/内存和读取开销。千节点标签仍会重叠，邻域模式/避让后续另做；仍为全图/全部历史读取，不称数据库分页。多中心展开/收起、系统150%缩放、完整读屏、长时/热机和安装包各有独立门槛。用户先前V3窗口日志中的真实鼠标拖动/边缘吸附验证限制继续保留，不因本轮构建通过而消失。

## 2026-10-21 后续开发计划细化

用户询问继续开发的计划。本轮核对git状态/9f8cad9、现有实施与纠偏计划、性能报告，并用rg检查studio流式更新、Graph全图读取/筛选/投影及Rust命令入口。细化六步顺序：长回答和刷新定位→按需读取→多中心探索→图谱阅读→Windows交付→Decision Lab/评估；下一实施切片限定为第一步。详细范围和可验证标准写入implementation-plan。

本轮仅修改计划和记录，未实施上述优化，未运行类型检查、构建、单元或集成测试。用户参考图片及原desktop.spec.ts改动保留；无数据库、业务代码、模型参数、依赖或系统设置变化。

## 2026-10-21 长回答与图谱刷新隔离

用户确认实施后续计划第一项。复读development、开发规范、实施计划与AGENTS，先测量再优化，沿用永久V3；本轮未推进数据库按需读取或多中心探索。

### 修改及依据

代码路径为studio流式Channel→根App→完整studio属性→GraphPage→KnowledgeGraph重新筛选及新建数组→GraphViewport的缓冲/实例/标签效果。固定流原生基线证实200段文字期间，静置千节点/五千关系图谱标签层增删各206次、绘制307次。修改App使用useMemo组合图谱所用四项稳定依赖，GraphPage使用React.memo默认比较；GraphStudio/KnowledgeInspector以Pick限定方法依赖。导航订阅、自身状态、工作区和knowledgeRevision继续更新；执行图继续真实轮询。两个方法均沿用已有useCallback及当前view引用。

保留原片段更新节奏、seq检查、取消、保存点和终态，不增加刷新节流、状态库、Worker、自定义比较器、数据库迁移、业务命令、能力或依赖。仅性能标记启用时，studio记录一条有界准备IPC耗时；测试转发观察真实Ollama首文字及结束，不写虚构执行事件或普通UI指标。

实际产品文件为App.tsx、Graph.tsx、ExecutionTrace.tsx、Knowledge.tsx、studio.ts；检查文件为playwright.config.ts、stream-profile.spec.ts、graph-model.spec.ts、chat.spec.ts；文档为ADR0013、streaming-performance、architecture、test-plan、implementation-plan、refactoring-plan、本日志及旧性能报告的后续说明。用户参考图和desktop.spec.ts改动保留。

### 实际命令和检查

以下npm/Playwright命令目录为apps/desktop；原生操作使用require_escalated与隔离数据库/专用WebView目录。未运行单元或全套测试。

- npm.cmd run typecheck：实现稳定props后、计时修改后及最终脚本排版后通过。最初按不存在的src-tauri/src/context.rs查找计时路径失败，随后用rg定位实际db.rs/retrieval.rs，未更改这些文件。
- CORTEXOS_STREAM_PROFILE=1、fixture为artifacts/p1-c2a-fixture、variant=before-fa52fd6：npx.cmd playwright test stream-profile.spec.ts --project stream-profile --output ../../artifacts/stream-before：1条通过，总12.5s，用例10.6s；使用改动前原生二进制。
- CORTEXOS_REAL_GRAPH=1、同fixture/variant，依次CORTEXOS_GRAPH_ACTIVITY=idle/interactive运行npx.cmd playwright test graph-model.spec.ts --project real-graph，输出stream-model-before-idle/stream-model-before-interactive：各1条通过，总17.9s/13.4s，用例15.8s/11.5s。两次实际请求哈希一致。测试端替换invoke未捕获准备计时，该项缺失；后改为独立Performance标记，不推导旧版准备耗时。
- npm.cmd exec --yes --package=prettier@3.9.10 -- prettier --write tests/stream-profile.spec.ts tests/graph-model.spec.ts；最终又整理graph-model/chat脚本。未新增依赖。
- npm.cmd run tauri -- build --debug --no-bundle：两次通过，含类型/Vite构建，Rust部分14.65s/10.21s。第二次加入有界准备计时；最终资源index-BvLt9UEy.css、index-DkvE4lSB.js、GraphViewport-BJsOR2H2.js。CSS产物哈希与之前相同，已有约937kB图模块警告保留。
- CORTEXOS_STREAM_PROFILE=1、CORTEXOS_EXPECT_STABLE_GRAPH=1、同fixture、variant=after-stable-graph：npx.cmd playwright test stream-profile.spec.ts chat.spec.ts graph-instances.spec.ts graph.spec.ts trace.spec.ts --project desktop --project stream-profile --output ../../artifacts/stream-after：4通过/1失败，总37.8s。固定流、实例点击/聚焦恢复、知识/关系/视角与重启、执行事件及精确版本/消息导航均通过；chat旧预期漏掉已实现的citations_checked事件。
- 补齐chat事件后，npx.cmd playwright test chat.spec.ts --project desktop --output ../../artifacts/stream-chat-check：在9.9s因旧云模型错误文案失败；核对当前Rust和界面，同步云模型拒绝及输出上限文字，保留调用数量、事件顺序和持久化断言。再次输出stream-chat-final：1条通过，总38.0s，用例35.7s。实际覆盖跨工作区/草稿、取消、异常NDJSON、正常关窗取消、强制中断后重启恢复、云模型/缺失模型阻止、连接失败恢复、30秒超时和长度上限后重试。
- CORTEXOS_REAL_GRAPH=1、同fixture、variant=after-stable-graph，按512/idle、512/interactive、2048/interactive依次设置CORTEXOS_GRAPH_OUTPUT_LIMIT与CORTEXOS_GRAPH_ACTIVITY，运行graph-model.spec.ts --project real-graph，分别输出stream-model-after-512-idle、stream-model-after-512-interactive、stream-model-after-2048-interactive：各1条通过，总17.2s/14.3s/34.5s，用例15.1s/12.2s/32.5s。
- py -3 -X utf8只读汇总上述JSON，断言四次512实际请求哈希相同、五次GraphData哈希相同，通过。真实模型检查五次serviceStartedByCheck均为false，保留原服务；没有调整系统环境、结束其他应用或下载模型。
- git diff --check通过，仅已有换行提示。提交前确认独立仓库根目录；保持令牌与artifacts忽略，不读取或提交凭据。

### 实测与验收边界

固定流前后采样墙钟6311.5/6286.3ms：标签层增删206/206→0/0，静置绘制307→0，渲染器累计任务1968.7→117.4ms，JS堆样本55.8→13.5MiB。随后筛选正常，200段最终文字逐字一致落盘，图谱未改变。模拟HTTP不是模型吞吐或屏幕FPS；JS堆不是进程内存上限。

真实512静置/交互前后请求相同。静置观察绘制291→0、任务1889.4→165.7ms；交互任务1169.1→301.8ms，操作时间和采样间隔不同，不当作CPU利用率。模型吞吐仍约74–77 Token/s；收益是界面重复工作减少，没有观测到推理加速。新准备IPC约11.5–11.8ms，包含设置/上下文/事务/IPC；检索366–379微秒，独立于模型输出耗时。详表见streaming-performance。

2048场景实际模型总耗时27.671s、终态completed/length、输出3035字符；交互前后仍在生成且文字增长，保存请求、事件、输出及图谱哈希一致。此时设备显存加载后约4405MiB、交互约4439/4300MiB，与上轮约7576MiB不同；未复现旧高负载超时，根因继续未确定。未以延长超时或降低用户参数冒充修复。

已查看本轮实际原生1600×900真实模型静置图、375px知识详情/筛选、执行记录截图；布局/样式保持原V3。375px为CDP内容视口，执行图PNG为默认窗口的物理截图，不冒称实测系统150%缩放。图谱与知识编辑/审核、关系历史、WebGL降级、原消息与知识版本定位的原专项通过。来源文件权限/完整业务、三尺寸视觉全量、完整读屏、安装升级和持续高GPU负载未重跑。

停止在本切片门槛。下一步按计划先明确图谱按需读取的摘要、有效关系、邻域、历史分页/总数、请求并发和旧界面兼容契约，再增量实现；此轮无数据库或API重构。高负载模型问题保留独立复现入口。


## 2026-10-21 图谱按需读取

用户“做”确认后续计划第2项。复读总纲、开发规范、实施计划、AGENTS及永久V3；沿用已有数据与页面，先记录ADR0014读取契约，再交付分页和历史按需读取，停在该项验收点。使用ponytail技能复用现有连接/命令/详情/UPSERT，不增加状态框架或页缓存。

### 实际修改

- Rust新增graph_query.rs：节点摘要/过滤/直接关联SQL页最多200；单节点摘要；加载端点间有效关系；当前关系页50；历史version游标页50。工作区/参数校验、真实总数、跨页快照与稳定全工作区布局序号。commands.rs/lib.rs/build.rs和main capability注册五项只读业务命令。
- Cargo.toml只为现有rusqlite启用functions，使用确定性Unicode小写子串匹配；无新crate/版本、迁移、表或索引。Cargo.lock与package-lock.json内容未变。
- graph-query.ts、Graph.tsx与新GraphRelations.tsx：正式知识图替换全量读取，初始200/直接关联50，显示真实已加载/总数，关系和历史按需展开，丢弃迟到响应，失败保留草稿并提供重读。端点选择仅打开编辑器时读取全工作区摘要；冲突仅读取具体关系最新版本。位置只提交改动ID，保留跨页草稿及未加载坐标。
- graph-data.ts兼容可选布局序号/已存坐标；演示/执行图保留原事实来源。按钮文案改为“适应当前视图”“重置已加载节点布局与视角”，没有修改V3样式文件。
- 检查文件：playwright.config.ts、新graph-query.spec.ts、graph-fixture.ts及原graph/graph-neighborhood/graph-preview/graph-baseline/graph-model/stream-profile专项。后两类千节点脚本增加明确加载全图步骤，避免首页200节点冒充千节点性能。
- 文档：ADR0014、graph-on-demand-reads、architecture、security、test-plan、implementation-plan、refactoring-plan及本日志。用户参考图片、desktop.spec.ts原换行差异保留且不纳入提交。

更正检查中最初关于“保存会删除未提交坐标”的判断：原graph.rs已经使用UPSERT，不存在该删除逻辑，本轮没有修改保存服务。实际需要的是前端只提交改动坐标与跨页草稿保留。

### 实际命令与修复记录

npm、npx与cargo命令目录为apps/desktop；原生检查均使用隔离数据库/专用WebView目录与require_escalated。没有运行单元或全套测试。

- Rust首次检查遇到可选坐标闭包E0282，补显式Result类型后通过。早期桌面构建在新检查脚本遇到未使用GraphNode导入及rootNodeId被推断为null，修正后通过。最后执行cargo check --manifest-path src-tauri/Cargo.toml --locked通过（0.58s）。
- npm.cmd run typecheck在接口接入、检查扩展及最终排版后通过；npm.cmd run tauri -- build --debug --no-bundle多次构建修复后的实际WebView2版本通过。保留Vite现有GraphViewport块大于500kB提示，不当作构建失败或已处理体积问题。
- npx.cmd --yes prettier@3.9.10 --write仅整理本轮Graph、GraphRelations、graph-query及相关检查脚本。py -3 -X utf8仅用于目标文件编辑和只读JSON汇总，无库安装。
- npx.cmd playwright test graph-query.spec.ts --project desktop --output ../../artifacts/graph-query：首次数据准备因关系未变化正确被拒绝，改为实际交替关系类型建立101修订；随后发现画布未就绪时列表选择可能不开详情，修正为直接开详情，未删掉检查。
- graph-query.spec.ts graph-neighborhood.spec.ts graph-instances.spec.ts graph.spec.ts --project desktop，输出graph-query-regression：前三条通过，原graph专项发现关系保存后新组件因revision key重挂载丢失展开状态；改为只按节点ID挂载，revision继续触发读取。
- graph.spec.ts单独输出graph-query-editor：通过关系保存与并发比较路径后停在旧计数文案断言。适配“已加载”后与扩展query脚本一起输出graph-query-final：发现清除筛选时向仍禁用按钮过早聚焦，改为读取结束后恢复；第一次迟到响应注入未观测到延迟，改为fetch层暂缓实际IPC响应，未虚构返回数据。
- npx.cmd playwright test graph-query.spec.ts graph.spec.ts graph-neighborhood.spec.ts --project desktop --output ../../artifacts/graph-query-acceptance：3条通过，总24.1s。覆盖跨页及101历史、迟到真实响应、端点选择、草稿、审核、并发、重启、邻域筛选/分页及键盘路径。查询脚本后续调整截图滚动位置，在graph-query-visual-final再次通过（6.5s）。
- 设置CORTEXOS_GRAPH_BASELINE=1、CORTEXOS_STREAM_PROFILE=1、CORTEXOS_EXPECT_STABLE_GRAPH=1、CORTEXOS_GRAPH_FIXTURE_DIR为artifacts/p1-c2a-fixture、CORTEXOS_GRAPH_VARIANT=on-demand-reads；执行npx.cmd playwright test graph-instances.spec.ts trace.spec.ts graph-preview.spec.ts graph-baseline.spec.ts stream-profile.spec.ts --project desktop --project edge --project graph-baseline --project stream-profile --output ../../artifacts/graph-query-measurements：原生实例点击、执行图、三规模读取基线及固定流4条通过；浏览器演示因前轮V3文案/折叠筛选断言已过时失败。
- 浏览器演示分别在graph-query-browser、graph-query-visual-final定位旧“清除筛选”隐藏入口及已改名的演示条目，按实际V3交互更新检查，不更改演示产品功能。最终npx.cmd playwright test graph-preview.spec.ts --project edge --output ../../artifacts/graph-query-browser-final：1条通过，总5.0s。浏览器控制台保留Three.Clock弃用提示，没有新增运行错误。
- py -3 -X utf8比较三个规模的全量GraphData哈希，与p1-c2a-v3-final逐一一致；新links逐项等于全量中两端已加载的当前有效关系。git diff --check通过，仅既有CRLF提示。提交前git rev-parse --show-toplevel确认D:/vibecoding/CortexOS，Gittoken.txt仍忽略，credential.helper=manager；未读取令牌正文。

### 结果、截图和门槛

同库每组3次全量/首页+links往返中位：100节点/1000边101.2→109.7ms；500/1000为175.2→93.2ms；1000/5000为546.8→143.3ms。千节点首页为200节点/985条连线，JSON从1,782,087到359,719字节（减少79.81%）。小图略慢；往返包含SQL/IPC/序列化/CDP，不声称纯SQL或全部规模提速。前缀策略下从首页继续加载全部千节点另耗1652.5ms，不能把236.0ms首页首帧当成全图加载时间。

千节点/五千关系明确全部加载后固定流200段：6.302s观察窗内标签增删0/0、静置绘制0，累计主线程任务123.3ms。完整输出和图谱哈希断言通过，不是本轮真实模型吞吐或屏幕FPS。三个规模空闲350ms均无新增绘制，上一帧提交均4。详细方法和原始记录位置见graph-on-demand-reads。

已生成并查看实际原生筛选详情、历史展开及直接关联截图；1366×768、1600×900、1920×1080内容视口无横向溢出，375px降级及执行图沿原专项检查。WebView2截图与浏览器演示分别记录，未以CDP视口冒称系统150%缩放。

本轮保留限制：前缀页会重复读取；计数/排序/快照仍可能扫描工作区；密集有效关系尚无边分页；端点选择打开时仍读全工作区摘要；10,000节点请求上限与1,000改动坐标保存上限不是流畅规模保证。未重跑真实Ollama长回答/高显存复现、系统150%、完整读屏、安装升级与长时稳定性，原高负载模型问题仍未确定根因。

停止在第2项“按需读取”验收点。第3多中心探索、第4阅读整理、第5桌面交付、第6Decision Lab/评估继续后续分段实施，不宣称整个P1-C或项目已完成。


## 2026-10-21 多中心关联探索

用户“继续”确认六步计划第3项。复读总纲、开发规范、实施计划、AGENTS及永久V3，先记录ADR0015的范围来源和收起语义，沿用已交付的按需读取。执行中用户重新提供AGENTS，继续遵守“不跑非必要单元测试、当前切片验收后停止”；没有扩大到标签/安装包/P2。

### 实际修改与判断

- graph_query.rs在原query_graph_nodes增加可选rootNodeIds，兼容单中心、互斥、最多20、不重复、逐项工作区校验；参数绑定JSON集合取得真实去重总数。空集合与省略明确不同，没有新命令、权限、表或迁移。
- graph-query.ts保存有序展开列表及nodeID到全部展开来源的映射；各中心初始50并独立继续读取，节点去重后读取有效连线，全批共用快照。演示显式走内存匹配页与相同合并流程，不调用原生数据库。
- Graph.tsx把range改为expansions，保留原“只看直接关联”，追加/收起逐个来源，全部收起进入空探索，显式返回全图。共享节点/位置、当前选中详情及临时显示保留；跨页、工作区、演示分别保留会话内状态。相机与坐标沿用原保存服务，不写知识或执行事实。
- 新GraphExplorations.tsx及graph-design.css复用V3控件实现最高160px可滚动范围列表；两个探索操作间留8px，显示范围折叠区放在知识正文之后，标题/状态/审核仍优先。没有新灰字说明区、卡片体系或视觉特效。
- 修复两处时序焦点：Graph在读取中保留请求，等提交/启用后聚焦；GraphRelations在新关系条目实际提交后，通过组件内引用定位第一个新条目。原键盘断言保持。读取失败明确当前仍是上次成功范围，并保留位置草稿。
- 检查文件为playwright.config.ts、新graph-exploration.spec.ts及graph-preview.spec.ts；文档为ADR0015、graph-multi-center、architecture、security、test-plan、implementation-plan、refactoring-plan及本日志。用户desktop.spec.ts换行差异、参考图和后来新增的reference/cortexos-home.html保留，不读取或纳入本轮提交。

使用ponytail技能保持现有命令/状态/坐标保存实现；ui-ux-pro-max对focus restoration removed control及focus return after close两次检索仅返回通用键盘/按钮/焦点外观指导，没有已验证的收起后焦点恢复专项匹配。具体返回路径依项目交互推导并在原生检查，不声称用户研究或新设计系统。

### 实际命令与结果

以下npm/npx/cargo/rustfmt命令目录为apps/desktop；原生检查使用require_escalated、隔离数据库与专用WebView目录。没有运行单元或全套测试，也没有安装系统软件、模型或新依赖。

- npm.cmd run typecheck首次发现一处旧range变量残留（TS2552），修正后通过；后续类型检查随每次桌面构建通过。
- npx.cmd --yes prettier@3.9.10 --write整理Graph、graph-query、GraphExplorations、新检查及graph-preview；后续另整理GraphRelations。rustfmt --edition 2024 src-tauri/src/graph_query.rs仅格式化该文件。
- npm.cmd run tauri -- build --debug --no-bundle先后构建读取/界面、临时显示保留、Graph焦点及关系分页焦点版本，均通过。最后一版Rust编译24.25s，实际程序在src-tauri/target/debug/cortexos-desktop.exe。Vite仍提示既有GraphViewport块大于500kB，没有把提示当失败或宣称已消除。
- npx.cmd playwright test graph-exploration.spec.ts graph-neighborhood.spec.ts --project desktop --output ../../artifacts/multi-center-first：2条通过，总15.1s。覆盖新多中心与旧单中心的主要路径。
- 增加临时显示跨收起与纯浏览器演示多中心后，npx.cmd playwright test graph-exploration.spec.ts graph-query.spec.ts graph.spec.ts graph-preview.spec.ts --project desktop --project edge --output ../../artifacts/multi-center-acceptance：3条通过、原graph专项清除筛选焦点失败，总45.0s。按读取/DOM提交时序修复产品代码，没有删除断言或增加固定等待。
- npx.cmd playwright test graph.spec.ts graph-exploration.spec.ts --project desktop --output ../../artifacts/multi-center-focus：2条通过，总26.6s；用例10.9/11.8s。保留全部业务断言及最终三尺寸实际截图。
- cargo check --manifest-path src-tauri/Cargo.toml --locked通过，3.85s；锁文件与npm锁文件内容未改动。
- 共用焦点修复后补验npx.cmd playwright test graph-neighborhood.spec.ts graph-preview.spec.ts --project desktop --project edge --output ../../artifacts/multi-center-keyboard-final：浏览器1条通过（3.8s），原生关系分页聚焦失败。定位为GraphRelations的RAF可能早于新条目提交；改为组件提交后effect聚焦。原检查未变。
- 最后npx.cmd playwright test graph-neighborhood.spec.ts --project desktop --output ../../artifacts/multi-center-relations-final：1条通过，总13.7s，用例9.9s；涵盖关系50→56、节点分页、单中心筛选、跨页/工作区、窄屏及WebGL降级。浏览器保留Three.Clock弃用提示，没有新增页面运行错误。
- py -3 -X utf8仅编辑目标代码/文档，无库安装；第一次view_image使用错误的推测产物目录，随后rg定位实际路径并查看真实图片，未伪造截图。git diff --check通过，仅既有CRLF提示。提交前确认独立仓库根目录，Gittoken.txt保持忽略，credential.helper为manager，未读取令牌正文。

### 验收证据和边界

70节点夹具：A一跳59，B一跳17，并集65；初始各50/17合并56，继续A为65，收起A余17，最后一个收起为0，未把重叠相加。包含实际A→B→共享节点→A的环、停用边和未展开两跳。显式临时显示的独占节点在收起所有来源后仍留1个；选择详情不丢、共享坐标草稿保持12并显式保存。浏览前后知识与关系逐项相等；跨工作区、重复、21个根和单/多根同时传入拒绝；真实旧集合IPC延迟后收起A，结果仍保持新范围。旧Unicode/261节点/101修订历史和跨页端点编辑通过，知识编辑审核、关系冲突比较、视角重启继续可用。

已查看multi-center-focus中1366×768、1600×900、1920×1080实际原生截图，均检查无横向溢出；标题、正文优先，范围为紧凑列表，解释只在折叠详情。原单中心专项另覆盖375px降级视口。纯浏览器双中心路径5→4→0→7另验，不以此冒称WebView2。CDP内容视口不等同于系统150%缩放。

当前上限20个展开中心、合计10,000读取额度、最终10,000显示节点及原1,000改动坐标保存限制，是输入/交互约束，不是已测性能保证。仍重取前缀页，计数/排序可能扫描工作区，密集连线未单独分页。没有新千节点/20中心/真实Ollama性能、完整读屏、系统150%、高显存复现、长时或安装升级验收；此前高负载模型根因未确定限制继续有效。相机保留当前视角，不因展开强制适应；视口裁切及标签遮挡交下一阅读切片。

本轮停止在第3项验收点。后续剩第4图谱阅读整理、第5Windows桌面交付、第6Decision Lab/评估；LoRA继续后置，不宣称整个P1-C或项目完成。
## 2026-10-22 图谱阅读整理

用户“继续”确认六步计划第4项。复读总纲、开发规范、实施计划及V3，先记录ADR0016，沿用现有数据、相机和渲染实现。没有修改数据库、知识引擎、模型调用、权限或依赖，锁文件未改。

### 修改及代码检查

新增GraphLabels.tsx，在知识图和执行图复用24个DOM标签槽，最多7个节点标签；选中优先，其次直接邻接节点，未选中优先屏幕中心。按投影边界、标签和实际面板矩形避让，短引线保持锚点；长标题省略，草稿状态独立保留。尺寸批量写入/测量，观察器只对实际元素变化更新订阅。代码检查发现重订阅ResizeObserver可能自触发，已在原生验证前修正。

GraphViewport.tsx移除旧标签层，复用现有颜色Token突出选中节点的实际相接连线，保留其他形状、材质和方向。Graph.tsx增加默认关闭的“显示选中节点的连线名称”，每次最多3个，按工作区/演示分别保留会话偏好；原完整关系与历史入口保留。graph-design.css沿用V3。新增graph-reading.spec.ts并注册；graph-baseline.spec.ts仅增加实际标签数量观测。文档更新ADR0016、graph-reading、architecture、test-plan、implementation-plan、refactoring-plan、V3和本日志。用户desktop.spec.ts换行差异及reference目录新增文件保留、不纳入提交。

使用ponytail技能保持现有依赖与渲染路径；ui-ux-pro-max本地检索chart label overlap（chart域）及data visualization labels clutter（ux域）返回无关的图表/表单标签指导，没有经过验证的图谱标签专项匹配；具体设计依项目规范与实际测量，不声称用户研究。

### 实际命令和结果

npm/npx命令目录为apps/desktop；原生构建及检查使用require_escalated、隔离数据库和专用WebView目录。遵守用户指示，没有运行单元或全套测试，没有安装系统软件或新依赖。

- npm.cmd run typecheck首次因移除旧标签后遗留清理代码出现TS1128/TS1005。第一次修正脚本在apps/desktop目录使用仓库根相对路径导致FileNotFoundError，没有写入其他位置；随后在正确目录移除残留并修正颜色缓存和观察器，类型检查通过。
- npx.cmd --yes prettier@3.9.10 --write整理GraphLabels、GraphViewport、Graph和graph-reading检查文件。npm.cmd run tauri -- build --debug --no-bundle通过类型检查、Vite及Rust编译，首次Rust13.03s；保留现有GraphViewport块大于500kB提示。
- npx.cmd playwright test graph-reading.spec.ts --project desktop --output ../../artifacts/graph-reading-first：1条通过，总9.2s，用例6.9s。查看实际截图发现长标题把草稿标记裁切，修正为独立flex标题和状态，并增加标记位于标签框内的检查，没有删除原断言。
- 再执行prettier整理GraphLabels和graph-reading；npm.cmd run tauri -- build --debug --no-bundle通过，最终Rust18.93s。没有后端代码修改。
- 设置CORTEXOS_GRAPH_BASELINE=1、CORTEXOS_STREAM_PROFILE=1、CORTEXOS_EXPECT_STABLE_GRAPH=1、CORTEXOS_GRAPH_FIXTURE_DIR为artifacts/p1-c2a-fixture、CORTEXOS_GRAPH_VARIANT=graph-reading；执行npx.cmd playwright test graph-reading.spec.ts graph-instances.spec.ts graph-exploration.spec.ts trace.spec.ts graph-preview.spec.ts graph-baseline.spec.ts stream-profile.spec.ts --project desktop --project edge --project graph-baseline --project stream-profile --output ../../artifacts/graph-reading-acceptance：7条通过，总59.4s。浏览器演示2.9s、原生多中心7.0s、实体点击3.6s、标签阅读6.0s、执行图9.1s、三规模观测13.7s、固定流12.1s。浏览器保留Three.Clock弃用提示，无新增页面运行错误。
- 最终npm.cmd run typecheck再次通过。py -3 -X utf8只读汇总JSON并断言三个规模完整GraphData哈希与graph-query-measurements一致，通过。git diff --check最初提示本日志尾部多余空行，移除后通过，仅既有CRLF提示。提交前确认git rev-parse --show-toplevel为D:/vibecoding/CortexOS，credential.helper=manager，Gittoken.txt仍忽略，未读取令牌正文。

### 实际截图、观测和边界

已逐一查看graph-reading-acceptance的长标题、密集选中、1366×768、1600×900、1920×1080实际原生内容视口截图，以及千节点和执行图截图。标签矩形不互相重叠、不与实际面板重叠且在画布内，数组末尾选中标签优先，草稿标记完整，连线开关及实际关系历史可用；相机外/后方节点不显示标签。浏览前后完整数据相等。三尺寸无横向溢出；CDP内容视口不等同于系统缩放验收。

同一隔离夹具100/1000、500/1000、1000/5000节点/关系，标签槽均24，截图时显示7/7/4个；首页首帧868.6/158.9/224.5ms，继续加载全图4.1（无需后续页）/319.4/1653.9ms，空闲350ms均新增绘制0、上一帧提交4。千节点固定HTTP流200段观察6.290s，标签增删0/0、静置绘制0、主线程任务118.2ms、JS堆样本约35.7MiB，文本及图数据断言通过。不是严格同负载前后对照、真实Ollama吞吐、屏幕FPS或峰值内存保证。

避让只解决文本/面板矩形，不解决3D几何遮挡或节点布局。选中节点在镜头外、面板覆盖或无空位时，标签可隐藏，全文仍在列表/详情。24槽限制DOM数量，候选投影和排序仍随已加载节点/相关边增长；没有10,000节点、20中心性能承诺。系统150%、完整读屏、真实模型高负载/长时、安装升级未验；此前高显存长回答根因仍未确定。

本轮停止在第4项验收点。后续剩第5Windows桌面交付、第6Decision Lab/评估，LoRA后置；不宣称整个P1-C或项目已完成。

## 2026-10-22 Windows内部交付：自动部分

用户继续第5项；复读development、开发规范、实施计划、AGENTS和V3，先记录ADR0017。用户异步选择“保留系统设置，提供人工检查表”，不擅改Windows缩放或启动读屏。遵守不跑非必要单元测试，只做本切片必要原生安装/恢复检查。使用ponytail保持现有NSIS、数据/模型逻辑、语义样式及已有测试；没有新增依赖或系统安装。

### 实际修改

- tauri.conf.json明确currentUser及SimpChinese/English安装语言和选择入口，正式版本仍0.1.0。
- tests/fixtures/delivery-previous.json、delivery-current.json以独立产品名CortexOS Delivery Check和com.cortexos.deliverycheck生成同源0.0.9/0.1.0调试伴随包。不是历史发行代码兼容证明。
- native.ts只新增可选executable用于启动实际安装后的程序；生产程序无调试后门变更。playwright配置用CORTEXOS_DELIVERY_CHECK显式开启独立项目。
- 新windows-delivery.spec.ts实际安装、升级、卸载，验证文件版本与完整数据、实际HWND缩放、键盘窗口按钮、可访问树和迁移备份；delivery-db.py用Python标准库在artifacts内建立6版合成库并只读校验/哈希，拒绝覆盖既有seed。
- chat.spec.ts在既有检查加入可选60/120秒流和真实HTTP连接销毁；保留取消、工作区、格式错误、超时、关闭/中断恢复及重试。没有修改业务代码、数据库、模型逻辑或V3布局，两个锁文件未改。
- 文档为ADR0017、windows-delivery、README、implementation-plan、refactoring-plan、test-plan、security和本日志。用户desktop.spec.ts换行差异及reference新增文件保留、不纳入提交。

### 实际命令与结果

以下npm/npx命令目录为apps/desktop；构建与原生操作使用require_escalated、隔离数据与专用WebView目录。没有运行单元或全套测试、没有访问正式数据。

- npm.cmd run tauri -- build --debug --bundles nsis --config tests/fixtures/delivery-previous.json：通过，Rust15.94s；构建器首次下载并校验NSIS3.13和nsis_tauri_utils0.5.3到Tauri缓存，不是系统软件安装。隔离旧版包4.04MiB。
- 同命令配置delivery-current.json：通过，Rust13.09s；当前包4.04MiB。两个包内容源码相同，仅配置版本不同。
- npm.cmd run tauri -- build --bundles nsis：通过，首次完整优化Rust3分33秒，正式包2,902,672字节。Vite保留既有940.72kB图谱块大于500kB提示；release另有lib.rs/commands.rs仅debug使用变量的两条unused_mut提示，未当失败或宣称已消除。
- 初次npm.cmd run typecheck发现测试使用目标库不支持的replaceAll及GraphNode不含content（TS2550/TS2339）。改为replace正则、断言摘要标题，全文另外由完整知识版本哈希校验；后续typecheck通过。
- npx.cmd --yes prettier@3.9.10 --write整理新检查及两份配置首次因沙箱npm缓存EPERM失败；在require_escalated下重跑通过，未改项目依赖；后续整理chat专项和最终delivery检查。
- CORTEXOS_DELIVERY_CHECK=1；npx.cmd playwright test windows-delivery.spec.ts --project delivery --output ../../artifacts/windows-delivery-first：1条通过，总21.1s、用例19.1s，覆盖实际安装升级/迁移/卸载和窗口/名称。
- npm.cmd run tauri -- build --debug --no-bundle：通过，Rust22.16s；将默认调试程序恢复正式identifier，未把伴随包留为日常开发程序。
- CORTEXOS_DELIVERY_CHECK=1及CORTEXOS_CHAT_SOAK_MS=60000；npx.cmd playwright test windows-delivery.spec.ts chat.spec.ts --project delivery --project desktop --output ../../artifacts/windows-delivery-acceptance：2条通过，总1.8分钟；安装专项7.0s、恢复专项约1.6分钟。包含60秒流、取消、真实HTTP断连接、格式错误、30秒超时、关闭取消、强制中断重启以及云端模型阻止。
- 最终补充四条知识版本/旧生效新草稿、审核记录和卸载应用文件删除断言，格式化、npm.cmd run typecheck通过；仅补跑npx.cmd playwright test windows-delivery.spec.ts --project delivery --output ../../artifacts/windows-delivery-final：1条通过，总9.1s、用例6.9s；没有重跑已通过的60秒恢复专项。
- Get-FileHash、Get-AuthenticodeSignature读取正式包：SHA256为108e763c7c76aad9c573253b847d9b8a95600e70a779f2c3da37840316d2b691，NotSigned。写实际manifest到artifacts/windows-delivery-release，不提交安装包。Python只读隔离SQLite汇总真实事件：model_response_received至run_cancelled间隔60.202s，保留379个字符、cancelled；间隔包含切工作区/点击开销，不冒称精确计时或真实模型。结果写controlled-stream-evidence.json。
- 已查看首次及最终已安装应用截图和失败恢复截图；git diff --check通过，仅既有CRLF提示。提交前再次确认独立仓库及凭据管理器，Gittoken.txt保持忽略，未读取正文。

### 结果和停止门槛

调试伴随包实际安装0.0.9→0.1.0，已安装文件版本匹配；升级前后及重启/默认卸载后全表数据哈希一致。最终夹具含2条消息、1次运行及10个事件、请求快照、4条知识版本/1次审核（旧生效版与新草稿同时保留）、2条关系修订、1份来源副本及相机/位置。完整GraphData相等。6→7迁移保留原知识和工作区，备份全表哈希与旧库一致，完整性/外键均通过；默认卸载删除应用exe，保留隔离数据。

真实HWND从原矩形调整到1200×800物理窗口，无横向溢出；GetDpiForWindow120及WebView2 DPR1.25，当前为125%环境。键盘Enter最大化/还原和关键控件可访问名称通过；没有以此冒称鼠标拖动或读屏实测。截图路径、三类构建与测量边界见windows-delivery。

本轮交付第5项自动部分，尚未达到第5项整体验收门槛。正式Release仅构建、未安装启动，安装运行证据来自隔离debug伴随包；系统150%、鼠标窗口拖动、真实读屏、至少30分钟真实模型使用和干净系统WebView2安装仍未验证。用户选择的人工检查表已交付，全部保持待执行。以前千节点/真实512和2048观测保留原边界，高显存旧超时根因没有新结论。停止，等待人工结果及后续确认；不进入Decision Lab，不标P1-C完成或公开发行。

## 2026-10-22 正式Release交由用户验收

用户要求“弹出来给我测试我来验收”。确认独立仓库、Release exe版本0.1.0及当前无同名应用进程后，经require_escalated用Start-Process启动正式Release并WaitForInputIdle；随后Get-Process及原生IsWindowVisible确认进程存活、窗口标题正确且可见。ShowWindowAsync显示窗口；SetForegroundWindow返回false，未绕过Windows前台限制。使用正常应用目录，不启用调试数据/CDP覆盖，不安装正式NSIS包，不修改缩放或读取用户数据库正文。程序留给用户操作；未运行测试，未声称页面/模型/安装及人工验收通过。当前仍在第5项，Decision Lab尚未启动。

## 2026-10-22 用户验收修复：Markdown与镜头过渡

用户反馈输入/输出不支持Markdown、聚焦和节点切换过快。本轮限定UI呈现与相机时长，继续V3和第5项验收，不进入Decision Lab。复读相关开发规范和永久V3；没有重构数据库、检索或模型逻辑，没有新Tauri权限。使用ponytail复用textarea/状态/原文及相机控制；ui-ux-pro-max查询camera transition motion duration（ux域）得到Animation/Duration Timing，匹配“根据距离与场景定时长”，不将通用数值冒称相机研究或用户测量。

### 实际修改

- 新MarkdownContent.tsx复用到AnswerContent与ChatComposer；使用固定react-markdown10.1.0、remark-gfm4.0.1、micromark-util-decode-string2.0.1。npm元数据和官方仓库说明已核对。标准解析器支持常见块/行内/GFM格式，避免手写Markdown；原有直接依赖版本不改，更新package.json与npm锁文件，Cargo.lock未变。
- 输入保留原textarea、Enter/Shift+Enter/输入法规则；眼睛/铅笔在原文和预览间切换，草稿/焦点保留，发送仍是原文。用户消息和回答共用排版，回答增加原文切换；复制继续取原文。跨格式选中文字无法对应原文时提示重新选择，避免静默换为整条。新markdown.css使用已有Tokens定义正文、标题、代码和局部滚动表格，未重排全局布局或加常驻灰字。
- 引用仍用原服务和报告。AST只转换普通文字节点，按原始UTF-16偏移逐项校验，标准解码处理转义/字符实体；粗体/表格可点击，代码/链接内为字面标记。原始HTML显示文字、不执行，危险URL不生成链接，图片只显示说明、不自动请求；未添加新权限或云端功能。
- GraphCamera原固定420ms变为按位置/目标位移限幅1100–1600ms，使用五次平滑曲线；从当前实际视角开始，保留原探索快照、新聚焦/拖动/滚轮中断、恢复及减少动态效果。除格式化外其余事件/保存语义不变。
- 新markdown-camera.spec.ts注册desktop两个必要场景；旧citations分隔点断言改为当前V3逗号，保留所有业务断言。文档为design/markdown-and-camera、V3、实施计划、test-plan、security、README、Windows交付记录与本日志。用户desktop.spec.ts差异及reference文件保留不提交。

### 实际命令和结果

npm/npx命令目录apps/desktop；构建与必要原生检查使用require_escalated、隔离库/专用WebView目录。遵守用户不跑非必要单元测试要求，没有运行单元或全套测试，没有安装系统软件。

- npm.cmd ls react-markdown remark-gfm marked markdown-it --depth=0确认无既有解析器；npm.cmd view核对react-markdown10.1.0、remark-gfm4.0.1。npm.cmd install --save-exact react-markdown@10.1.0 remark-gfm@4.0.1 micromark-util-decode-string@2.0.1完成，新增101包，用时5分钟，npm提示旧advisory端点退役；没有单独运行或声称安全审计通过。rg首次Windows通配路径无效，改用目录加-g过滤。
- npm.cmd run typecheck在接入、预览及新增检查后通过。npx.cmd --yes prettier@3.9.10 --write只整理本轮TSX/相机/检查文件；原相机文件一部分差异为排版。
- npm.cmd run tauri -- build --debug --no-bundle通过，Rust15.15s；Vite主块530.32kB/gzip160.51kB（原371.65/111.78），图谱940.90kB。新增解析能力有体积成本，主块和图谱均有500kB提示，不声称已优化或消除提示。
- npx.cmd playwright test markdown-camera.spec.ts citations.spec.ts graph-instances.spec.ts trace.spec.ts --project desktop --output ../../artifacts/markdown-camera-first：5条通过，总33.0s；引用5.0s、实例3.6s、Markdown3.6s、相机7.4s、执行图10.9s。包含原引用错误/历史/来源/重启及原节点选择、位置/探索视角恢复，未删断言。
- 补充跨格式选取保护、原文选取保存和回答顶部截图，仅运行npx.cmd playwright test markdown-camera.spec.ts --project desktop --grep "native Markdown" --output ../../artifacts/markdown-final：1条通过，总5.8s、用例3.6s。
- 补充真实空白拖动取消，单跑相机至camera-final失败：手势落点在画布左上20px，实际被top/left16px的状态条覆盖，未触发canvas拖动。改到画布内部18%/45%的空白点并先断言elementFromPoint确为canvas，保持原“不重开详情”断言，未改产品代码；npx.cmd playwright test markdown-camera.spec.ts --project desktop --grep "native focus" --output ../../artifacts/camera-drag-final：1条通过，总11.5s、用例9.4s。
- npm.cmd run tauri -- build --bundles nsis通过，Rust2分22秒。保留两条原release unused_mut提示与Vite体积提示；新内部包2,948,332字节，SHA256为cb7a1f33a1ca622aaa9a93713ee743bcf223158249d7a879043b04cc7de98ca0，仍未签名0.1.0，不公开发行。更新Windows交付记录和本机manifest，旧安装升级证据不冒充新UI包安装验证。
- 已查看实际1366×768、1600×900、1920×1080预览截图、回答顶部及动画中截图；三尺寸未横向溢出。原生检查内容视口不代表系统150%。最终相机操作至详情约1327.3/1264.1ms，含IPC/调度，空闲350ms新增绘制0；非屏幕FPS或纯动画耗时。
- 先确认无同名应用运行，Start-Process启动新正式Release并WaitForInputIdle；原生标题正确、IsWindowVisible=true，未启用测试入口。ShowWindowAsync已显示，SetForegroundWindow=false，未绕过Windows前台限制；程序留给用户验收。git diff --check通过，仅既有CRLF提示。提交前确认独立仓库和配置凭据管理器，令牌文件保持忽略且未读取。

### 边界和停止点

Markdown用原文存储，不改变请求或证据；不支持数学公式/代码着色，不自动下载图片，外链实际系统打开未专项验收；代码/链接内引用为字面文字。没有超长Markdown/真实模型长回答解析性能、系统150%、真实读屏或长时模型新结论。手感由用户继续验收，不将自动检查当人工通过；第5项其余门槛仍在。本轮修复后停止，不推进Decision Lab。


## 2026-10-22 镜头复验：应用偏好与420ms恢复

用户反馈实际聚焦/切换仍瞬间完成，随后明确将时长恢复“初版”。最终420ms，保留五次平滑、可中断及原视角语义。本轮没有数据库、模型、依赖、权限或全局UI重排；不进入Decision Lab。用户此前已要求新版弹出供测试，更新后继续交回验收。

### 检查依据和修复

- 只读SystemParametersInfo(SPI_GETCLIENTAREAANIMATION=0x1042)，调用成功，返回false；已对照Microsoft官方API字段说明。确认原运行进程3132确实来自本仓库release目录。但隔离原生WebView未仿真时媒体查询为false，两种结果不一致；未读取旧正式页面媒体查询，因此不能把系统开关断言为用户实际问题的唯一原因。前一轮的no-preference用例没有覆盖减少动态效果下显式要求平滑的情况，本轮补齐。
- GraphMotionControl复用studio的preference/remember，只导出已有两个帮助函数；默认平滑，提供跟随系统/立即切换。仅跟随系统才按媒体查询跳过；localStorage键cortexos.graph-motion，全应用知识图/执行图共用，重载保留，存储不可用时保留本会话选择。不修改Windows设置。
- 控件放在原“图例与操作/图例与记录说明”，复用V3 field、原生select和语义Tokens，不增加常驻灰字。首次检查发现label包裹select时精确名称包含选项，改成useId关联的独立label，保留可访问名称断言。
- GraphCamera按最新要求恢复固定420ms，并将动画起点延后到第一条实际useFrame回调，避免首帧前同步工作把过渡耗尽。GraphViewport沿用原观测dataset记录真实相机位置，回归核对实际插值，而非只等待检查器出现。
- markdown-camera专项增加减少动态效果下默认平滑、真实中间坐标、三个选项、重载、首帧前500ms同步工作和三尺寸截图；500ms为隔离构造输入，不是设备性能结论。固定坐标实体检查显式选立即模式，另由相机专项验动画；旧实体/执行图业务断言保留。
- 文档更新README、V3、design/markdown-and-camera、implementation-plan、test-plan、Windows交付记录和本日志。ExecutionTrace的无关格式化已去掉，仅留导入与控件两行；用户desktop.spec.ts换行差异及reference新增文件仍排除提交。

使用ponytail保持现有相机、偏好帮助函数和原生表单；ui-ux-pro-max检索reduced motion user override（ux域）匹配Reduced Motion一般指导，本轮根据用户明确要求设应用级平滑默认，并保留跟随系统/立即选项。一般指导不是该设备根因的证明。

### 实际命令和过程

命令目录apps/desktop；原生构建/检查使用require_escalated和隔离库/WebView目录。没有运行单元或全套测试，未安装依赖或系统软件，锁文件未变。

- npx.cmd --yes prettier@3.9.10 --write整理新增偏好、相机与相关检查；studio/Graph未额外格式变化。npm.cmd run tauri -- build --debug --no-bundle通过，初次Rust11.00s。
- npx.cmd playwright test markdown-camera.spec.ts graph-instances.spec.ts trace.spec.ts --project desktop --grep "native focus|instanced shapes|execution graph" --output ../../artifacts/camera-motion-preference：实体4.3s、执行图10.8s通过，偏好控件精确label未找到失败。修正独立label后构建通过（Rust9.88s）；单跑native focus到camera-motion-preference-final通过，总14.5s/用例12.3s。一次apply_patch因格式化后上下文不匹配被拒绝，没有写入；定位实际内容后应用。
- 严格核对3132可执行路径后CloseMainWindow，Exited=true、Forced=false，正常关闭旧验收程序，未强制结束或读取用户正文。
- 用户要求恢复初版发生在慢速中间Release构建中。该中间包构建成功（Rust2分20秒），未启动交付；随后恢复420ms调试构建通过（Rust9.45s）。增加实际相机位置断言后，camera-420-acceptance三条通过，总24.6s（4.2/8.9/9.0s）。对应420ms中间Release构建通过（Rust2分30秒），后被最终首帧计时/控件样式版本替代，未交付用户。
- 控件复用field后调试构建通过（11.89s），首帧计时补充版构建通过（8.88s）。中间调试/Release曾并行使用同一dist，未将这些结果作为最终交付；全部结束后最终Release单独重建，避免混用产物。
- npx.cmd playwright test markdown-camera.spec.ts graph-instances.spec.ts trace.spec.ts --project desktop --grep "native focus|instanced shapes|execution graph" --output ../../artifacts/camera-420-final：3条通过，总25.7s（实体4.3s、相机9.8s、执行图9.2s）。没有重跑已通过的Markdown正文/模型/安装升级专项。
- 最终npm.cmd run tauri -- build --bundles nsis及实际包哈希/窗口启动结果接下方。保留既有两个release unused_mut提示和Vite大块提示，不当成已解决。主块531.07kB/gzip160.77kB，图谱940.81kB。

### 最终原生证据和边界

已查看camera-420-final中1366×768、1600×900、1920×1080真实内容视口截图；控件沿用V3，无横向溢出。相机操作至详情约511.3/535.8ms（含IPC/调度，不是纯动画时间）。120ms观测：首次相机z=26.0401，位于30→14之间；切换x=0.9375，位于0→5之间。首帧前500ms同步工作后仍有中间坐标，说明未在首帧直接完成。空闲350ms新增绘制0；拖动/滚轮/快速切换、系统与立即模式、重载保存通过。隔离原生未仿真查询false，随后显式仿真reduce验证覆盖；不伪称已读到旧正式窗口的媒体查询。

仍待用户实际窗口手感复验；没有新的千节点性能、系统150%、读屏、模型长时、安装升级结果，不标第5项全部完成。

最终补记：npm.cmd run tauri -- build --bundles nsis单独完成，Rust2分24秒；当前安装包2,949,609字节，SHA256为1c067bed99cf0e22a738272a067a36eaad3d5f30fe682d09f6fa1d24825d3c23，NotSigned。更新本机manifest与Windows交付文档。Start-Process启动最终正式Release，WaitForInputIdle后原生窗口可见、标题正确（进程19148）；SetForegroundWindow=false，不绕过前台限制。新窗口留给用户操作，未操作知识或自动代替人工验收。提交前git diff --check通过（仅既有CRLF提示），确认独立仓库与credential.helper=manager，Gittoken.txt仍忽略且未读取。

### 2026-11-22 全界面原型视觉接入

真实前端接入顶部五页导航、页面边框、消息与输入区域、设置分类布局。执行 npm.cmd --prefix apps/desktop run build，通过类型检查与生产构建。使用 Node / Playwright 在 Edge 载入构建产物，切换五页、检查图谱画布就绪、切换深色与 800px 抽屉导航。实际截图位于 artifacts/ui-prototype-integration/。上述验证为浏览器预览；本次未执行原生 WebView2 或模型调用。

### 2026-11-22 五页视觉层级微调

知识库空状态集中显示可用操作；对话依据栏尊重手动偏好；定位链接合并到评测类型行；演示图首次取景使用范围适配，数据来源紧邻计数；宽窗设置页精简重复标题。执行 npm.cmd --prefix apps/desktop run build，通过类型检查与构建。使用 Edge 加载生产产物，验证五页、知识空状态、依据展开偏好重载、图谱画布就绪与评测链接弹窗。截图位于 artifacts/ui-hierarchy/。本轮为浏览器验证，未调用模型或验证原生窗口。

### 2026-11-24 极简视觉重构（令牌、控件与页面层）

用户要求整体更换视觉语言，采用 Apple × Linear 的极简风格，保留全部功能与接口。本轮重写 `styles/tokens.css`、`styles/layout.css`、`styles/controls.css`、`styles/motion.css`、`markdown.css`，并修改 `styles/features.css`、`benchmark/benchmark.css`、`benchmark/harness/harness.css`。DOM 结构、类名、`aria-*`、路由与快捷键未改动。

- 主操作改为高对比实心，品牌色降级为 `--accent`；删除玻璃模糊、内高光、渐变与逐行 inset 分割线。
- 侧栏统一为 252px（此前样式为 224px，与 `chat-design.spec.ts`、`knowledge-layout.spec.ts` 断言的 252px 不一致，本轮按测试与参考 HTML 统一）。知识列表 290px、正文 860px 保持不变。
- `text-bloom` 动画名保留（`prototype-ui.spec.ts` 有断言），但去掉模糊滤镜只保留透明度过渡。

实际命令与结果（目录 `apps/desktop`）：

- `npm run build`：类型检查与生产构建通过（Vite 6.87s，仅既有大块提示）。
- `cargo build`（`src-tauri`）：1m07s 通过，用于预热 `tauri dev`。
- `node --experimental-strip-types .capture-native.ts`：以 `tauri dev` 启动原生 WebView2，创建测试工作区并保存、确认一条知识，截图浅深两套五页。实测 1366/1600/1920 下 `.prototype-sidebar` 252px、`.knowledge-sidebar` 290px、知识正文 860px（断言上限 861）、无横向溢出。
- `node --experimental-strip-types .capture-chat.ts`：用本地假 NDJSON 服务（固定文本，未加载任何模型）完成一次发送，截图消息排版；助手消息无容器、操作行默认弱化。
- Playwright 浏览器检查：1366/900/560 三档 × 五页，`documentElement.scrollWidth` 均未超过视口；900px 以下顶栏导航按既有断点隐藏。
- `npx playwright test tests/knowledge-layout.spec.ts --project=desktop`：失败于 `waitForURL(tauri.localhost)` 超时。原因是本轮 `cargo build` 生成的 dev-profile 二进制指向 `devUrl`（127.0.0.1:1420）而非内嵌资源，属构建方式问题，不是样式断言失败；原生断言改由上面的 `tauri dev` 会话直接核对。以 `tauri build --debug` 产出的包重跑该用例仍待执行。

截图：`artifacts/ui-redesign/before`（改造前）、`artifacts/ui-redesign/s1`（浏览器与原生）、`artifacts/ui-redesign/native`（真实数据页面）。临时截图脚本 `.capture-native.ts`、`.capture-chat.ts` 已在验证后删除。

未验证：打包安装后的原生窗口未重新走查；任务中心、执行过程面板、数据可迁移对话框等次级界面只做了继承样式，未逐页截图；系统 150% 缩放与读屏未复测。

### 2026-11-25 界面排版与动效收尾（第 3–7 步）

补齐 ui-review.md 剩余的第 3–7 步视觉与交互收尾：

- **空状态与输入框**：`EmptyState.tsx` 快速开始示例点击后自动聚焦输入框并清理冗余变量；`layout.css` 将模型选择器与上下文工具行提升为 `--text-compact`（12px），为 `.composer-help > summary` 增加悬停底色与内边距；`motion.css` 为 `.message-actions .button svg`（复制/对勾切换）补入淡入过渡。
- **知识页**：`Knowledge.tsx` 去掉面包屑中的重复版本号（保留下方状态行与列表行版本）；`features.css` 对齐 `.knowledge-page > .page-heading` 与详情区 `860px` 右边界，并对齐「在知识图谱定位」文字按钮左侧。
- **设置页**：`Settings.tsx` 将「检测连接」移至服务地址输入框右侧同行（`.field-inline-action`），为 `LocalRuntimeSettings` / `RagIndexSettings` 增加 `.settings-subsections` 顶部分隔线，明确「保存设置」作用域；`.connection-status` 改用 `--text-control`。
- **图谱页与令牌清理**：`features.css` 消除 `.graph-page > .page-heading` 空白高度、放大 `.graph-legend > summary` 点击区，并将 `features.css` 中写死的字号与圆角统一替换为 `tokens.css` 变量。

实际验证（目录 `apps/desktop`）：

- `npm run build`：`tsc --noEmit` 与 `vite build` 通过（Vite 720ms，退出码 0）。
- `git diff --check`：无空白/格式错误。
- Edge 浏览器预览（`vite preview` + Playwright）：检查 `1366×768`、`1600×900`、`1920×1080` 三尺寸 × 浅/深主题 × 对话/知识/图谱/设置共 24 组组合，页面错误 0、横向溢出 0；截图保存于 `artifacts/ui-polish/`。

未验证：本次未重启原生 WebView2 桌面端重跑或重新打包安装包，亦未调用任何本地或云端模型。

### 2026-11-26 分层生成参数与云端/本地参数透传

为用户补充面向普通用户与进阶用户的分层生成控制，并修复云端模型与本地 Ollama 的参数透传：

- **Rust 后端与协议适配（由 Codex GPT 执行 `src-tauri/src/` 改动）**：
  - `models.rs`：`ModelSettings` 新增带 `#[serde(default)]` 的 `top_p`（默认 `0.9`）与 `deep_thinking`（默认 `false`），兼容既有 5 字段 `app_settings` JSON 无需迁移数据库；补充 `top_p` 范围校验（`0.05..=1.0`），并定义共享的 `GenerationOptions`。
  - `ollama.rs`：`chat_request` 在 `options` 中传递 `top_p`；仅在 `deep_thinking == true` 时附带 `"think": true`，关闭时省略以避免非思考模型报错。
  - `cloud/runtime_protocol.rs`、`cloud/runtime_connections.rs`、`db.rs`、`generation.rs`：将 `GenerationOptions` 贯通至 `CloudTarget::generation_chat_wire` / `generate_stream` 与 `Endpoint::generation_chat_wire` / `send_generation_stream`；OpenAI / Compatible 传递 `temperature` 与 `top_p`（开启深度思考时按 DeepSeek / Compatible / Official OpenAI 分别设置 `thinking` / `reasoning_effort` / `reasoning`）；Anthropic 在非思考模式下传递裁剪至 `0.0..=1.0` 的 `temperature` 并省略 `top_p`，开启深度思考时按 Anthropic 规则启用 `thinking` 且确保 `max_tokens` 与无冲突采样字段；Gemini 在 `generationConfig` 中写入 `temperature`、`topP` 及可选 `thinkingConfig`。同步更新 `benchmark/` 与测试中的 `ModelSettings` 字面量。
- **前端分层界面**：
  - 扩展 `studio.ts`（`ModelSettings`）、`studio-settings.ts`（`defaultSettings`）、`settings-draft.ts`（`rebaseSettingsDraft`）与 `tests/unit/settings-draft.test.ts`。
  - 新增 `src/components/GenerationStylePanel.tsx`，提供三档一键风格预设（「严谨准确」`0.2 / 0.85`、「均衡日常」`0.7 / 0.9`、「灵感发散」`1.1 / 0.95`）、`开启深度思考` 开关及可折叠「高级生成参数微调」（`temperature`、`topP` 滑块与 `numPredict` 输入框，附诚实跨端说明与恢复默认），分别接入「我的助手」设置面板（`AssistantSettingsPanel.tsx`）与对话输入框工具栏弹窗（`ChatComposer.tsx`）；同步在「模型连接」高级设置（`Settings.tsx`）补齐 `核采样（top_p）` 与 `开启深度思考`。

实际验证（未运行任何本地 Ollama 推理或在线云端请求）：

- `npm.cmd --prefix d:\vibecoding\CortexOS\apps\desktop run build`：`tsc --noEmit` 与 `vite build` 通过（Vite 758ms）。
- `npm.cmd --prefix d:\vibecoding\CortexOS\apps\desktop run test:unit`：4/4 通过。
- `npx.cmd --prefix d:\vibecoding\CortexOS\apps\desktop prettier --check d:\vibecoding\CortexOS\apps\desktop\src\settings-draft.ts d:\vibecoding\CortexOS\apps\desktop\tests\unit\settings-draft.test.ts`：通过（另注：`npm run format:check` 对既有未改动文件 `src/api/{chat,knowledge,memory}.ts` 有格式提示）。
- `cargo test --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked generation_options`：6 passed, 0 failed（含旧配置兼容、`top_p` 校验、Ollama 与 OpenAI/Anthropic/Gemini 协议编码）。
- `cargo test --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked multimodal`：2 passed, 0 failed。
- `cargo test --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked cloud_chat_loopback_preserves_roles_and_excludes_private_lineage`：1 passed, 0 failed。
- `cargo build --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`：通过（1m 26s）。
- `git -c core.safecrlf=false diff --check`：通过。

未验证：真实 WebView2 窗口交互截图、真实本地 Ollama 与线上云端模型推理表现待后续人工确认或授权后实测。

### 2026-11-26 分层生成参数原生 WebView2 验收与草稿同步修复

补充原生 WebView2 端到端 Playwright 用例，并在实机测试与截图走查中捕获、修复了三项前后端同步细节：

- **问题修复**：
  - `apps/desktop/src/components/GenerationStylePanel.tsx`：为「开启深度思考」复选框增加本地乐观草稿 `thinkingDraft`（`catch` 失败回滚并与 `studio.settings.deepThinking` 同步），避免异步保存 IPC 未返回前点击后回弹导致点击态异常或快速操作丢失。
  - `apps/desktop/src/settings-draft.ts`、`apps/desktop/src/studio-settings.ts`、`apps/desktop/tests/unit/settings-draft.test.ts`：扩展 `rebaseSettingsDraft(draft, previous, saved, previousSaved?)` 并传入 `current.saved`；当用户通过对话工具栏或「我的助手」面板保存预设时，「模型连接」表单未编辑的草稿字段同步更新为最新配置，同时保留用户未保存的手工修改。
  - `apps/desktop/src/Settings.tsx`：`changed` 判定由 `JSON.stringify` 改为显式按字段值比较，消除 Rust `ModelSettings`（`deepThinking` 键序在前）与前端草稿对象键序不一致引发的误报「有未保存的更改」。
- **新增原生测试**：
  - [generation-style-native.spec.ts](apps/desktop/tests/generation-style-native.spec.ts)（已注册到 [playwright.config.ts](apps/desktop/playwright.config.ts)）：使用本机 `127.0.0.1` 回环假 Ollama NDJSON 服务启动 `cortexos-desktop.exe`，验证聊天工具栏弹窗切换「灵感发散」+ 开启深度思考、`Escape` 关闭、切换「严谨准确」+ 关闭深度思考两次发送的 `/api/chat` 请求体（`options.temperature`、`options.top_p`、`think`）、我的助手高级微调 `numPredict=768` 持久化（`bootstrap`），以及「模型连接」页同步数值与「保存设置」禁用状态。

实际验证（仅使用本机回环假服务，未调用真实 Ollama 或线上云端模型）：

- `npm.cmd --prefix d:\vibecoding\CortexOS\apps\desktop run build`：`tsc --noEmit` 与 `vite build` 通过（Vite 561ms）。
- `npm.cmd --prefix d:\vibecoding\CortexOS\apps\desktop run test:unit`：5/5 通过（含新增外部预设保存草稿 rebase 用例）。
- `npx.cmd --prefix d:\vibecoding\CortexOS\apps\desktop prettier --check d:\vibecoding\CortexOS\apps\desktop\src\settings-draft.ts d:\vibecoding\CortexOS\apps\desktop\tests\unit\settings-draft.test.ts`：通过。
- `cargo build --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`：通过（17.93s）。
- `$env:CORTEXOS_NATIVE_ONLY="1"; npx.cmd playwright test tests/generation-style-native.spec.ts --project=desktop --output=../../artifacts/generation-style-native`（目录 `apps/desktop`）：1 passed（5.9s）。
- 已查看原生 WebView2 实拍截图：`artifacts/generation-style-native/generation-style-native-ge-a525d-settings-and-Ollama-payload-desktop/{composer-style-popover,settings-assistant-style,settings-model-advanced}.png`，确认弹窗与设置页排布清晰、「保存设置」已正确禁用且无溢出。

未验证：真实本地 Ollama 模型推理（受本地显存调用确认门槛限制）与真实线上云端模型返回表现待后续人工或授权测试确认。

### 2026-11-26 路线 A · 切片 A1：深度思考流式折叠展示

为用户补充本地与云端模型的「思考过程」流式解析、独立落库与可折叠前端展示，严格将模型输出思考文本与正式回答、执行追踪以及后续多轮历史隔离开：

- **Rust 后端与协议适配（由 Codex CLI GPT 执行）**：
  - 数据库迁移 [0039_message_thinking.sql](apps/desktop/src-tauri/migrations/0039_message_thinking.sql)：为 `messages` 表新增 `thinking TEXT` 列（[db.rs](apps/desktop/src-tauri/src/db.rs) 注册迁移，`execution_checkpoint`、`finish_recorded_run` 与 `chat_messages` 读写同步支持 `thinking`，且后续上下文组装与 `execution_events` 均不混入思考文本）。
  - 流式解析与安全过滤：
    - [ollama.rs](apps/desktop/src-tauri/src/ollama.rs)、[model_runtime.rs](apps/desktop/src-tauri/src/model_runtime.rs)：扩展 `FrameMessage.thinking` 与 `GenerationFrame.thinking`。
    - [chat_stream.rs](apps/desktop/src-tauri/src/cloud/chat_stream.rs)、[completion.rs](apps/desktop/src-tauri/src/cloud/completion.rs)、[runtime_protocol.rs](apps/desktop/src-tauri/src/cloud/runtime_protocol.rs)、[runtime_connections.rs](apps/desktop/src-tauri/src/cloud/runtime_connections.rs)：引入 `StreamChunk::{Content, Thinking}`，支持 OpenAI-compatible/DeepSeek（`delta.reasoning_content` / `delta.reasoning`）、官方 OpenAI Responses（`response.reasoning_summary_text.delta` / `response.reasoning_text.delta`）、Anthropic（`thinking_delta`）与 Gemini（`thought: true`）；正文与思考文本共同受密钥跨分片泄漏拦截保护，非流式 `completion::parse` 同步提取 `thinking`。
    - [generation.rs](apps/desktop/src-tauri/src/generation.rs)：本地与云端流式过程同步推送 `StreamUpdate.thinking`，250 ms 检查点与最终落库原子保存；若主模型失败前已输出思考文本，则保留当前结果而不在同一消息内切换备用模型。同步更新相关调用方与单测文件。
- **前端可折叠展示与原生验收**：
  - 扩展 [chat-stream.ts](apps/desktop/src/chat-stream.ts)（`StreamUpdate.thinking`）与 [studio.ts](apps/desktop/src/studio.ts)（`Message.thinking`）。
  - 升级 [ChatMessage.tsx](apps/desktop/src/components/ChatMessage.tsx) 与 [features.css](apps/desktop/src/styles/features.css)：在正式回答上方增加「正在深度思考… / 已深度思考」折叠面板，标注 `模型输出思考文本` 徽章与字数；正文未开始前自动展开，正式正文输出或完成后支持收起/手动切换，复制、划词与存为知识仅作用于 `message.content`。
  - 新增原生测试 [thinking-stream-native.spec.ts](apps/desktop/tests/thinking-stream-native.spec.ts) 并注册到 [playwright.config.ts](apps/desktop/playwright.config.ts)。

实际验证（未运行任何真实本地 Ollama 推理或在线云端请求，全部基于单测与本机回环假服务）：

- `npm.cmd --prefix d:\vibecoding\CortexOS\apps\desktop run build`：`tsc --noEmit` 与 `vite build` 通过（Vite 10.32s）。
- `npm.cmd --prefix d:\vibecoding\CortexOS\apps\desktop run test:unit`：5/5 通过。
- `cargo test --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked thinking_stream`：6 passed, 0 failed（覆盖 Ollama NDJSON、四种云端 SSE 思考增量与密钥防泄漏、非流式提取、迁移 0039/检查点/持久化隔离与备用模型边界）。
- `cargo test --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked generation_options`：6 passed, 0 failed。
- `cargo test --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked cloud_chat_loopback_preserves_roles_and_excludes_private_lineage`：1 passed, 0 failed。
- `cargo build --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`：通过（57.32s）。
- `$env:CORTEXOS_NATIVE_ONLY="1"; npx.cmd playwright test tests/thinking-stream-native.spec.ts --project=desktop --output=../../artifacts/thinking-stream-native`（目录 `apps/desktop`）：1 passed（5.0s）。
- 已查看原生 WebView2 实拍截图：`artifacts/thinking-stream-native/thinking-stream-native-str-e60a9-content-and-persist-cleanly-desktop/{thinking-collapsed,thinking-expanded}.png`，确认折叠与展开排版、诚实标签及内容隔离正常。

未验证：真实本地 Ollama 模型（如 `qwen3`）与真实线上 DeepSeek / Anthropic / Gemini / OpenAI 推理输出尚未端到端实测。

### 2026-11-26 路线 A · 切片 A2：网页 URL 正文一键入库并挂载对话上下文

打通聊天输入框的网页链接识别与弹窗一键抓取能力，将公开网页清洗后的正文直接存入本地知识库并自动挂载到当前对话上下文：

- **Rust 后端（由 Codex CLI GPT 修改 `web_search.rs`）**：
  - `WebPageContent` 新增 `saved_version: Option<i64>`（`savedVersion`），抽取可复用的 `save_web_page_knowledge` 辅助函数并返回新建 `evidence` 节点的 ID 与版本号。
  - 严格保持生产构建的 SSRF 校验（`result_url`、`PublicPageResolver`、重定向地址过滤），仅在 `#[cfg(debug_assertions)]` 且同时配置 `CORTEXOS_TEST_WEB_PAGE_URL` 与隔离 `CORTEXOS_TEST_DATA_DIR` 时允许抓取指定 `127.0.0.1` 测试替身。
  - 新增 4 项 `web_page_import` 离线单测（内网/本机地址拦截、HTML 标题/正文提取与脚本剥离、`evidence` 节点与版本号落库、测试替身同源匹配）。
- **前端与原生测试**：
  - 扩展 [web-search.ts](apps/desktop/src/api/web-search.ts)（`savedVersion`）。
  - 更新 [ChatComposer.tsx](apps/desktop/src/components/ChatComposer.tsx) 与 [features.css](apps/desktop/src/styles/features.css)：在 `.composer-actions` 新增「导入网页链接」弹窗（支持粘贴 URL 一键抓取存入知识库并去重挂载至 `studio.contextSelection`），并在输入内容包含 `http(s)://` 链接时于工具栏上方展示快捷「抓取正文并挂载」提示条。
  - 更新 [native.ts](apps/desktop/tests/native.ts)（支持 `webPageUrl`）、新增 [web-url-import-native.spec.ts](apps/desktop/tests/web-url-import-native.spec.ts) 并注册到 [playwright.config.ts](apps/desktop/playwright.config.ts)。

实际验证（仅使用本机 `127.0.0.1` HTTP HTML 测试替身，未访问外部公开互联网或真实模型）：

- `npm.cmd --prefix d:\vibecoding\CortexOS\apps\desktop run build`：`tsc --noEmit` 与 `vite build` 通过（Vite 638ms）。
- `npm.cmd --prefix d:\vibecoding\CortexOS\apps\desktop run test:unit`：5/5 通过。
- `npx.cmd --prefix d:\vibecoding\CortexOS\apps\desktop prettier --check d:\vibecoding\CortexOS\apps\desktop\src\api\web-search.ts`：通过。
- `cargo test --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked web_page_import`：4 passed, 0 failed。
- `cargo build --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`：通过（47.77s）。
- `$env:CORTEXOS_NATIVE_ONLY="1"; npx.cmd playwright test tests/web-url-import-native.spec.ts --project=desktop --output=../../artifacts/web-url-import-native`（目录 `apps/desktop`）：1 passed（4.4s）。
- 已查看原生 WebView2 实拍截图：`artifacts/web-url-import-native/web-url-import-native-one--2cfed-d-mounts-it-to-chat-context-desktop/{web-import-popover,web-import-mounted}.png`，确认弹窗布局、成功 Toast 与「手动选择: 1 项知识」状态更新清晰无溢出。

未验证：真实公网复杂网页兼容性与本地/云端模型基于抓取网页的实测回答。

### 2026-11-26 路线 A · 切片 A3：跨会话聊天全文检索与定位跳转

在对话侧栏新增基于 SQLite FTS5 的跨会话全文搜索与点击跳转定位，严格只索引 `purpose = 'chat'` 会话中的正文（`messages.content`），绝不索引思考过程、执行追踪或评测数据：

- **Rust 后端（由 Codex CLI GPT 执行 `src-tauri/` 改造，并补齐 Tauri 2 命令权限）**：
  - 数据库迁移 [0040_chat_messages_fts.sql](apps/desktop/src-tauri/migrations/0040_chat_messages_fts.sql)：建立 `chat_messages_fts` 虚表；[db.rs](apps/desktop/src-tauri/src/db.rs) 复用 `retrieval::terms`（CJK 双字 + 英文分词）实现用户发送、回答落库同步、删除会话清理及历史会话惰性回填（`search_chat_messages` 查询前自动补建缺失索引），支持 BM25 排序与单字/符号子串回退匹配，生成居中摘要片段。
  - [models.rs](apps/desktop/src-tauri/src/models.rs)、[commands.rs](apps/desktop/src-tauri/src/commands.rs)、[lib.rs](apps/desktop/src-tauri/src/lib.rs)、[build.rs](apps/desktop/src-tauri/build.rs)、[capabilities/default.json](apps/desktop/src-tauri/capabilities/default.json)、[portability/scope.rs](apps/desktop/src-tauri/src/portability/scope.rs)：新增 `ChatSearchHit` 与 `search_chat_messages` 命令并注册权限与备份校验；新增测试模块 [chat_search_tests.rs](apps/desktop/src-tauri/src/db/chat_search_tests.rs)。
- **前端检索栏与跳转定位**：
  - 扩展 [chat.ts](apps/desktop/src/api/chat.ts)（`ChatSearchHit`、`search_chat_messages`）。
  - 更新 [ConversationList.tsx](apps/desktop/src/components/ConversationList.tsx) 与 [features.css](apps/desktop/src/styles/features.css)：在侧栏新增带防抖（180ms）与一键清空的「搜索历史对话」输入框，展示所属会话标题、发言角色（你/助手）、归档标记与命中摘要；点击后打开目标会话并经 `navigate("chat", { workspace, conversation, message })` 滚动定位高亮原消息。
  - 新增原生测试 [chat-search-native.spec.ts](apps/desktop/tests/chat-search-native.spec.ts) 并注册到 [playwright.config.ts](apps/desktop/playwright.config.ts)。

实际验证（仅使用单测与本机 `127.0.0.1` 假 Ollama 服务，未调用真实模型）：

- `npm.cmd --prefix d:\vibecoding\CortexOS\apps\desktop run build`：`tsc --noEmit` 与 `vite build` 通过（Vite 600ms）。
- `npm.cmd --prefix d:\vibecoding\CortexOS\apps\desktop run test:unit`：5/5 通过。
- `cargo test --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked chat_search`：2 passed, 0 failed（覆盖中英文/归档会话命中、排除 `thinking`、工作区隔离、旧消息回填与删除清理）。
- `cargo test --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked thinking_stream`：6 passed, 0 failed。
- `cargo build --manifest-path d:\vibecoding\CortexOS\apps\desktop\src-tauri\Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`：通过（37.46s）。
- `$env:CORTEXOS_NATIVE_ONLY="1"; npx.cmd playwright test tests/chat-search-native.spec.ts --project=desktop --output=../../artifacts/chat-search-native`（目录 `apps/desktop`）：1 passed（4.5s）。
- 已查看原生 WebView2 实拍截图：`artifacts/chat-search-native/chat-search-native-cross-c-1b47e-nd-jumps-to-located-message-desktop/{chat-search-results,chat-search-located}.png`，确认侧栏命中结果列表、角色标记及点击后跨会话跳转并高亮目标消息（`data-located="true"`）表现正常。

未验证：大规模多轮长历史下的检索延迟压测与真实模型生成内容搜索尚未开展。
