# D3-2b：内层沙盒兼容试验

2026-10-31。用户验收D3-2a并要求继续。本片先解决安全环境门槛，不调用模型，不启动真实编程任务。

## 已取得的证据

固定镜像中UID65534、CapEff/CapBnd均0、NoNewPrivs=1、Seccomp=2、过滤器数量2；user.max_user_namespaces=31182，Docker SecurityOptions为内置seccomp和cgroupns。固定unshare(0)及unshare(NEWUSER)均EPERM。结合[Docker默认规则](https://docs.docker.com/engine/security/seccomp/)推断系统调用过滤很可能参与阻断；仅凭这些值尚不能排除其他限制，须做有界A/B试验。

Docker官方引用的Moby规则已固定到提交`4b8214fc4971e06172938a1f1554ccff6647ee80`，原文件SHA-256为`6416b47770785a41ac59073cdc77d9fe98517df2799dc83ef207e622de3053f6`。这是一份可审查的官方规则快照，不声称已导出或精确匹配安装引擎的内置过滤器。来源、完整原文件、派生规则和差异保存在artifacts/harness-codex-namespace。

## 需确认的具体变更

`scripts/harness/prepare_codex_seccomp.py`只生成文件，不应用配置。保留原defaultAction=ERRNO、默认错误码和全部原规则；仅准备以下追加规则：

1. userns诊断版：允许unshare，参数只允许USER/MOUNT/UTS/IPC/PID/NET及0，其他标志仍拒绝。
2. bubblewrap候选版：在上述规则上增加amd64 clone的NEWUSER必需位、禁止NEWCGROUP位；允许mount、umount2、pivot_root、chroot供新建的内部隔离环境使用。保留clone3的原限制，不增加setns权限。

候选不增加容器初始Linux capabilities、不关闭seccomp、不改LSM或宿主sysctl。网络none、只读根目录、无宿主目录/Socket、非root、内存/CPU/PID/时间/输出限制均保持。额外允许的调用会增加内核接口暴露面；seccomp不能检查路径字符串或证明某次mount必定发生在子user namespace中，仍依赖内核权限、挂载隔离与其他边界，不能宣传为与默认策略风险完全相同。

## 试验范围与退出条件

获得用户确认后，仅用现有固定镜像运行最多三组无模型对照：显式官方基线、userns诊断版、bubblewrap候选版。每组仅固定可信探针，容器记录准确归属，完成即清理；没有仓库代码、认证或模型。若候选仍需新增capability、setns、关闭保护或超出已批准的系统调用范围，则停止并报告具体原因。

候选若能启动内层sandbox，还需反向检查：允许的函数目录可写、同容器可写的目录外被拒绝、系统/题目只读、初始capabilities为0、额外mount/网络/敏感syscall与越界文件仍受约束、输出及超时有效、停止/恢复/清理保持。通过后才将固定策略/版本/hash接入Codex专用环境检查；原Python评分和任务副本继续使用默认策略，旧记录不重算。界面与数据库只做必要字段调整，不重排UI，不新增模型调用入口。

如果安全条件无法同时满足，则保留阻断，不使用全权限或旧沙盒回退完成任务。模型通道、JSONL和单题执行仍在本片之后；实际调用本地模型时另行弹窗确认。
