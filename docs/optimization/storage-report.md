# Phase G0 磁盘与数据生命周期报告

2026-11-18，使用 Python 3.13 的 scandir / lstat 读取文件元数据。原始记录见 [storage.json](../../artifacts/phase-g0-2026-11-18/storage.json)、[细分统计](../../artifacts/phase-g0-2026-11-18/storage-detail.json)、[数据库只读观测](../../artifacts/phase-g0-2026-11-18/databases.json)。扫描限定于项目、应用数据、Ollama 模型和已知 Docker 安装数据目录。

## 口径

表中使用实际逻辑字节数，GB 为十进制 10⁹ 字节，GiB 为 2³⁰ 字节。链接与 reparse point 跳过并记录，硬链接与相同内容没有去重。逻辑文件总和不能直接等同可回收物理空间。Docker VHDX 另外通过 GetCompressedFileSizeW 测了宿主分配字节。本轮没有删除、压缩、截断或重建数据。

初次仓库扫描为 **52068461892 字节，143288 个文件，0 个读取错误**，约 52.07 GB / 48.49 GiB。后续细分扫描发生在生成审计副本之后，数字有时间差，不能把两批总量强行加总。

## 开发与安装环境

| 项目 | 实测字节 | 路径与范围 |
| --- | ---: | --- |
| Rust target，细分扫描 | 36592838606 | D:/vibecoding/CortexOS/apps/desktop/src-tauri/target |
| 历史与本轮审计产物，细分扫描 | 15245951646 | D:/vibecoding/CortexOS/artifacts |
| node_modules | 331409790 | apps/desktop/node_modules；包含开发工具、源码映射和 Vite 缓存 |
| React / TypeScript / CSS 源码 | 871470 | apps/desktop/src，共 111 个文件 |
| Rust / 容器辅助 Python 源码 | 1773273 | apps/desktop/src-tauri/src，共 129 个文件 |
| 内置 Benchmark 数据集 | 387693 | apps/desktop/src-tauri/datasets，共 10 个文件 |
| 前端 dist | 1919845 | apps/desktop/dist，共 6 个文件 |
| Git 元数据 | 22393470 | .git，首轮扫描 |
| 文档与设计参考 | 16509420 | docs，首轮扫描，包含参考图 |
| 最新 Phase F 调试程序 | 49598976 | artifacts/cortexos-phase-ef-final.exe |
| 历史 Release 程序 | 7915008 | apps/desktop/src-tauri/target/release/cortexos-desktop.exe，文件时间 2026-10-22 |
| v0.2.0 NSIS 包 | 2948478 | apps/desktop/src-tauri/target/release/bundle/nsis/CortexOS_0.2.0_x64-setup.exe |
| v0.1.0 NSIS 包 | 2949609 | 同目录 CortexOS_0.1.0_x64-setup.exe |

最新调试程序和历史 Release 包对应不同代码及构建配置，二者大小差异不能当成本轮优化收益。node_modules、Rust target、开发工具和验收 WebView profile 属于开发环境。NSIS 包体积只是安装介质大小。

只读查询当前用户卸载登记未发现 CortexOS 安装位置，两个常见安装目录也不存在。因此正式安装目录中的程序总体积为**未测量**。实际应用数据目录已定位，但其数据库仍为 schema 7，与最新测试库 schema 26 有版本差异。

| 正式应用相关目录 | 实测字节 | 说明 |
| --- | ---: | --- |
| %APPDATA%/com.cortexos.desktop | 2159144 | 数据库、WAL、SHM、4 个迁移备份和锁文件 |
| %LOCALAPPDATA%/com.cortexos.desktop | 至少 38920796 | WebView2 profile；9 项元数据读取被拒绝，数字为可读部分下界 |
| %USERPROFILE%/.ollama/models | 4994593302 | 17 个文件，两个已安装 Qwen 标签。模型属于外部共享资源 |
| D:/CortexOSRuntime/DockerData | 7051673600 | 两个 VHDX，宿主文件总量 |
| D:/CortexOSRuntime/Downloads | 1738252340 | WSL / Docker 安装介质、分片和核验文件 |

Docker 数据盘 D:/CortexOSRuntime/DockerData/disk/docker_data.vhdx 为 6951010304 字节，分配字节也为 6951010304；main/ext4.vhdx 为 100663296 字节，分配字节相同。镜像层、容器可写层及构建缓存位于其中，当前引擎未运行，docker system df -v 无法取得明细。各项分别为**未测量**，不能把整个 VHDX 当作 CortexOS 缓存。

## 最大十项占用来源

