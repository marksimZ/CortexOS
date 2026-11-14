# D2b-0：WSL2 / Docker安装与恢复记录

2026-10-29。用户明确批准安装官方WSL2、当前用户版Docker Desktop，并下载Python3.13镜像；镜像数据位置为`D:\CortexOSRuntime\DockerData`。本记录区分“文件已下载”“程序已安装”“服务可用”“隔离验收通过”，不能相互代替。

## 已执行

- 安装前确认两个Windows功能VirtualMachinePlatform、Microsoft-Windows-Subsystem-Linux均Disabled；基线含启动时间，保存在`D:\CortexOSRuntime\Downloads\wsl-baseline.json`。
- 本机内置`wsl --install --no-distribution --web-download`退出1，提示WSL未安装，没有假称安装成功。改用Docker官方文档同样列出的微软稳定版MSI安装途径。
- 通过微软官方发布API绑定WSL **3.0.3**、`wsl.3.0.3.0.x64.msi`，367575040字节，官方SHA256 `ae977286974255bd26bf925f00f40f5692c6b2084155cbc0563c09aea091f211`。
- 通过Docker官方更新清单及版本校验文件绑定Desktop **4.97.0 / 244584**，620496816字节，官方SHA256 `2668273f82b342a058a28f9ab53ea7bddde10bb6d51307ce2ec54c2d36459744`。
- `Enable-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform -All -NoRestart`已完成，返回`RestartNeeded=true`。没有执行重启，没有为WSL1额外启用旧Subsystem组件。功能显示Enabled不表示当前启动的内核已支持运行WSL2。
- 官方下载遇到慢连接及TLS连接中断；只停止了本任务拥有的下载进程，保留未完成文件。后续按固定官方地址分段续传，最后必须同时满足完整长度、官方SHA256与有效微软/Docker签名才可安装。没有禁用TLS验证、使用第三方镜像或执行未校验文件。

## 重启前后恢复顺序

1. 下载完后核对签名与官方hash，使用微软MSI `/qn /norestart REBOOT=ReallySuppress`安装WSL程序；再按当前用户及WSL2后端安装Docker Desktop，将数据目录指向D盘。实际结果追加在下文，不以本计划代替执行成功。
2. 若Windows仍要求重启，由用户保存其他软件编辑后自行重启。代理不调用Restart-Computer、shutdown或强制关闭其他应用。
3. 重启后先检查`wsl --version`、`wsl --status`、`docker version`。Docker若显示首次运行协议，由用户自行确认；不将目录列举、CLI版本输出或应用窗口打开当成Linux服务运行成功。
4. 服务就绪后下载官方`python:3.13-slim`，实取Image ID/RepoDigests及Python3.13版本。运行器仍只接受实际本地`sha256:`镜像，不在评测时自动拉取。
5. 复用现有`benchmark/python.rs`完成受限配置、正反答案、超时/输出/取消与容器清理实测。无宿主目录或Socket挂载，不在宿主机执行提交代码。
6. 环境通过后，以当前实际模型身份建立新的Python题验收计划，保留原20项Docker阻断；然后进入D2b-1任务实现。没有真实环境证据前不进入D3。

