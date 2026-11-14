# D2b-0：真实Docker隔离验收

> 2026-11-14 的新补验批次见 [Phase D 补齐交付](harness-fit-phase-d-completion.md)。2048 Token 上限下，原 10 道截断题中 9 道仍截断，`code-006` 独立评分未通过。

2026-10-30。承接已授权的WSL2/Docker/Python安装和用户完整Windows重新启动。启动失败及官方重装证据见[运行环境恢复](harness-runtime-setup.md)。本切片验证已有Python控制器，不执行第三方Harness、仓库脚本、云端或生产数据操作。

## 实际环境

| 项目 | 本机实际值 |
| --- | --- |
| Docker Desktop | 4.97.0，build244584，当前用户安装 |
| Docker Engine / CLI | 29.9.2，服务端linux/amd64 |
| WSL | 3.0.3，内核6.18.54.1-microsoft-standard-WSL2 |
| Docker端点 | 本地`npipe:////./pipe/dockerDesktopLinuxEngine` |
| 虚拟磁盘 | `D:\CortexOSRuntime\DockerData\disk\docker_data.vhdx`及`main\ext4.vhdx` |
| 官方镜像 | `python:3.13-slim`，实际Python3.13.18 |
| 固定Image ID / RepoDigest | `sha256:2d44738ebf4ce5f63b9d9ea871e9c7cfdc0426f7b0c97cac915fd88ded1d86bb` |
| cgroup | v2；CPU1核配额、内存256MiB、PID64 |

数据位置来自实际VHDX文件，不仅是安装参数。CLI的用户PATH登记及当前进程可见性均确认。镜像由本轮明确授权的pull下载；评分继续`--pull=never`，不接受可变tag代替实际ID。

## 复用与实际验证

原`benchmark/python.rs`的`environment`、create/inspect、`score`/`score_with`、有界命令和清理逻辑保持不变；只增加cfg(test)的显式忽略集成检查`python_live_tests.rs`。标准Core60 v2未修改。必要证据采集调用真实command，不提供模拟Docker回复；代码字符串仅通过stdin进入受限容器。实际payload只有code/function/args，标准答案仍留在控制器侧。

| 场景 | 实际结果 | 依据 |
| --- | --- | --- |
| 正确实现 | Passed，7/7 | `sum_numbers`真实输入，含空、负值和大整数 |
| 错误实现 | Failed，1/7 | 仅空输入碰巧正确，不因单个用例通过判整题成功 |
| bool冒充int | Failed，0/7 | 严格Python类型验证 |
| 修改输入 | Failed，1/7 | 非空输入被修改，空输入不被虚构为修改失败 |
| 运行时边界 | Passed，7/7 | 容器内UID/GID65534、NoNewPrivs=1、CapEff=0，cgroup实际CPU/内存/PID值；根目录写入拒绝、外部连接失败、Socket不存在，临时文件只写/tmp |
| 输出超限 | Failed，0/7 | 直接输出131072字节，真实命令限制触发output_limit |
| 内存超限 | Failed，0/7 | 分配512MiB被256MiB容器限制，移除前实际State含OOMKilled |
| 单断言超时 | Failed，0/7 | 无限循环分别触发5秒wall_timeout，整题约39.5秒 |
| 取消 | Cancelled | 在实际start后发取消，约2.65秒返回并清理；不计为普通错误答案 |
| 整题上限 | Execution Error | 合成32断言循环，约60.217秒触发case_wall_timeout，已记录11项，尚未运行的断言未伪造结果 |

每个实际创建的容器都由控制器执行force/volumes清理；逐容器移除状态及不存在检查通过，最终按cortexos-bench名称筛选没有遗留。实际inspect确认none网络、ReadonlyRootfs、非特权、无宿主绑定/Socket、cap-drop ALL及no-new-privileges；/tmp为容量受限tmpfs。日志、完整配置和逐断言结果见`artifacts/harness-docker-live/`。这些结果验证本机既定策略，不代表仓库级Agent运行器已经实现。

运行时边界场景是专用的受控探针，Passed只表示探针内检查符合预期，不是一次模型作答，也不作为纯函数证明或模型成绩。

必要命令分别只执行`scoring_python_live_docker_limits_and_cleanup`和`scoring_python_live_case_deadline`两个忽略的真实集成检查；未运行无关单元。首轮证据筛选将context inspect字符串误认成容器配置，测试失败而实际正确评分已Passed；修正检查脚本后完整通过，没有修改安全参数或评分标准。准确命令和耗时见里程碑。

## 原20项的独立补验

`benchmark-python-real.spec.ts`在安全检查通过后显式启用，新建独立SQLite/Profile。以原Core60 v2数据hash、两个实际仍在本机的原model digest、原4096/1024/temp0/120秒参数及本地固定镜像，仅运行code-001至code-010各一次。不向原计划写入，不重新评分或覆盖旧阻断。

补验必须保存原runId→新runId关联及原报告SHA256前后校验；模型答错、截断、未支持和环境错误按原评分状态呈现，不能因为Docker可用就宣称模型全部答对。最终记录、统计与报告截图在实际完成后追加；当前进度不作为完成证据。

### 实际完成结果

`benchmark-python-real`专项实际通过（4.2分钟），新计划`8a933739-988e-42c1-aac8-6d22cf0a9aba`，工作区`e6709e4f-da23-4810-b8ad-015bb562818b`。20次本地请求全部完成，20项均有终态及新runId；没有新的Docker环境或评分器错误。原报告SHA256仍为`29b8f0d51f045f63492fbcb59340e24de72fe6d926bf5365e891793b7245a047`，旧20项未修改。

| 模型 | 完成调用 | 实际评分覆盖 | Passed / Failed | 需要复核 | 已评分正确率 |
| --- | --- | --- | --- | --- | --- |
| qwen3:4b-instruct | 10/10 | 10/10 | 10 / 0 | 0 | 100% |
| qwen3:4b | 10/10 | 0/10 | 0 / 0 | 10 | 不适用 |

后者10项真实保存的评分原因均为`output_truncated`；沿用原1024输出上限，没有调整预算或为了分数重新调用。它们没有进入Python断言执行，不当作Failed，也不能称为已验证代码正确。因此本次总体调用覆盖100%、评分覆盖50%，不是“20道题全部通过”。这是固定条件的Smoke补验，不是正式模型能力排行榜。后续如需增加输出预算，应独立建对照计划并保留本次结果。

实际SQLite核对只有20条benchmark用途会话，普通聊天为空、知识节点0，全局设置与开始时一致；最后按名称检查没有本切片遗留容器。新旧runId关联、每项配置/输出/逐断言、实际环境、完整报告及原生截图保存在`artifacts/benchmark-python-real/benchmark-python-real-Pyth-98815-ut-changing-historical-runs-benchmark-python-real/`的`links.json`、`details.json`、`environment.json`、`report.json`、`acceptance.json`及`python-rerun-report.png`。已实际查看报告原图。

## 下一门槛

完成环境和独立补验后，停在D2b-0验收。既有编程草案仍未做参考/无修改/错误实现对照或人工批准，发布继续关闭。D2b-1真实任务验证、D3 CLI执行与Model×Harness实验分别按原计划推进。
