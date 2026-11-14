# D3-2b：Codex 内层沙盒兼容

2026-10-31。用户验收D3-2a并要求继续，另行弹窗明确批准最多三组无模型策略对照，反向检查通过后仅接入Codex专用容器。[审查方案](harness-fit-d3-sandbox-plan.md)及原始候选/差异在artifacts/harness-codex-namespace；本片没有模型、Agent会话、项目代码执行或系统安全设置修改。

## 实际原因和修复

本机Docker采用内置seccomp，初始CapEff/CapBnd均0、NoNewPrivs=1。默认策略下unshare(0)和unshare(NEWUSER)均EPERM，而用户命名空间数量上限为31182。固定官方基线对照复现阻断；仅增加受限unshare后两者成功，但Codex仍被阻断；增加已批准的clone及隔离文件系统调用后，Codex通过原失败位置。对照确认这些系统调用规则是本机的实际兼容条件，没有更改内核参数或开启管理员能力。

同时修正D3-2a探针的两处错误，原记录不改：

- 固定CLI 0.166.0的实际`codex help sandbox`要求`codex sandbox [OPTIONS] [COMMAND]`；原探针的`linux`被当作待执行程序，先前被更早的权限错误遮住。现按实际帮助去除该参数，不根据网页示例猜测版本接口。
- 正常拒绝越界写入可能返回EROFS(30)，不只PermissionError；新探针仅接受EPERM/EACCES/EROFS等真实拒绝，其他错误不能充当保护通过。

新探针protocol=2、probeVersion=codex-sandbox-2，必须全部取得：允许目录可写、目录外拒绝、系统目录只读、网络拒绝、额外挂载拒绝、BPF拒绝、CapEff/CapBnd均0、no-new-privileges、seccomp启用、无Docker Socket，以及UID65534。网络对照先证明父容器中的监听端口可达，再由内层命令连接；内层连接拒绝。不能以本来不存在的外部服务充当网络保护证明。

本机固定候选已实际取得这十项true及UID65534，目录外/系统写入errno30、mount/BPF errno1。它是固定受控探针的行为证明，不是完整漏洞评估或任意程序都无法逃逸的保证。

## 固定策略与适用范围

策略ID `codex-bwrap-1`；SHA-256 `46a3ec985105a85a8d6bb5d21974a5dd5093f176411d766f8acf51bf5f2d9f13`。保留固定Moby官方快照的全部规则和默认ERRNO，追加受限unshare、要求NEWUSER且排除NEWCGROUP的amd64 clone、mount/umount2/pivot_root/chroot。没有增加setns、放开clone3、capability或关闭seccomp。

来源提交`4b8214fc4971e06172938a1f1554ccff6647ee80`，原文件hash及来源见catalogs/codex-seccomp/README.md；Apache-2.0许可随文件保留。该官方快照不是本机内置过滤器的导出，因此不声称相对安装引擎仅有六个调用的差异。允许更多内核接口会增加攻击面；路径字符串不能由此seccomp规则检查，仍依赖内核权限、外层挂载/网络/资源隔离及内层沙盒。

只有Codex环境检查可以选该策略。Docker默认仍是默认选项，普通Python评分和人工任务副本的create_args保持原样。前端只能发送两个固定策略ID，不能提供profile、路径或命令；未知ID/未确认输入拒绝。创建后读取实际SecurityOpt并与嵌入策略逐字段比较，错误/禁用/篡改过滤器不能通过；实际配置保留在观察中。

CLI只接受本次私有临时策略文件；名字由已持久化任务UUID确定，create_new写入、创建容器后移除。清理和重开根据原任务ID核对文件类型、重定向属性及内容hash，仅删除本任务文件；失败保持待清理，不扫描或删除用户其他文件。

外层继续network none、readonly root、UID65534、CPU1、256MiB、PID64、cap-drop ALL、no-new-privileges、无宿主目录和Socket，时间/输出限制沿用。内层workspace-write及network_access=false，不使用全权限或关闭审批的回退。

## 使用、历史与数据

原位置：评测 → 项目适配 → 测试任务 → Codex执行前检查 → Codex隔离检查。选择“Codex兼容策略1”，确认窗说明范围与额外接口风险；通过后显示“目录与隔离检查已通过，编程任务尚未运行”。这不等于整个Agent执行条件已就绪，canRun仍false。

schema仍11、用途仍harness_codex_runtime；新记录version=codex-runtime-check-2，固定策略ID/hash、来源及新probeVersion。v1原记录继续只读、不补写策略字段、不重跑或重算。新环境检查、停止/恢复及清理独立追加记录，不写聊天、知识、执行图或普通评测成绩。历史记录的实际策略与“下一次检查”的选择分开显示。

## 实际文件与验收

- 新`scripts/harness/namespace_probe.py`、`prepare_codex_seccomp.py`、`check_codex_seccomp.py`：固定诊断、生成审查文件和三组有界对照，不安装或调用模型。
- 新`src-tauri/catalogs/codex-seccomp/{profile-v1.json,README.md,LICENSE}`及`benchmark/python/codex_policy.rs`；修改codex.rs/codex_probe.py、python.rs、harness/runtime.rs与records.rs。无新依赖、迁移或Tauri能力。
- 修改CodexRuntime.tsx，复用原控件/确认/状态和历史，不新建页面或CSS。扩充已有原生专项的兼容模式及v1历史检查。
- 同步计划、规范、索引及实际里程碑。命令与原生结果以[里程碑](milestone-log.md)为准；脚本的三组实验与应用原生检查分开记录，修正探针后的复验不算新增策略组。

## 下片

实际验收：`cargo test codex_ --lib --offline -- --nocapture`两项通过；TypeScript、Rust检查及桌面调试构建通过。兼容模式原生专项1项通过，28.4秒，覆盖十项真实保护、原v1记录完全不变、错误策略拒绝、取消、重开、强杀恢复与手动清理；六张1366/1600/1920浅深截图无横向溢出，已实际查看浅1366及深1920。独立库schema11、外键检查为空、完整性ok，messages/knowledge_nodes/execution_runs/benchmark_runs均0；三个新记录拥有的策略临时文件均不存在。

证据：`artifacts/harness-codex-policy-native/harness-codex-runtime-Linu-f820c-oped-cleanup-without-models-harness-codex-runtime/runtime.json`、database-verification.json及六张runtime截图。只测试固定可信探针，不构成真实Agent/项目任务验收，也未重做真实Windows缩放/读屏。源码新增策略选择及清理经过必要检查，没有运行无关全套单元。

本片只交付安全环境兼容，不启用真实Agent任务。D3-2整体尚未完成。下一片设计/实现容器到Ollama的受控通道、真实CLI JSONL、请求/时间/输出预算及单个已审核任务；在实际加载或调用本地模型前，先弹窗说明实际模型与有界批次并等待确认。没有安全通道、确认或模型兼容证据时继续阻断。
