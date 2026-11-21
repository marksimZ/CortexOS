# H4 正式 Windows 安装验收

## 样本和环境

- 旧包使用本地留档的公开 v0.2.0 安装器。发布包 SHA-256 为 `B3A18E39B590C2615B024293E172E434949B263DAF37CD7D122FF8C71A9ED1EB`，Windows 签名状态为 `NotSigned`。 [公开发行版](https://github.com/marksimZ/CortexOS/releases/tag/v0.2.0)
- 新包使用本轮 Windows x64 NSIS 候选。`candidate.json` 记录实际文件名、版本、大小、签名和校验值。测试覆盖正式产品标识 `com.cortexos.desktop` 和当前用户安装位置。
- 使用两个独立的 Windows 沙盒会话。测试数据与 Credential Manager 留在一次性来宾账户。默认关闭网络、剪贴板重定向和硬件 GPU。来宾只读访问本次测试输入，单独写入本次结果。Windows 或 WebView2 阻止运行时记录原始状态。 [Windows 沙盒配置](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-configure-using-wsb-file)

## 验收矩阵

2026-11-21 启用沙盒后，重启成功解除 Windows 待重启状态。首次客户端更新在等待窗口内超时，Windows 先启动经典沙盒，随后完成官方客户端 `0.8.114.0` 安装。来宾保持离线，两个沙盒分别用于全新安装和升级。

首个离线来宾运行 rc.1 安装器失败。第二个来宾确认没有预装 WebView2，补齐微软签名的 Evergreen x64 后，rc.1 安装成功，但实际窗口打开构建机的 `file:///D:/…/frontend`，显示 `ERR_FILE_NOT_FOUND`。原因是构建覆盖配置把带盘符的绝对资源目录交给了 Tauri 的 URL 分支。rc.2 改用相对目录，并在解包后检查所有嵌入资源路径。rc.1 保留为故障证据，后续验收使用 rc.2。

来宾使用的 WebView2 离线安装器为 294,099,168 字节，Windows Authenticode 验证结果为 `Valid`，签发对象为 `Microsoft Corporation`，SHA-256 为 `47BC51387AFF95C1275CE38CFBBB5FBAB99B43B33214049877BBCDBCC7A970D5`。实际运行时版本为 `158.0.4370.33`。该运行时仅用于来宾验收。 [微软 WebView2 发行指南](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)

rc.2 全新安装、首次启动、重开、默认卸载并重装、显式删除均已在隔离来宾执行。通过应用界面绑定合成凭据，通过回环固定答复建立一条 Core 60 评测记录，重装后两项可读。删除选项只作用于来宾的应用目录，对照文件保留。旧版 v0.2.0 也已安装并打开，创建了 Schema 7 工作区。原文件占用及新路径参数未阻止 NSIS 替换程序。将默认路径中的主程序位置替换为有标记的目录，才实际阻碍写入。rc.2 在此条件返回 0，因此增加 NSIS 写入前检查并安排 rc.3 复测。

| 项目 | 判据和证据 |
| --- | --- |
| 全新安装、首次启动和重开 | 全新来宾安装正式名称的候选，核对卸载注册表、产品版本、实际 WebView2 加载和 SQLite 查询。 |
| 从真实 v0.2.0 升级 | 先用已发布安装包生成原位 Schema 7 数据库。录入合成工作区及两版知识，保留已生效版本。升级验证 Schema 26 与可读的 Schema 7 迁移备份。 |
| 升级失败的数据保护 | 在一次性来宾暂存已安装主程序，在其路径放置同名测试目录。采集退出状态、注册表、旧程序哈希和数据库完整性，归还原程序后再正常升级。 |
| 评测历史及 API 凭据 | 已公开 v0.2.0 的 Schema 7 尚无评测及运行密钥表。当前候选先用来宾回环固定答复创建一条真实本地评测记录，并通过原生接口绑定合成凭据。卸载与重装后再校验记录和凭据。 |
| 默认卸载保留数据 | 在来宾用随包卸载器移除程序与卸载注册表项，验证 SQLite 保留，重装后读取其工作区和专用凭据。 |
| 显式删除应用数据 | 只在一次性来宾勾选卸载选项，检查两个应用标识数据目录及不相关对照文件，保留退出与结果。 |
| 不同 DPI 和多屏 | 记录 Windows 显示设置、物理屏幕、窗口几何和鼠标操作。沙盒远程窗口与 WebView2 尺寸检查单独列出。 |

## 重放方法

用 `scripts/prepare-windows-release-sandbox.ps1` 准备只读输入和两个 `.wsb` 文件。准备命令需要候选输出目录、已发布安装器和 `artifacts` 下的新输出目录。若来宾缺少运行时，通过 `-WebView2Installer` 传入微软官方 Evergreen Standalone x64 安装器，脚本核对签名和 SHA-256，来宾仅执行本地副本。来宾执行 `scripts/windows-release-guest.mjs` 的 `fresh` 或 `upgrade`，并通过 Windows UI Automation 操作真正安装的程序。各 `.wsb` 分别从干净会话启动，运行时保留 `guest.log` 和 JSON。 [WebView2 下载页](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

手动检查时，在来宾启动安装器和应用，先保留一份样本工作区或备份。分别测 100% 和 150% 缩放，打开对话与设置，在弹窗场景拖动主标题栏，调整边缘大小，最大化再恢复。在支持多屏的隔离环境把窗口从一屏拖到另一屏，核对点击坐标、布局与弹窗关闭。保存测试条件和实际结果。
