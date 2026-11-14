# D3-3b：Claude 沙盒条件验证

2026-11-03。用户在D3-3a验收点要求继续。本片只解决程序参数与隔离门槛，完成后停在验收；模型消息通道、真实编程任务和Aider保持后续切片。

交付更新：依赖镜像和最多三组对照已分别取得明确许可，按本方案执行；同一候选补读能力字段及子命名空间拒绝证据后，13项通过，已接入独立Claude策略与v2检查。原生/新旧记录/恢复验证完成，见[实际交付](harness-fit-d3-claude-sandbox.md)。下文“尚未应用”是提交权限确认时的快照。

## 已有证据与最小修改

- 固定Claude Code 2.1.309文件及专用镜像沿用上片大小/hash/签名证据，不下载其他Claude版本。
- 默认策略下实际参数解析对照：控制组和`--max-turns 1`组均在固定无效选项处退出1，没有会话或模型请求；`--bare --print --help`退出0。程序二进制也含预算参数定义，官方CLI参考列出该参数。新检查可记录“参数被解析器接受”，不把它写成实际回合预算已验证。
- 原环境实际为Debian13/trixie；bwrap/socat缺失、unshare(NEWUSER)返回EPERM。依赖存在、命名空间可用、Claude实际Bash沙盒、模型通道分别记录。
- 修改固定探针和分类时追加新probe/plan版本，保留旧版本定义及读取校验，历史记录不重算、不覆盖。复用原用途、任务绑定、清理/恢复和控件，不迁移数据库或重排页面。

## 工具准备：需明确确认后执行

具体构建文件为`scripts/harness/claude-sandbox.Dockerfile`。只从已固定的Claude镜像派生，使用镜像已有Debian archive keyring验证官方APT索引签名，启用HTTPS Debian源；固定bubblewrap `0.12.0-1~deb13u1`、socat `1.8.0.3-1+deb13u1`，关闭推荐包。libwrap0等最小依赖使用经签名索引的实际版本，记录所有安装版本、清单与镜像ID。若固定版本不可用，停止复核，不静默升级。

需要下载官方仓库索引及少量运行包；只安装在项目Claude专用Docker镜像，不写Windows/WSL系统、原项目或个人登录。镜像构建时联网取包，实际检查容器仍network=none、非root、只读根目录、无宿主挂载/Socket、cap-drop=ALL、no-new-privileges及原CPU/内存/PID/时间/输出限制。没有模型调用。

本次工具准备确认不包含修改seccomp、增加capabilities、弱化嵌套沙盒或关闭保护。上片原镜像和Codex/Python镜像保留，不覆盖旧记录引用。

## 权限试验：先提交具体差异再确认

依赖镜像准备后先用默认策略检查，确定真正阻断位置。若须增加内核接口权限，先形成独立Claude策略、固定来源/hash、精确差异及最多三组可信探针对照计划，向用户说明暴露面后确认。Codex策略的旧许可不覆盖Claude；不使用privileged、unconfined、setns、宿主挂载、系统sysctl或弱化配置来取得通过。

### 2026-11-03：默认检查后的具体候选（尚未应用）

依赖准备获用户批准并完成：APT签名索引约10.1MB，3个新包共536kB，无包升级；bubblewrap/socat和libwrap0 7.6.q-36已在专用镜像中。原默认策略下bwrap实际仍因无法创建命名空间退出1；容器已清理。不是仅凭依赖存在猜测可用。

`artifacts/harness-claude-sandbox/review.json`已生成独立Claude评审文件，只写文件、不应用权限。复用已核验的官方Moby源快照（提交4b8214fc4971e06172938a1f1554ccff6647ee80，基线hash6416b477...），并非安装引擎过滤器的导出。准备最多三组：

1. 显式官方基线：不追加规则。
2. userns诊断组：仅追加unshare，允许标志范围USER/MOUNT/UTS/IPC/PID/NET及0，其他标志仍拒绝；hash e0d7a76d...。
3. bubblewrap候选组：再增加amd64 clone的NEWUSER必需位/禁止NEWCGROUP位，允许mount、umount2、pivot_root、chroot；clone3等原限制保持，不增加setns；hash46a3ec98...。

本候选规则字节与上次Codex兼容候选相同，但Claude需新的明确许可、独立观察和用途记录，Codex的旧许可及测试结果不作为Claude通过依据。新增接口增加内核暴露面，seccomp不能按路径检查mount或证明它一定发生在子user namespace；继续依赖其余容器和内核权限边界。不增加容器初始capabilities、不关闭任何过滤器、不修改Windows/WSL/sysctl。

获得确认后，仅在本次固定依赖镜像用最多三组可信无模型探针对照，记录准确归属并完成即清理。目录、网络、系统调用反向证据和实际CLI条件齐备后才考虑独立Claude接入；无法验证真正CLI沙盒时保留未验证。需要超出上述调用范围、弱化沙盒或其他系统权限时停止，不自行扩大试验。

## 严格隔离与验收

1. 参数元数据检查不创建模型会话；固定无效选项确保解析失败退出，记录原始stdout/stderr/退出码。其结果仅证明参数解析。
2. 实际Claude沙盒需有来自该固定CLI的执行证据，证明允许目录可写、同一外层容器中可写但不允许的目录被拒绝、只读系统/题目被保护、无网络及越权系统调用受限。单独跑bwrap或另装srt不冒充Claude内层沙盒通过。
3. 采用enabled=true、failIfUnavailable=true、allowUnsandboxedCommands=false；弱化嵌套/网络隔离保持false，excludedCommands为空。验证失败时不运行未隔离替代命令。
4. 无模型方式若无法触发实际CLI沙盒，则明确保留“内层沙盒未验证”；通过受控替身驱动工具的方案须单独审查，不调用本地或云端模型。
5. 先代码逻辑检查，必要时只跑新信任边界单项；类型检查、构建和真实WebView2/独立QA检查覆盖新旧记录、停止/恢复/清理。截图使用三尺寸浅深主题，未做的部分如实记录。

## 本轮查阅依据

- [Claude CLI参数](https://code.claude.com/docs/en/cli-reference)：max-turns为print模式预算，实际运行预算仍需后续受控验证。
- [Claude沙盒配置](https://code.claude.com/docs/en/sandboxing)：默认缺依赖可能不使用沙盒，需严格失败/禁止未隔离重试；手动`!`命令不能作为模型Bash沙盒证据。官方的弱化嵌套建议不等于本项目授权。
- [Debian bubblewrap](https://packages.debian.org/trixie/bubblewrap)、[socat](https://packages.debian.org/trixie/socat)：官方包及所需运行库。

## 当前剩余

D3-3b已交付，等待本片验收。剩余D3-3c受控模型与单题、D3-3d Aider、D4组合对照、D5报告与图谱。真实模型预算、完整项目和跨文件尚未验证；任何模型批次依AGENTS另行弹窗。
