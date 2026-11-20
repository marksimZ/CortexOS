# H1.3 接口、导航与代码结构验收

## 实际改动

- [对话](../../apps/desktop/src/api/chat.ts)、[知识与图谱](../../apps/desktop/src/api/knowledge.ts)、[评测报告](../../apps/desktop/src/api/benchmark.ts) 共整理 51 条 IPC 命令。调用方通过命令名推导参数和返回类型。用注册清单逐条核对命令名，抽查 Rust 类型、序列化字段和空值。编译检查证明缺少工作区、错写命令名或使用错误评测范围会被拒绝。
- [桥接层](../../apps/desktop/src/api/request.ts) 原样传递后端错误，界面沿用已有的脱敏解析。只读视图取消订阅后会丢弃旧回复，Rust 的任务取消命令及超时仍负责实际执行。知识图分页与评测历史复用此行为。
- [导航](../../apps/desktop/src/navigation.ts) 共用 URL 参数解析。工作区选择从应用路由发起，跨工作区会话加载保留目标归属。报告定位保留批次、筛选、页码和题目，迟到结果检查当前地址后才更新。浏览器前进、后退及执行图返回都使用原链接格式。
- [知识编辑](../../apps/desktop/src/KnowledgeEditor.tsx) 和[关系编辑](../../apps/desktop/src/RelationshipEditor.tsx) 拆成独立按需加载组件。编辑、审核、冲突恢复与草稿继续从原入口操作。[个性化评测历史读取](../../apps/desktop/src/benchmark/usePersonalizedHistory.ts) 单独管理设置与历史，重复刷新时舍弃已经过期的读取结果。
- [模型设置草稿](../../apps/desktop/src/studio-settings.ts) 重新读取时保留已编辑字段，未编辑字段接收最新保存值。保存期间新改动继续留在草稿，晚到的启动读取不会覆盖更新的保存结果。

## 执行结果

| 检查 | 实际结果 |
| --- | --- |
| `npm.cmd run build`、离线锁定 Rust custom-protocol 调试构建 | 通过，包含 TypeScript。分拆后的知识编辑包约 8.21 kB，关系编辑包约 7.01 kB。原有按需图谱包仍超过 500 kB。 |
| ESLint、Prettier、前端单元 | ESLint 0 错误、64 条提示。Prettier 既定范围通过。4 个 Node 单元通过，覆盖设置草稿合并和导航参数。另有随 TypeScript 检查的接口反例。 |
| 浏览器 Edge | 4 项通过。覆盖报告页码、筛选、地址往返、个性化刷新、迟到结果、详情焦点和渲染错误恢复。 |
| Windows 原生 WebView2 | 两组 6 项通过。覆盖分页图谱及关系编辑、知识审核冲突和重启、执行图定位、文件来源、设置表单和标准评测结果按需加载。 |
| 原生性能与截图 | `--project=h1-baseline` 2 项通过。五页、两主题、四尺寸 40 张截图，页面无横向溢出。 |

Playwright 在独立 Windows 桌面运行。原生测试用隔离 SQLite 库及固定回环 HTTP。没有真实模型或云端调用。原始产物在本地 `artifacts/phase-h1/h13/`。

| 同条件样本 | H1.2 | H1.3 |
| --- | ---: | ---: |
| 60 段回复的 React 根树提交 / Markdown 流式入口 | 20 / 18 | 19 / 18 |
| 首屏入口及预加载 JS | 526,927 B | 528,209 B |
| 打开 1,000 条会话，最新消息挂载中位耗时 | 54.36 ms | 54.21 ms |

同条件指同机、同套固定数据与四种视口配置。100 / 500 条样本中位由 45.15 / 47.99 ms 变为 61.46 / 55.40 ms，各三次，包含浏览器轮询和进程运行差异，保留原始数据用于继续对照。

## 工程边界

契约由前端集中维护。Rust 现有命令混用 Serde 结构和按业务构造的 `serde_json::Value`，自动生成仍需先将开放的 JSON 输出改成明确的 DTO，再验证版本及可选字段语义。本轮保留原 Rust 命令、数据库与权限，尚未迁移的设置和 Harness 命令可随各自功能修改逐步接入。

工程检查使用项目内开发依赖。ESLint 9 是所选 Babel 和无障碍插件声明共同兼容的版本，npm 已标记弃用，后续需随着两者的兼容范围更新。Hooks 依赖与旧界面部分标签规则保留为提示。格式检查默认覆盖新接口、草稿逻辑和测试，既有文件逐步采用。 [React Hooks 规则](https://react.dev/reference/eslint-plugin-react-hooks) · [Prettier CLI](https://prettier.io/docs/cli)
