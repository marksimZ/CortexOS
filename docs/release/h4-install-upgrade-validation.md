# H4 正式 Windows 安装验收

## 样本和环境

- 旧包使用本地留档的公开 v0.2.0 安装器。发布包 SHA-256 为 `B3A18E39B590C2615B024293E172E434949B263DAF37CD7D122FF8C71A9ED1EB`，Windows 签名状态为 `NotSigned`。 [公开发行版](https://github.com/marksimZ/CortexOS/releases/tag/v0.2.0)
- 新包使用本轮 Windows x64 NSIS 候选。`candidate.json` 记录实际文件名、版本、大小、签名和校验值。测试覆盖正式产品标识 `com.cortexos.desktop` 和当前用户安装位置。
- 使用两个独立的 Windows 沙盒会话。测试数据与 Credential Manager 留在一次性来宾账户。默认关闭网络、剪贴板重定向和硬件 GPU。来宾只读访问本次测试输入，单独写入本次结果。Windows 或 WebView2 阻止运行时记录原始状态。 [Windows 沙盒配置](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-configure-using-wsb-file)

## 验收矩阵

2026-11-21 通过 Windows 官方组件安装命令启用沙盒，系统返回 `3010`，需要一次重启。当前宿主仍未出现可启动的沙盒程序。下表验收项为预定判据，正式包尚未在来宾执行。

| 项目 | 判据和证据 |
| --- | --- |
| 全新安装、首次启动和重开 | 全新来宾安装正式名称的候选，核对卸载注册表、产品版本、实际 WebView2 加载和 SQLite 查询。 |
| 从真实 v0.2.0 升级 | 先用已发布安装包生成原位 Schema 7 数据库。录入合成工作区及两版知识，保留已生效版本。升级验证 Schema 26 与可读的 Schema 7 迁移备份。 |
| 升级失败的数据保护 | 对一次性来宾的已安装主程序施加临时文件占用。采集安装退出状态、旧程序哈希、数据库完整性，再释放占用后正常升级。 |
| 评测历史及 API 凭据 | 已公开 v0.2.0 的 Schema 7 尚无评测及运行密钥表。当前候选先用来宾回环固定答复创建一条真实本地评测记录，并通过原生接口绑定合成凭据。卸载与重装后再校验记录和凭据。 |
| 默认卸载保留数据 | 在来宾用随包卸载器移除程序与卸载注册表项，验证 SQLite 保留，重装后读取其工作区和专用凭据。 |
| 显式删除应用数据 | 只在一次性来宾勾选卸载选项，检查两个应用标识数据目录及不相关对照文件，保留退出与结果。 |
| 不同 DPI 和多屏 | 记录 Windows 显示设置、物理屏幕、窗口几何和鼠标操作。沙盒远程窗口与 WebView2 尺寸检查单独列出。 |

## 重放方法

用 `scripts/prepare-windows-release-sandbox.ps1` 准备只读输入和两个 `.wsb` 文件。准备命令需要候选输出目录、已发布安装器和 `artifacts` 下的新输出目录。来宾执行 `scripts/windows-release-guest.mjs` 的 `fresh` 或 `upgrade`，每项判据写入对应结果目录。各 `.wsb` 分别从全新会话启动，运行时保留 `guest.log` 和 JSON。WebView2 已存在时全程离线。缺失运行时，则准备经验证的微软官方 Evergreen Standalone x64 安装器后另行复测。 [WebView2 下载页](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

手动检查时，在来宾启动安装器和应用，先保留一份样本工作区或备份。分别测 100% 和 150% 缩放，打开对话与设置，在弹窗场景拖动主标题栏，调整边缘大小，最大化再恢复。在支持多屏的隔离环境把窗口从一屏拖到另一屏，核对点击坐标、布局与弹窗关闭。保存测试条件和实际结果。