官方来源：[Docker Windows安装/参数及MSI方式](https://docs.docker.com/desktop/setup/install/windows-install/)、[Docker 4.97.0发布与校验入口](https://docs.docker.com/desktop/release-notes/#4970)、[微软WSL 3.0.3](https://github.com/microsoft/WSL/releases/tag/3.0.3)。完整命令与失败记录见里程碑。

## 本轮实际结果：程序已安装，等待手动重启

| 项目 | 实际状态 | 依据 |
| --- | --- | --- |
| 两个官方安装包 | 完整下载、SHA256与数字签名通过 | 90个分段完成，最终字节数与上述官方hash一致；Microsoft Corporation / Docker Inc签名均Valid |
| WSL程序 | 安装成功 | MSI退出0；`C:\Program Files\WSL\wsl.exe`文件版本3.0.3.0 |
| Docker Desktop程序 | 当前用户安装成功 | 安装器退出0；`%LOCALAPPDATA%\Programs\DockerDesktop\Docker Desktop.exe`文件版本4.97.0.244584 |
| D盘数据位置 | 安装参数已指定，目录已创建 | `--wsl-default-data-root=D:\CortexOSRuntime\DockerData`；实际VM磁盘创建位置在首次运行后复核 |
| Windows重启 | 需要，尚未执行 | VirtualMachinePlatform启用返回RestartNeeded=true，所有安装操作未触发重启 |
| Linux Docker服务 | 尚未验证 | 等待用户重启并打开Docker Desktop，首次协议由用户确认 |
| Python3.13镜像 | 尚未下载 | 需Linux服务就绪后pull并固定实际Image ID |
| 容器安全、旧20项补验 | 尚未运行 | 服务与镜像门槛未满足；原历史记录未修改 |

本轮没有应用源码/迁移变化，未运行无关单元、模型或仓库代码。安装脚本语法检查通过，实际签名/哈希及安装退出码如表，不把它们称为Linux运行验收。安装退出结果在`D:\CortexOSRuntime\Downloads\installation-results.json`，核验清单在`verified-installers.json`，MSI日志在`wsl-install.log`；本机恢复进度在仓库`artifacts/harness-runtime-setup/progress.json`。

**2026-10-30用户要求暂存此步骤，先完成不依赖重启的开发。** 当前不继续启动Linux服务、拉镜像或执行容器。用户方便时再保存其他应用编辑，手动重启Windows并打开Docker Desktop，首次运行如有协议提示由用户确认；之后恢复Docker服务/数据目录/Python镜像与现有隔离控制器验收。原安装授权继续有效，旧20项阻断不覆盖。

## 2026-10-30：用户报告重启后复核，Linux服务仍未就绪

用户表示“重启好了”，因此恢复原授权范围，只读检查真实状态。WSL3.0.3程序正常输出版本，但`wsl --status`明确报告WSL2无法启动；尽管退出码为0，也不能判定为服务可用。当前VirtualMachinePlatform=Enabled、固件虚拟化=true、SLAT=true，但HypervisorPresent=false，Component Based Servicing仍有RebootPending。实际读取LastBootUpTime为2026-10-29T08:07:25.5000000Z，与安装前基线相同；这表明当前内核尚未体现新启动，不能据此推断用户具体采用了何种重启方式。

已安装Docker CLI29.9.2可运行，但`docker version`的Server=null，本地docker_engine命名管道不存在。Docker Desktop进程与首次settings-store.json均未出现；D盘目录存在，实际虚拟磁盘位置仍未核验。没有重复安装、改BIOS、修改bcdedit配置、启动容器或下载镜像。

只读`bcdedit /enum '{current}'`未显示显式hypervisorlaunchtype Off，尚无依据改启动配置。按[微软WSL故障排查](https://learn.microsoft.com/en-us/windows/wsl/troubleshooting#error-0x80370102-the-virtual-machine-could-not-be-started-because-a-required-feature-is-not-installed)先确认Windows完整重新启动，再核对实际Hypervisor状态；已向用户确认重启方式。若用户确认已做Windows“重新启动”，继续定位启动未加载原因，不能反复将重启当成已解决。

本机记录：`artifacts/harness-runtime-setup/post-reboot-check.json`。Python镜像、安全隔离与旧20项补验均未运行，历史数据未改，原安装授权继续有效。

用户随后确认采用了“Windows关机后再开机”。微软[电源状态说明](https://learn.microsoft.com/en-us/windows/win32/power/system-power-states)指出，快速启动会保留内核会话，Windows重新启动执行完整启动周期。因此本机现象与快速启动路径吻合，但未把“实际启用了快速启动”作为已检测事实。当前请用户保存编辑，选择开始菜单→电源→重新启动，回来后打开Docker Desktop并处理首次提示，再恢复真实服务检查；不需要修改BIOS、关闭快速启动或重复安装。

## 2026-10-30：完整重启后Docker启动报错的恢复

用户提供“getting backend binary path / cannot find registry key”截图。此次只读确认LastBootUpTime已变为2026-10-30T02:10:49.5000000Z，HypervisorPresent=true、CBS RebootPending=false；WSL不再报WSL2虚拟化不可用。缺少WSL1组件的提示与本项目WSL2方案无关，没有为此安装其他组件或发行版。

Docker卸载登记和程序文件存在，但HKCU/HKLM两种32/64位视图均未找到截图指定的Docker Inc.键，启动日志复现同一错误。通过普通Windows桌面Shell与当前工具分别运行同一只读脚本，两边结果一致；GetCurrentPackageFullName均返回15700（没有包身份），安装文件物理路径为真实LocalAppData，排除了本次两环境路径视图不同的假设。没有从公开Issue的相似症状推断本机根因。

恢复动作均保持原当前用户/WSL2/D盘授权范围：

1. 重验原官方完整安装包：SHA256仍为2668273f…59744，Docker Inc数字签名Valid。保留旧日志与卸载登记；实际WSL无已安装发行版，D盘及默认Docker/wsl路径均没有VHDX。
2. 原完整安装包直接`install --user --quiet --backend=wsl-2 --wsl-default-data-root=D:\CortexOSRuntime\DockerData`返回3，没有补回登记，不把退出3称为修复成功。
3. 按[Docker官方卸载入口](https://docs.docker.com/desktop/uninstall/)运行本次新安装的已签名`Docker Desktop Installer.exe uninstall --quiet`，退出0；随后原完整安装包按相同参数重装，退出0。只进行一次受控卸载/重装，没有手工清理注册表、删除目录或切换全用户安装。CortexOS及Ollama数据不参与操作。
4. 实际启动重装后的Docker Desktop；新日志显示已启动com.docker.backend，`docker version`取得Linux Server29.9.2，Desktop4.97.0/244584、WSL内核6.18.54.1、linux/amd64、cgroup2。**即使重装后旧注册表键仍不存在，实际启动已成功；具体哪项安装状态恢复导致变化尚未定位，不能声称已手工补回该键。**
5. 实际虚拟磁盘为`D:\CortexOSRuntime\DockerData\disk\docker_data.vhdx`及`main\ext4.vhdx`，默认LocalAppData Docker/wsl下没有VHDX。安装指定值与实际磁盘位置分别验证。
6. 在已授权范围执行官方`python:3.13-slim`下载成功。实际Image ID与RepoDigest记录为`sha256:2d44738ebf4ce5f63b9d9ea871e9c7cfdc0426f7b0c97cac915fd88ded1d86bb`，linux/amd64；评分只传固定本地ID，不自动拉取。

本机证据：`artifacts/harness-runtime-setup/{native-context-probe,tool-context-probe,after-reinstall-probe,docker-environment}.json`、`docker-repair-attempt/`。两个原模型的实际tag/digest及Ollama0.40.9与历史一致；安全验证与20项独立补验的最终结果另记，不因服务已运行提前宣称通过。安装脚本均未使用--accept-license，不替用户处理首次协议，也未读取或使用云端凭据。
