# Phase G0 依赖与构建审计

2026-11-18，依据当前 package.json、package-lock.json、Cargo.toml、Cargo.lock 和实际安装目录。没有安装、升级、卸载或去重依赖。原始清单：[dependencies.json](../../artifacts/phase-g0-2026-11-18/dependencies.json)、[npm-tree.json](../../artifacts/phase-g0-2026-11-18/npm-tree.json)、[cargo-metadata.json](../../artifacts/phase-g0-2026-11-18/cargo-metadata.json)、[cargo-duplicates.txt](../../artifacts/phase-g0-2026-11-18/cargo-duplicates.txt)。

## 前端直接依赖

有 10 项运行依赖和 9 项开发依赖。每项运行依赖均能在 src 中找到真实导入，未发现可以直接删除的未使用运行依赖。

| 能力 | 当前依赖 | 依据 |
| --- | --- | --- |
| React | react / react-dom 19.3.1 | App、main 及弹窗 Portal |
| 桌面 IPC | @tauri-apps/api 2.12.4 | studio、领域页面及 graph-query |
| 3D | three 0.186.1、@react-three/fiber 9.8.2、@react-three/drei 10.7.9 | GraphViewport、GraphInstances、GraphCamera 等；Drei 直接使用 OrbitControls |
| 图标 | lucide-react 1.57.0 | 现有工具栏、按钮和状态图标 |
| Markdown | react-markdown 10.1.0、remark-gfm 4.0.1、micromark-util-decode-string 2.0.1 | MarkdownContent，保留解析和引用定位语义 |

开发依赖包括 TypeScript 7.0.2、Vite 8.3.4、@vitejs/plugin-react 6.1.2、Playwright 1.64.1、Tauri CLI 2.12.3 和相应类型包。它们的安装占用不等于正式安装包占用。source 导入的 three-stdlib 和 mdast 还包括类型用途，应在将来修改依赖时核对声明归属，不为去重随意改变运行代码。

## 安装尺寸与传递依赖

node_modules 共 331409790 字节。以下按顶层包目录归集，包含包内嵌套依赖；因此 stats-gl 一项包含自己的 Three.js 副本。没有把每个父包和子包重复相加。

| 安装目录 | 实测字节 | 判断 |
| --- | ---: | --- |
| lucide-react | 36087673 | 含 ESM/CJS 等多种文件；不能当作 UI 实际加载 36MB |
| hls.js | 32827430 | 3D 辅助依赖链带入；当前 UI 未直接导入它 |
| @typescript/typescript-win32-x64 | 28365027 | 构建工具 |
| stats-gl | 27550016 | 含嵌套 Three.js，应用未直接使用统计面板 |
| three-stdlib | 26925971 | 类型及 Drei 依赖链 |
| .vite | 25167750 | 开发可重建缓存，不是运行依赖 |
| @rolldown/binding-win32-x64-msvc | 21664198 | 打包工具 |
| three | 20443256 | 根版本 0.186.1 |
| @mediapipe/tasks-vision | 20412943 | 辅助组件传递依赖；当前 UI 未直接导入它 |
| @tauri-apps/cli-win32-x64-msvc | 16086315 | 原生打包工具 |
| playwright-core | 13656167 | 验证工具 |

实际依赖树中 Three.js 有根 0.186.1 与 stats-gl 下 0.170.0 两个版本。约 27MB 的嵌套副本是开发安装去重的候选，不表示已经进入前端图谱块。不要直接用 overrides 强制跨版本：先核对上游范围和实际使用路径，再在隔离安装中验证。

当前 Drei 仅直接用于 OrbitControls。未来可比较直接使用已有 three-stdlib 的 controls 或较窄入口；预计收益主要是减少安装依赖面，运行包收益尚未测量。需保留相机取消、缩放、焦点切换、状态恢复和 reduced-motion 语义。没有必要仅为目录尺寸更换现有 3D 框架。

现有主 JS 899089 字节、图谱块 941050 字节，首页没有请求图谱块。主块中哪些库占比最高尚未取得模块级构建清单。G1 先生成对应构建分析，再决定图标或 Markdown 是否值得调整。正文解析和安全策略具有产品用途，不为了体积删除既有能力。

## Rust 与原生构建

[Cargo.toml](../../apps/desktop/src-tauri/Cargo.toml) 使用 Tauri、rusqlite bundled、reqwest native-tls、serde、jsonschema、Tokio、uuid、chrono、sha2、对话框插件、zeroize、zip 及 Windows API。jsonschema、zip、reqwest、chrono 已有明确 feature 约束。bundled SQLite 用于稳定的 FTS 和迁移行为，zeroize 及 Windows Credentials 用于密钥保护，均有现有职责。

cargo tree -d 中既有真正多版本，也有同版本按 host/build/target 不同上下文列出的分支。例如 bitflags 1.3.2 与 2.13.2 分属图像构建及运行依赖；重复打印同一版本不等于程序重复加载两个实例。不能用输出行数推断可删除的运行开销。

Release 已配置 codegen-units=1、lto=true、opt-level=3、panic=abort、strip=true。本轮不将这些既有设置列为新优化。后续比较 opt-level=s/z 必须同时测体积、启动、RAG/评分吞吐及编译耗时。strip 对 MSVC PDB 的实际处理也需以文件验证，不以配置名称推断。

crate-type 同时包含 staticlib、cdylib、rlib。target 实测 36592838606 字节，主要来自调试对象、增量与库文件；单个 staticlib 可达约 1.75GB。是否可减少 Windows 桌面构建输出，先核对 Tauri 打包和其他目标需要，并用新的独立 target 目录对照。当前没有改 crate-type，也没有运行 cargo clean。

## 可维护性与职责边界

| 文件 | 实测行数 | 后续处理原则 |
| --- | ---: | --- |
| src-tauri/src/db.rs | 1934 | 已有 db 子模块；随具体查询切片提取，保留数据库单入口与事务 |
| src/Graph.tsx | 1489 | 编辑器、画布和筛选同文件；路由拆块时先分离共享导出 |
| src/Knowledge.tsx | 1088 | 避免 App 静态导入编辑器再次引入整页 |
| src-tauri/src/knowledge_scope.rs | 1048 | 快照/分支合同独立，行数本身不是重写依据 |
| src-tauri/src/rag.rs | 911 | 增量索引和缩小锁区可成为自然拆分边界 |

行数包含类型与测试。没有按文件长度给出“必须拆分”的结论，优先消除已测得的请求放大与资源重复。

## 实施选择

1. G1 首选路由分块和按需读取，复用现有库，预期风险和改动面较小。
2. 依赖裁减另做独立切片：隔离 npm ci、依赖图与 bundle 分析、类型检查、打包及原生图谱交互对照。保留 lockfile，失败恢复至原锁文件。
3. 构建配置只在相同代码、相同工具链、独立输出目录比较。开发缓存回收交由 G2 的预览及确认，不与业务性能改动混在一起。
4. 本轮未执行在线漏洞扫描、npm audit、cargo audit 或安全版本调查，不声称依赖安全审计通过。本报告关注实际占用、导入和复用边界。