以下采用互不重复的分类，artifacts 的子类与整个 artifacts 不同时相加。它们来自细分扫描及外部目录扫描，体现当时测量规模。

| 排名 | 来源 | 实测字节 | 所在路径 / 归属 |
| --- | --- | ---: | --- |
| 1 | Rust 编译产物 | 36592838606 | apps/desktop/src-tauri/target |
| 2 | Docker 虚拟磁盘 | 7051673600 | D:/CortexOSRuntime/DockerData，共享外部运行环境 |
| 3 | 历史验收 WebView profiles | 5738549194 | artifacts 下 401 个名为 webview-profile 的目录 |
| 4 | Ollama 模型 | 4994593302 | %USERPROFILE%/.ollama/models |
| 5 | 调试 PDB | 3977617408 | artifacts/**/*.pdb，已排除前述 profile |
| 6 | 其他验收证据 | 1753947469 | artifacts 下尚未归入其他类别的证据与产物 |
| 7 | WSL / Docker 下载介质 | 1738252340 | D:/CortexOSRuntime/Downloads |
| 8 | 仓库下载目录 | 1582389770 | artifacts/downloads；其中 OllamaSetup.exe 为 1581967176 字节 |
| 9 | 验收 SQLite、WAL、SHM | 793446840 | artifacts 的数据库文件，已排除 WebView profiles |
| 10 | Harness 工具缓存 / 副本 | 744278477 | artifacts 中 Harness 的 context、wheels、cache、workspace 等目录 |

第 6 类混有不可变审计证据，第 10 类也混有 CLI 来源、构建上下文及任务产物，尚未证明可回收。十项是占用排名，回收资格必须另做引用检查。

Rust target 中 .o 为 15819369999 字节，.rlib 为 5946353324，.bin 为 4580773278，.pdb 为 3871297536，.lib 为 3500708640，.rmeta 为 1886348140。单个静态库文件可达 1748768042 字节。既有 crate-type 同时输出 staticlib、cdylib 和 rlib，是 G1 构建测量对象。

## SQLite、WAL、FTS5 与向量

首轮已定位的业务/验收 .sqlite3 文件共 747 个、308645888 字节，关联 WAL 共 461896352 字节。该统计包括迁移备份，排除了名为 webview-profile 的目录；不代表 747 个活跃数据库。审计读取 schema、行数、索引、长度和只读查询计划，并执行简化列表 SQL 计时。正文结果不写入报告或原始统计，只保存行数、耗时及计划。

正式数据路径 %APPDATA%/com.cortexos.desktop/cortexos.sqlite3 的主文件为 4096 字节，WAL 为 1495592，SHM 为 32768。SQLite 视图中为 93 页 × 4096 字节、空闲页为 0；有效页已包含 WAL 的最新可见页面，不能将其再次作为独立物理占用相加。库有 1 工作区、1 会话、10 消息、2 知识节点、5 次执行和 50 条事件。

Python SQLite 未编译 dbstat，**逐表及逐索引的物理页占用未测量**。可测到正式库 knowledge_fts_data.block 的 payload 共 586 字节，属于内容长度，未计页及索引开销。正式库 schema 7 没有向量表。

schema 26 的原生验收副本中，rag_vectors.vector payload 为 20 字节，其中 1 个未激活旧 profile 仍留有 1 行；两个 profile 维度不同，是受控模型切换的历史样本。它只证明重建数据存在保留生命周期，不能据此认定正式库有向量泄漏。现有样本未查到相同字段、唯一性和过滤条件下的重复 B-tree 索引。重复 content_hash 跨模型或知识版本可能合法，回收时需检查实际引用。

本轮 CPU 批次隔离库采样时主文件为 888832、WAL 为 1841672 字节；只读观测保存在 execution-write-observation.json。该值与正式库及原始验收副本分别记录，不能当作正式库 WAL 增长。未采集连续 WAL 曲线，未执行 checkpoint 来改变现状。

## 回收方案及恢复要求

G2 先提供容量和引用预览，默认仅报告。用户原文、知识版本、请求快照、评测成绩、来源及审计事件长期保留。可重建索引以模型与内容指纹检查引用，重建验证后才允许回收旧索引。任务副本需绑定 job/container 所有权、终态和未导出修改。WebView cache 与导出报告、数据库和截图分开统计。

任何回收都需明确确认、逐文件清单、保留/恢复路径和结果日志。开发 target 或工具缓存的回收收益只能在隔离副本做完构建恢复后确认。Ollama 模型和系统 Docker 镜像走其各自管理入口，保持共享所有权边界。
