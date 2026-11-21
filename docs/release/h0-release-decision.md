# Phase H0 发布决定

**暂缓正式发布。** H0 的标准评测修复通过原生固定回复与有界真实单题，可进入独立 Windows 候选包验收。Windows 正式身份验收、签名以及分发材料仍须关闭。真实性能与模型验收见[性能复核](h0-performance-review.md)，本机 Windows 安装清单见[安装矩阵](h0-windows-validation.md)。

## 发布安全检查

| 问题 | 结果、方法与文件 | 发布影响 |
| --- | --- | --- |
| Windows 签名与完整性 | 使用 Windows Authenticode 验证与 SHA-256 复核。候选名、大小及哈希见[安装矩阵](h0-windows-validation.md)。 | 最终产物应按发布策略签名，并对签名后的字节重算校验值。没有采购证书，也没有更改 Windows 执行策略。 |
| 安装包内容 | 检查 [Tauri 配置](../../apps/desktop/src-tauri/tauri.conf.json)、NSIS 生成脚本、H0 前端构建产物及主程序。追加静态解包后，公开 v0.2.0、G5 和 H0 留档的安装器各有 9 个条目。清单、哈希及原始字节模式扫描见[安装矩阵](h0-windows-validation.md)。 | 签名后的实际发布产物仍要复核内容与哈希。 |
| 更新渠道 | [构建选项](../../apps/desktop/src-tauri/tauri.conf.json)、[Windows 交付文档](../windows-delivery.md) 与 [README](../../README.md) 显示当前通过 GitHub Releases 手动下载，未配置 Tauri updater、自动更新公钥或更新端点。 | 发布时同一入口提供安装器和 `SHA256SUMS.txt`，核对下载字节及 Authenticode 签发者。哈希一致只能证明与公布的字节相符。 |
| Docker 边界 | 代码复核 [Python 启动参数](../../apps/desktop/src-tauri/src/benchmark/python.rs) 及 [Harness 会话](../../apps/desktop/src-tauri/src/benchmark/harness/session.rs)。G5 原生固定回复通过真实 Docker 接力及归属清理。H0 没有更改容器条件，也未重跑全套隔离反向测试。 | 保留无网络、无宿主敏感目录及 Docker Socket、非 root、只读根、能力与资源限制。G5 限定场景证据可追溯。 |
| API Key | [Windows 凭据存储](../../apps/desktop/src-tauri/src/cloud/vault.rs) 通过 `CredReadW` / `CredWriteW`，凭据作用域含账户及工作区标识，调试与正式作用域分开。相关运行逻辑见[连接层](../../apps/desktop/src-tauri/src/cloud/runtime_connections.rs)。G5 合成凭据的保存、更新、重读、删除测试通过。 | 正式身份升级、恢复能力仍随安装矩阵验收。未读取真实用户凭据，也未发送真实云端请求。 |

## 依赖许可与分发

- Rust：离线重新提取 Windows x64 `cargo tree -e normal` 与 `normal,build`。G5 的 Windows 解析图有 320 个包（含主程序）。常见许可证为 MIT、Apache-2.0 和 Unicode-3.0。`option-ext 0.2.0`、`cssparser 0.37.0`、`cssparser-macros 0.7.1`、`dtoa-short 0.3.5`、`selectors 0.38.0` 在正常依赖树声明 MPL-2.0。正常依赖树也含过程宏，需沿打包边界确认实际分发的文件与编译产物，避免把构建机工具和用户收到的材料混同。原始审计输出为 `artifacts/phase-h0/cargo-windows-normal.txt` 与 `cargo-windows-build.txt`。
- npm：锁文件与 G5 一样，H0 按锁文件重新读取已安装的非开发依赖，共 169 项（见 `artifacts/phase-h0/npm-runtime-licenses.json`），常见许可为 MIT、Apache-2.0、ISC 和 BSD-3-Clause。H0 核对了元数据缺少 `license` 的 `webgl-constants 1.1.1`，随包 `LICENSE` 明示 MIT 和 Tim van Scherpenzeel 的版权。安装依赖清单与实际进入生产 Bundle 的模块需要分别核对，构建树计数不能直接代替分发清单。
- MPL-2.0 在实际分发所覆盖的可执行形式时，要求提供对应覆盖文件的源代码并告知接收者如何取得，保留适用声明与对源代码的权利。应固定实际分发依赖版本、源码取得方式与修改过的受覆盖文件，给安装用户可达的入口。核对 [MPL-2.0 第 3 节](https://www.mozilla.org/en-US/MPL/2.0/) 和 [Mozilla 分发说明](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)。
- 对实配 MIT、ISC、BSD、Unicode、Zlib 等依赖，按各自文本保留适用的版权和许可声明。对实际选用 Apache-2.0 的组件，随包保留许可文本、适用的 NOTICE 和修改说明。多许可条目按实际采用的条款记录。核对 [Apache-2.0 第 4 节](https://www.apache.org/licenses/LICENSE-2.0)。
- 仓库根缺少 `LICENSE`，安装资源未包含第三方许可汇总。现有 [Codex seccomp 目录许可](../../apps/desktop/src-tauri/catalogs/codex-seccomp/LICENSE) 归属于该目录。项目自身授权范围由权利人确定。正式包需带审批后的项目许可、第三方声明和适用源代码指引，并用最终包检查可读性。该决定与分发材料仍是发布阻断项。

## 发布准出

2026-11-21 复核前端分发清单。用当前源码构建测试专用生产源映射，31 个 JavaScript 产物的第三方源文件归入 85 个已锁定 npm 包，其中 82 项 MIT、2 项 ISC、1 项 MIT 或 Apache-2.0。G5 与当前锁文件的 169 个非开发条目逐项等值。该构建未包含开发专用依赖的源文件。已安装的 `@react-three/fiber` 9.8.2 目录缺少许可文本，已核对该发行标签的原文。原始归因证据在本地 `artifacts/phase-h1/release-inventory/npm-bundle.json`。Rust 实际分发范围、随包声明与正式包内容仍须一起复核。 [React Three Fiber 9.8.2 许可](https://github.com/pmndrs/react-three-fiber/blob/v9.8.2/LICENSE)

1. H0 标准单题、实际评分及释放已留证。图谱性能证据覆盖小图谱和模型请求的短时并行。若把长输出或大型图谱并行作为发布指标，另设相同条件复测。
2. 在独立 Windows 环境通过全新安装、实际旧发行版升级、凭据及历史保留、失败恢复、默认卸载、多屏和 DPI 验收。
3. 确认项目许可，完成实际分发清单与第三方声明，将文件收入最终安装包。确定签名和证书策略，经授权后签名、复核最终字节与签发者。相同版本号的 H0 本地候选用于验收，正式版本号与实际发布另行确认。
