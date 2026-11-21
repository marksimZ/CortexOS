# Phase H0 正式 Windows 安装验收

## 当前包身份

正式产品标识 `com.cortexos.desktop`，Windows x64、当前用户 NSIS。数据路径和凭据隔离由应用标识、Windows 用户及对应工作区决定。

| 产物 | 字节 | SHA-256 | 签名 |
| --- | ---: | --- | --- |
| 已发布 v0.2.0 本地留档 `artifacts/release-v0.2.0/CortexOS_0.2.0_x64-setup.exe` | 2,948,478 | `B3A18E39B590C2615B024293E172E434949B263DAF37CD7D122FF8C71A9ED1EB` | `NotSigned` |
| G5 候选留档 `artifacts/phase-g5/CortexOS_0.2.0_x64-setup.exe` | 6,306,029 | `C8494E4CE5148D62E5D3A8FE2451CB181EF82DE8EF1070D684527A4B1E3FBB08` | `NotSigned` |
| H0 候选 `artifacts/phase-h0/CortexOS_0.2.0_x64-setup.exe` | 6,306,612 | `2A260774422FA69197413B81FC5B89EC3C993FBA1B7981F9D436D2AD59C3325B` | `NotSigned` |

本轮在本机用 `Get-FileHash -Algorithm SHA256` 和 `Get-AuthenticodeSignature` 复核三个文件，已发布 v0.2.0 的哈希与原同目录 `SHA256SUMS.txt` 一致。H0 用 `npm.cmd run tauri -- build --bundles nsis` 离线构建，安装器及主程序均为 `NotSigned`。官方发版页见 [v0.2.0](https://github.com/marksimZ/CortexOS/releases/tag/v0.2.0)。

## 2026-11-21 离线安装包检查

用经 SHA-256 核对的便携 7-Zip 26.06 执行 `l -tNsis -slt`、`t -tNsis` 和 `x -tNsis`。三个归档均通过完整性检查，各解出一个主程序、一个卸载程序和 7 项 NSIS 临时插件或界面资源，没有额外数据文件、源码目录或凭据文件。解出的主程序与安装器签名均为 `NotSigned`。逐文件摘要和日志在本地 `artifacts/phase-h0/installer-audit/installer-payloads.json`。 [7-Zip 26.06 官方归档和摘要](https://github.com/ip7z/7zip/releases/expanded_assets/26.06)

H0 主程序 SHA-256 为 `804D84E9E866E0A014C22F6EA9B71BA50A2213A8610CF20FD02817B8C1722624`。它与当时本地 Release 主程序只有三个字节不同，均位于 Tauri 安装包类型标记。将本地标记从 `UNK` 替换为 `NSS` 后逐字节相同。三个解出的主程序在原始字节扫描中，对凭据文件名、PEM 私钥及两种 GitHub Token 模式均为零命中。原始字节扫描覆盖所列模式，嵌入资源内容另见构建产物检查。此次验证为静态解包，正式安装与运行仍见下方验收矩阵。 [Tauri 安装包类型标记](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/platform.rs)

## 验收矩阵

当前 Windows 会话能在非活动桌面运行调试程序，没有发现可用的 Windows Sandbox 或 Hyper-V 管理工具。非活动桌面仍共享当前账户下的正式注册表和凭据，正式安装、升级或卸载可能改变真实用户环境。本轮对正式身份的 GUI 测试保留人工验收。

| 项目 | 实际证据 | 正式身份验收 |
| --- | --- | --- |
| 全新安装、首次启动 | G5 独立名称与注册表身份 NSIS 测试通过。H0 调试程序在非活动桌面启动并完成标准评测固定回复。 | 待测。 |
| 从已发布 v0.2.0 升级 | 上一发行包及其 SHA-256 已准备。G5 独立身份测试从旧验收程序迁移 schema 7→26，保留工作区和知识。 | 待从实际发布包升级，验历史与证据定位。 |
| API 凭据恢复 | G5 使用随机测试项验证过 Credential Manager 保存、重读、替换、删除。H0 阅读现有凭据目标命名及作用域。 | 待同一 Windows 用户、同一正式数据路径、同一工作区升级复核。 |
| 升级失败保护 | G5 有隔离副本的故障注入和恢复记录。 | 待在可回滚的 Windows 快照测试正式失败与恢复。 |
| 默认卸载保留数据 | G5 独立身份测试移除程序并保留 SQLite。 | 待从正式卸载器操作，复核数据目录和重装。 |
| 物理拖窗、缩放、最大化还原 | 既有 G 阶段记录包含自动化窗口检查。 | 待物理操作、多屏和不同系统 DPI。 |

## 人工验收步骤

1. 准备独立 Windows x64 VM 或专用账户。记录 Windows/WebView2 版本和测试包 SHA-256，备份并创建快照。按所在组织的执行策略处理签名；签名策略拒绝时保留失败证据。
2. 从空快照安装 H0 包，启动、关闭并重开。检查安装范围、快捷方式和主界面。记录是否需要安装 WebView2。
3. 回到空快照，安装真实 v0.2.0。建立独立工作区，录入合成聊天、带版本的知识、评测记录和专用测试凭据。使用应用支持的凭据入口；旧版无法创建某类数据时记录该差异，再用独立代表性副本测试。退出应用，对 SQLite 使用一致性快照或备份，并保留迁移前的文件清单和相应哈希。
4. 同一用户下安装 H0。比对工作区、知识版本、评测历史和执行证据。核对 SQLite `PRAGMA integrity_check` 与 `PRAGMA foreign_key_check`，检查迁移备份可读。凭据使用专用测试值，在同一身份走官方功能或隔离回环核对；真实供应商请求须另有授权。
5. 在另一快照用无写入权限的目标触发升级失败，保存日志，确认可重新打开原备份、原库记录未损坏，恢复权限后重复升级。用默认卸载移除程序，核对数据是否保留，再验证重装可读取；正式数据删除分支只在可丢弃测试环境操作。
6. 使用实体鼠标在主页面及弹窗打开时拖窗、调边角尺寸、最大化和还原。在两块显示器设置 100% 与 150% 缩放，双向跨屏，观察布局、点击坐标和弹窗操作。保留失败截图与复现路径。
