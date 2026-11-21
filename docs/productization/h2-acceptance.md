# H2 数据管理验收

## 交付范围

| 切片 | 入口与交付 |
| --- | --- |
| H2.1 数据资产审计 | [资产盘点及实现说明](h2-data-management.md)覆盖本机库、界面保存、外部环境和凭据 |
| H2.2 备份 | 设置中的全量和工作区备份、范围预览、用户选址、包清单和校验、保存记录 |
| H2.3 恢复 | 暂存、显式确认、重启切换、全量替换、工作区合并与冲突检查、自动安全备份及结果历史 |
| H2.4 凭据 | 重新检查当前 Windows 凭据，对还原的连接原位绑定，保留连接编号 |
| H2.5 空间管理 | 复用回收确认与撤销，核验重建后的缓存再清理旧副本；补充备份暂存、知识、评测和 Harness 数据的实际字节 |
| H2.6 导出 | 知识 JSON 与 Markdown、现有评测 JSON 与 CSV、结构化备份包 |

[实施计划](h2-h5-implementation-plan.md)列出了 H3–H5 的阶段依赖及门槛。[专项验证](h2-backup-restore-test.md)列出了测试前提、命令和结果。

## 修改清单

- Rust 备份、恢复、工作区投影、知识导出及测试位于 \`src-tauri/src/portability.rs\` 和 \`src-tauri/src/portability/\`。
- 数据库启动与版本校验更新 \`db.rs\`，存储清单及缓存清理更新 \`storage.rs\`、\`storage/recovery.rs\`，云端凭据操作更新 \`cloud/runtime_connections.rs\`。
- 桌面接线更新 \`commands.rs\`、\`lib.rs\`、\`build.rs\`、\`capabilities/default.json\`。新权限限定于主窗口的备份、恢复、知识导出和凭据绑定操作。
- 前端新增 \`DataPortability.tsx\`、\`api/portability.ts\`、\`portable-ui.ts\`，接入 \`Settings.tsx\`、\`draft-store.ts\`、\`main.tsx\`、\`AppearanceControl.tsx\`、\`CloudRuntimeSettings.tsx\` 和 \`StorageInventory.tsx\`。
- 新增 \`tests/data-portability-native.spec.ts\`，扩展 \`tests/storage-recovery-native.spec.ts\` 并由 Playwright 桌面项目运行。

应用现有数据库迁移编号保持 26。备份格式为 1。Rust 和 npm 依赖保持原锁定版本。备份与恢复涉及一致性复制、哈希和压缩，随数据大小增加磁盘工作量。恢复会先核对本机空间，隔离样本不用于声称大数据库的处理时长。

## 构建与环境

- 在 \`apps/desktop\` 执行 \`npm.cmd run build\`、\`npm.cmd run lint\`、\`npm.cmd run format:check\`。构建含 TypeScript 检查，均通过。保留 H1 已有的 63 条 lint 警告和按需图谱包大小提示。
- \`cargo build --manifest-path src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop\` 通过。原生验证使用此调试构建、独立 Windows 桌面和隔离数据目录。
- 容量不足采用注入测试。正式安装、跨机器迁移、真实云端请求和生产大库耗时未测。实际文件大小、失败初查和最终测试证据见[验证记录](h2-backup-restore-test.md)。

## 验收步骤

在独立测试实例打开「设置 → 本地数据」。为应用创建完整备份，再在另一个实例打开备份，检查工作区和哈希，确认预约并重新启动。核对历史、知识与草稿，并在最近恢复记录查看安全备份。单工作区包可按原编号导入到另一实例。目标已含该编号时需要换一个实例或使用经过确认的完整备份。

正式安装升级、物理设备之间迁移和真实云端连通性单独验收。运行时容器中的临时内存副本与测得的库内项目副本保持独立。完成 H2 后等待验收，再推进 H3。
