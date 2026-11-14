# Phase D0：Execution Readiness

2026-10-29，基线提交0a1c7a9。依据实际代码、Phase C交付记录和本机只读检查；历史测试不是本轮重新执行。

| 模块 | 实现依据 | 实际验收及剩余门槛 |
| --- | --- | --- |
| 固定题库及六类评分 | `apps/desktop/src-tauri/datasets/cortex-core-60-v2/`、`benchmark/evaluation.rs`、`evaluation_json.rs`、`python.rs` | 切片8已有120计划/100完成调用/20 Docker阻断的真实本机证据；不能写成120道全部运行成功 |
| 个性化生成→修订→人工批准→发布 | `benchmark/personalized/{plan,generation,cases,datasets}.rs`，generation.rs的plan_generation/finish_generation/publish_personalized | C3–C5记录受控Provider链路和不可变发布通过；真实付费云端生成、账号配额/账单未验收 |
| Repository Profile | `benchmark/personalized/repository.rs::scan/save_repository_snapshot/repository_snapshot` | 文件夹/ZIP有界只读扫描、语言/依赖/符号/测试线索、不可变快照已实现；属于启发式分析，不是编译或代码正确性证明 |
| 云端凭据及接口 | `cloud/{mod,provider,completion,vault}.rs`，mod.rs::guard | Windows凭据库、OpenAI/Anthropic/Gemini目录及生成适配有代码和合成凭据/替身测试；目录通过不证明推理权限。不是第三方Harness通用认证 |
| 用途、工作区、任务隔离 | conversations.purpose；`tasks.rs`、`runner_v2.rs`、`personalized/batch.rs`、`report_read.rs` | 普通聊天和评测范围分离；切片8独立库普通聊天/知识均为空，停止恢复不重发完成项。TaskSlot是应用实例内互斥，不是跨进程操作系统沙盒 |
| 报告及独立评分 | `benchmark/{report_transfer,regrade,scoring_repository}.rs` | 切片7及8有真实保存/取消、原评分保留、无新模型请求对账；不等于仓库补丁评分已实现 |
| 实际图谱 | `execution.rs`、`ExecutionTrace.tsx`及既有图谱组件 | 可读真实执行事件；无Harness补丁/终端执行Adapter，不得补造此类事件 |
| 数据库 | `db.rs::migrate`、0001–0010迁移 | 版本/校验、升级前VACUUM INTO备份、事务回滚存在；D1只新增迁移，不修改历史 |

## 本机安全执行环境

本轮Get-Command未发现docker。wsl.exe存在，但`wsl.exe --status`退出50，解码后明确提示“未安装适用于Linux的Windows子系统”。没有执行安装、拉镜像或第三方Harness命令，也未读取个人凭据。

**结论：真实Harness代码修改/终端运行未就绪。D1纯本地文档分析可以实施。** D1没有子进程执行、云端请求或生产知识写入，不以Docker缺失阻断静态分析。此前20项编程阻断记录保留；环境通过后用同一题库和模型身份创建独立实验记录，不能覆盖历史。只重新评分原记录不能补出从未生成的Python答案。

## Docker/WSL配置与验收步骤（本轮未执行）

1. 经用户授权后，按[微软WSL说明](https://learn.microsoft.com/en-us/windows/wsl/install)安装/启用WSL2，按提示重启；用`wsl --status`及`wsl -l -v`确认实际发行版版本2。
2. 按[Docker Windows说明](https://docs.docker.com/desktop/setup/install/windows-install/)核对宿主要求，安装并启动Docker Desktop的Linux容器后端；用`docker version`和`docker info --format '{{json .OSType}}'`确认服务端可用且为linux。CLI可见不等于守护进程已就绪。
3. 另行确认下载Python3.13基础镜像；通过`docker image inspect`绑定实际sha256 Image ID，现有控制器不拉镜像、只接受明确的本地镜像。
4. 在独立验收库实测现有控制器的网络禁用、只读root、非root用户、CPU/内存/PID/时间/输出上限、无宿主挂载/Socket和取消清理。检查容器及临时文件确实清理，保留逐断言结果。
5. 新建两身份×10 Python题的独立计划，记录与原阻断运行的关联及新环境。D3仍需另审仓库工作副本、受控模型认证、隐藏测试隔离与补丁验证；Python纯函数容器通过不能自动授权Coding Harness。

## D1复用与文件方案

复用原生单文件安全读取及不可变RepositorySnapshot。新增本地D1业务命令读取文档、选取已有快照、规则提取、保存静态分析；不改变Phase C云端准入。新建单一追加式harness_fit_assessments表保存需求/修改、权重、目录与计算结果的版本快照，避免为每个未来名词建表。能力目录为版本化JSON，逐事实关联官方证据；未绑定本机版本或未实测保持未知/资料推断。

根目录design.md实际不存在。本轮UI使用AGENTS.md指定的`docs/design/html-prototype-ui.md`及现有Soft Glass Tokens，不声称读取了缺失文件。
