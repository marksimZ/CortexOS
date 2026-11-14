# D3-0：Codex检测与执行前检查

2026-10-30。承接D2b-1，按[D3分片计划](harness-fit-d3-plan.md)交付第一个准备切片。宿主安装检测、Docker可读取和真实Harness任务执行分别表达；当前不能启动Harness任务。

## 使用

评测 → 项目适配 → 4.测试任务 → Codex执行前检查。

1. 先完成Python编程草案的三组实际对照和人工发布。本页仅列出属于当前工作区、当前分析版本的已发布编程任务，最多100个版本；更新列表不会执行探针。
2. 选择任务版本，点击“检查执行条件”。后端重新核对任务hash、审核记录、项目快照和分析依据；不匹配时在本机程序探针之前拒绝。
3. 读取PATH中首个符合范围的原生codex.exe，仅执行固定参数--version；读取Docker本地Linux引擎与发布验证绑定的Python镜像，不启动容器、不运行模型或导入代码。
4. 分别列出任务依据、宿主CLI、Docker镜像，以及尚缺的Agent隔离环境/工作副本、模型通道、评分交接、预算/批准。检查完成不代表任务可运行，canRun=false、taskExecution=Not Run。
5. 保存独立检查快照，保留最近30次记录。切换任务先隐藏上一任务结果，选历史同步具体任务版本；重新打开只读取记录，不自动重新探测。环境改变后需显式新检查，旧记录不变。

## 检测范围

当前版本探针只覆盖Windows PATH中的原生程序；最多检查128个绝对本机PATH项的准确文件名，不递归扫描，不使用相对路径、网络路径、链接/重解析路径或Git工作树内程序。找到codex.cmd/ps1但没有可用原生程序时保留“版本未知”，不执行启动脚本。未找到只表示本次PATH检查没找到，不证明整台机器未安装。

探针使用独立临时目录，清除继承环境，只保留SystemRoot/WINDIR并将配置/用户/临时目录定位到本次空目录；不执行login、读取账号或枚举云端模型。5秒限制，stdout/stderr各最多4096字节；仅接受严格codex-cli版本行，保存程序路径/前后内容指纹、固定参数及清理结果，不保存任意原始输出。原生二进制最大512MiB，指纹读取同时检查实际累计字节上限。PATH中的程序属于宿主受信安装检测，这不构成不可信Agent代码的安全执行环境；CLI存在也不证明Linux镜像、认证或模型兼容。

Docker观察复用原python::harness_environment，只接受本地npipe/unix端点、Linux引擎及固定Image ID，错误保留具体受限原因；不拉镜像、不创建执行容器，也不把本次元数据读取当成重新通过全部隔离安全测试。

## 文件与数据

- 后端：`apps/desktop/src-tauri/src/benchmark/harness/preflight.rs`、`preflight/codex.rs`；接入harness/mod.rs。原programming_taskset读取仅调整模块可见性供复用，不改发布数据。
- 权限：src/lib.rs、build.rs、capabilities/default.json仅新增get_harness_execution_preflight与check_harness_execution_preflight两个业务命令。前端不能提交任意程序、命令参数、路径、镜像或环境变量。
- 前端：新`src/benchmark/harness/ExecutionPreflight.tsx`，TaskWorkbench嵌入、harness.css局部列表样式。沿用现有Tokens、SelectMenu/FitStatus，不重排导航或引入卡片体系。
- 记录：复用benchmark_generation_jobs，purpose=harness_execution_preflight，保存完成的检查计划/hash；查询限定工作区、分析版本和用途。旧计划不可变，检查不混入普通个性化生成/任务历史，不写BenchmarkRun、聊天、知识、图谱或成绩。
- schema仍11，迁移、依赖和锁文件不变。检查前后不改变已发布任务及历史验证。

## 验证与后续

实际命令、成功/失败、真实版本、原生截图与验收窗口写入[里程碑](milestone-log.md)。合成环境观察仅用于数据隔离检查，不冒充真实探针结果。未安装/脚本形式、其他操作系统和实际系统缩放/读屏未由本机正常路径验收替代。

本轮停在D3-0验收；下一片D3-1先建立固定快照的可写副本与补丁/独立评分交接，之后才接真实CLI。任何CLI安装/新执行镜像下载及模型认证通道均须按原规则确认；不借用个人Codex登录、CortexOS密钥或宽权限模式绕过门槛。其他D2任务类型、Claude/Aider、D4/D5分别后置。

官方接口依据：[非交互模式](https://learn.chatgpt.com/docs/non-interactive-mode)、[CLI命令](https://learn.chatgpt.com/docs/developer-commands?surface=cli)，本轮已实际查阅；文档中的JSONL与认证说明仅作为后续设计依据，不计为本机调用验收。
