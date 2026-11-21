# H4 发布工程

## Windows 身份

保持已发布 v0.2.0 和候选包的产品名 `CortexOS`、标识 `com.cortexos.desktop`、Windows x64、NSIS `currentUser` 和主程序 `cortexos-desktop.exe`。NSIS 在当前用户安装，默认位置 `%LOCALAPPDATA%\CortexOS`。卸载项位于 `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\CortexOS`。数据使用 Tauri 的应用数据目录，Windows 下为 `%APPDATA%\com.cortexos.desktop`。凭据由当前 Windows 用户的 Credential Manager 管理，其中通用模型连接还绑定数据库路径、工作区和连接编号。迁移流程继续读取原应用数据目录。

候选构建传入独立版本。应用关于页、诊断、备份、评测和项目评测的新记录采用实际打包版本。历史快照按原值读取。

NSIS 的 WebView2 模式为 `downloadBootstrapper`。已有可用运行时可离线安装 CortexOS。缺少运行时的设备需要先按 Windows 的标准方式获取 Evergreen 运行时。默认卸载保留应用数据。只有用户在卸载界面选中「删除应用程序数据」才删除应用数据目录，NSIS 的更新模式跳过该删除分支。凭据由应用单独移除或随可丢弃 Windows 用户环境消失。

## 更新渠道与用户操作

入口位于「设置 → 关于 → 应用更新」。用户选择稳定版或预览版，主动检查、查看说明、下载和校验，再确认安装。稳定版只接收正式 SemVer。预览版可接收较新的预发布版和正式版。两个渠道均拒绝同版和降级。应用启动不访问更新渠道。

正式包通过构建配置注入 `plugins.updater.pubkey` 和相应的 `stableEndpoint`、`previewEndpoint`，内容只含公钥与受维护者管理的 HTTPS 清单地址。安装器需由清单同源直接返回，无跳转。基础配置暂时使用手动更新。与用户界面隔离的本地回环演练仅存在于调试构建。调试版可校验包，实际安装由 NSIS 正式安装版执行。

Tauri 官方更新组件锁定 2.14.1，复用现有 Rust 网络栈。启用 `requireSignedVersion` 后，安装器字节和签名中的应用版本均需与更新匹配。更新包限 128 MiB，下载进度基于实际接收字节。下载完毕保存于当前进程，安装前再次验证。安装时确认退出，若仍有生成、评测、存储任务或项目副本待收尾则保留应用及已验证更新包。Windows 启动安装器失败时可重试。帮助链接及更新说明只以纯文本显示。 [Tauri updater 官方说明](https://v2.tauri.app/plugin/updater/) · [签名版本校验实现](https://docs.rs/tauri-plugin-updater/2.14.1/tauri_plugin_updater/struct.Update.html)

## Authenticode

- 当前 NSIS 路线适用受 Windows 信任的证书。可比较 CA 提供的 OV 或 EV 签名和托管签名服务。OV 或 EV 通常需身份验证及硬件令牌或 HSM，费用按提供商和签发主体评估。微软文档的 OV 参考区间约 150–300 美元/年。Microsoft Artifact Signing 的参考起价约 9.99 美元/月，有地区和身份限制。适用地区、报价和条款以申请时复核为准。 [Microsoft 签名方式比较](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options)
- 维护者核准发布者身份后，将签名密钥保留在硬件或托管签名服务，流水线只获取执行签名所需权限。用 SHA-256 对主程序和 NSIS 安装器签名，用 RFC 3161 的 SHA-256 时间戳。产物完成后以 Windows 信任策略验证签发者、证书链和时间戳，记录 SHA-256，再生成针对最终安装器的 Tauri 更新签名。证书更换前先安排身份验证和重叠期。 [SignTool 与时间戳](https://learn.microsoft.com/en-us/windows/win32/seccrypto/time-stamping-authenticode-signatures) · [Tauri 签名接入](https://v2.tauri.app/distribute/sign/windows/)
- Authenticode 验证 Windows 应用发布者。Tauri 更新签名验证客户端接受的更新字节与版本。分别保管签名权限及 Tauri 私钥，演练密钥只用于本机文件，正式私钥存入维护者控制的机密存储。

## 本地 Dry Run

从仓库根运行 `scripts/release-dry-run.ps1`，明确指定 RC 版本、`artifacts` 下新的输出目录和经校验的 7-Zip。脚本核对仓库版本和正式身份，执行类型与 Rust 检查，在临时前端构建中收集依赖归因，然后移走源映射，离线构建本地候选安装器。继续执行 NSIS 归档校验、解包白名单、材料哈希、候选 SHA-256、签名状态和指定凭据模式检查。`-RunTests` 可选运行已有 Node 测试及 Rust 存储专项，原生集成检查单独执行，以便隔离 Windows 桌面。脚本只生成本地工件和发布说明。

正式更新所需的最终包，可使用 `tauri build` 的 `createUpdaterArtifacts` 生成签名，或用 `tauri signer sign --app-version` 对已经 Authenticode 签名的最终字节签署对应版本。渠道清单必须与最终文件、签名和版本一起校验。托管上线需独立批准。

## 已验证的更新路径

`updates-native.spec.ts` 在非活动 Windows 桌面的原生 WebView2 上用 `tauri signer` 生成本轮专用签名。最终候选包复跑 1 项，用例耗时 4.1 秒，整次运行 4.7 秒。打开关于页时无更新请求，稳定版忽略候选版，预览版忽略仅增加构建标记的当前版本。篡改包、错误签名版本、跨地址下载、过大响应和服务 503 均验证了对应拒绝或失败状态，正确签名完成校验。调试包的实际安装入口不可用。正式安装交接仍待隔离来宾。原始日志与两种尺寸截图位于 `artifacts/phase-h4/updater-verified-rc.log` 和 `artifacts/phase-h4/updater-verified-rc/`。

## 候选构建与改动

- `scripts/release-dry-run.ps1 -Version 0.2.1-rc.1 -OutputDirectory artifacts/phase-h4/verified-rc -SevenZip artifacts/phase-h0/installer-audit/portable/7z.exe` 从干净的 `dcad612` 完成 Windows x64 候选构建。TypeScript、锁定的 Rust 检查、前端生产构建和 Tauri NSIS 打包通过。Vite 保留原有图谱块体积提示。没有执行全量单元测试。
- 相关代码包括 `updates.rs`、Tauri 配置、命令许可及 `AppUpdates.tsx`。应用版本读取共用于诊断、备份、评测与项目评测的新记录。SQLite 迁移和项目身份沿用原配置。
- 发布工具包括 `release-dry-run.ps1`、`release_inventory.py`、两份来宾准备与执行脚本、更新集成测试及第三方许可材料。安装包比 H0 留档增加 178,620 字节。应用空闲资源和运行性能未在 H4 重测。
