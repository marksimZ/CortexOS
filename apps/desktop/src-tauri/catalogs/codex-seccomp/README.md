# Codex 专用 seccomp 策略 1

用途仅为用户确认的 Codex 隔离检查，不改变通用 Python 评分器或人工任务副本。实际执行仍需外层无网络/无宿主挂载/非root/只读根目录/资源限制，以及 Codex 内层文件系统与网络保护。

上游：Moby profiles，Apache-2.0，许可在本目录LICENSE。

- 固定来源：https://github.com/moby/profiles/blob/4b8214fc4971e06172938a1f1554ccff6647ee80/seccomp/default.json
- 原文件SHA-256：6416b47770785a41ac59073cdc77d9fe98517df2799dc83ef207e622de3053f6。
- 修改：保留全部上游规则与默认拒绝，追加受限unshare、要求NEWUSER且禁止NEWCGROUP的amd64 clone，以及mount/umount2/pivot_root/chroot。没有增加capability、setns或放开clone3。
- profile-v1.json SHA-256：46a3ec985105a85a8d6bb5d21974a5dd5093f176411d766f8acf51bf5f2d9f13。
- 修改与实测日期：2026-10-31。来源是官方快照，不声称等同于安装引擎的内置过滤器。核对与风险见docs/harness-fit-d3-sandbox-plan.md。

脚本scripts/harness/prepare_codex_seccomp.py可从已固定的原文件生成同一候选。更改策略须新增版本/hash并重新审查和取得批准，旧运行记录保持原样。
