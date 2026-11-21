# H4 第三方许可证审核

维护者决定暂不指定根项目许可证。项目授权仍为正式发布阻断，第三方许可证按各自条款保留。当前仓库没有 `LICENSE`，公共仓库的可见性不能替代项目授权。

候选构建通过锁定 `cargo metadata` 区分 Windows 目标的普通依赖和其余依赖。仅由构建脚本或过程宏引用的库单列，编入实际二进制的覆盖文件由发布者复核。前端使用同一次生产构建的源映射，归因实际参与打包的模块。源映射仅保留在本地证据。每个候选生成 `license-audit.json`、`THIRD-PARTY-NOTICES.txt`、`third-party-materials.zip`。后两个文件随安装器提供，候选验收会核对解包后的哈希。第三方材料标注包名、锁定版本、已找到的许可原文和待复核项。

## 重点记录

- **MPL-2.0**。对于随二进制分发的 MPL 覆盖文件，告知接收者如何取得同版源码并保留适用声明。`release_inventory.py` 对发现的运行时 MPL 依赖核对官方 `.crate` 与锁文件 SHA-256 后，将源码归档与相应声明放入材料包。另行标记构建依赖与找不到对应源码的项。 [Mozilla 许可 3.2](https://www.mozilla.org/en-US/MPL/2.0/) · [Mozilla 分发说明](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)
- **前端缺项**。`webgl-constants 1.1.1` 的 npm 元数据缺少许可证，包内原文为 MIT，应照原文保留版权。`@react-three/fiber 9.8.2` 的发行目录缺少 LICENSE，采用上游同版本标签许可文本，构建前按固定 SHA-256 验证。 [React Three Fiber 9.8.2 上游许可](https://github.com/pmndrs/react-three-fiber/blob/v9.8.2/LICENSE)
- **Codex seccomp**。主程序内嵌的 Moby 衍生配置随材料包保存对应 Apache-2.0 文本、来源固定提交、改动说明和实际配置。工具安装包和外部 Docker 镜像各自有独立许可范围。 [项目使用与修改记录](../../apps/desktop/src-tauri/catalogs/codex-seccomp/README.md) · [Apache-2.0 第 4 节](https://www.apache.org/licenses/LICENSE-2.0)
- **附带的工具资源**。实际构建使用 NSIS 3.13，随包收录其许可、NSISdl 和 Modern UI 2 的原文，Tauri NSIS 插件随附 0.5.3 的原始 MIT 与 Apache-2.0 文本并核对发行摘要。WebView2 Evergreen、Ollama 模型、Docker 镜像和外部编程助手不作为应用随附资源。 [NSIS 3.13 发行包](https://github.com/tauri-apps/binary-releases/releases/tag/nsis-3.13) · [Tauri NSIS 插件 0.5.3](https://github.com/tauri-apps/nsis-tauri-utils/releases/tag/nsis_tauri_utils-v0.5.3)

Rust 发行包的缺项按锁定版本保存上游许可原文。保留 `selectors 0.38.0` 覆盖源代码和 MPL 文本，其余四个上游分别核对固定提交。 [alloc-stdlib](https://github.com/dropbox/rust-alloc-no-stdlib/blob/0a81fd6928ea3b33c8cd484aa4575d50ffb98012/LICENSE) · [jsonschema](https://github.com/Stranger6667/jsonschema/blob/4eb7a64d9cf45bcbf38c7a69e544477294460751/LICENSE) · [simd](https://github.com/Nugine/simd/blob/d74c030d9dc4f3cae02146d1f497ff62726ef09a/LICENSE) · [WebView2 Rust](https://github.com/wravery/webview2-rs/blob/edc2caf886175ccaebe86078c9cfe1ae2a187328/LICENSE)

多许可条目记录上游原始表达式。正式分发前还需由维护者核对最终采用条款、适用版权原文、上游 NOTICE 和所有人工复核项。候选材料用于核查，不能替代权利人授权。
