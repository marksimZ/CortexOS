# D2b-1：真实对照验证与人工发布

实施状态：本切片已完成，实际交付与剩余限制见[harness-fit-d2b-validation.md](harness-fit-d2b-validation.md)，命令及证据见里程碑。停止等待验收，不进入下一切片。

2026-10-30，D2b-0及草案准备已获用户继续授权。先完成本切片并停止验收，不进入D3 Coding Harness或仓库构建执行。

## 实施范围与判定

从固定草案修订读取代码、评分合同和来源，允许选择本机已有官方Python3.13镜像，绑定实际Image ID与Docker版本；不拉镜像、不调用模型。分别使用原Python控制器验证参考实现、原代码、错误实现，保存逐用例、真实时间、环境、实现/草案hash及容器清理结果。正常完成不等于对照通过。

准入规则：参考实现全部通过；原代码至少一个target用例失败、所有regression用例通过；错误实现至少一个正常返回的用例未通过（避免把语法错误当有效错误对照）；三组均完成全部断言，实际容器均清理。环境/评分错误、取消、中断或缺少结果均拒绝发布。目标/边界/回归分类和故障语义仍须人工审核。

每次验证单独追加计划和结果，历史草案保持原样。验证绑定当前revision/hash，草案修改后需新验证。人工批准须绑定成功验证ID/hash、最新草案修订及审核人；发布追加不可变任务集版本，保留来源、用例、实际对照及环境快照。Harness任务执行状态仍Not Run，不将参考验证当成模型或Harness成绩。

## 文件与存储

- `benchmark/harness/programming/validation.rs`：复用generation_jobs与generated_cases，以harness_programming_validation隔离计划及逐组结果；programming_drafts保留原格式。发布复用datasets/reviews，独立purpose=harness_programming_tasks，不加入普通benchmark_cases。
- `benchmark/python.rs`：增加本地Python镜像列举、环境快照及带审计的评分/清理窄接口，沿用原受限create/inspect/score/cleanup，不引入第二套Runner或通用命令入口。
- `db.rs`：恢复时仅把本用途validating标为interrupted，不自动重跑。创建容器前保留名字/所有者记录，正常结束清理；意外中断后允许清理本次验证拥有的容器，拒绝清理其他任务。
- 业务命令、精确Tauri权限及前端`ProgrammingValidation.tsx`接入现有草案，按需显示环境、验证历史/详情、停止、审核及发布历史。沿用Soft Glass Tokens和现有AppDialog/SelectMenu/FitStatus，不重排全局页面。

schema仍11，不改旧迁移、模型Provider、生产知识或图谱。必要的用途/修订/伪造结果/不可变发布/中断恢复检查保留可运行验证，其余先逻辑审阅。

## 验收

实际Rust/TypeScript检查、调试构建及独立WebView2+真实Docker验证：正确草案、错误参考/错误对照、取消/恢复、修订失效、来源/历史、审核发布不可变、普通题库/聊天隔离，以及三种视口浅深主题。只运行必要专项，不执行项目仓库脚本或付费云端，不改变旧20项补验。

已打开的用户窗口若仍占用程序，只在需要更新时请求保存关闭；未经用户同意不强制关闭。实际命令、失败与剩余限制写入milestone-log；完成后供用户验收。
