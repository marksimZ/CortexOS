# CortexOS contributor instructions

- 每个开发切片完成后，向用户列出当前阶段已完成、剩余步骤和实际阻断项。

## 本地模型操作先弹窗确认

- 开发或验证中，实际发起本地模型推理、向量生成、模型评测，或执行可能加载模型并占用显存的操作前，先用交互提问弹窗告知用户实际模型、用途和调用范围，等待明确确认后执行。
- “继续开发”不自动授权新的模型调用。一次确认只覆盖弹窗说明的模型与有界批次；改变范围时重新确认。历史安装、下载或运行许可不替代本次调用确认。
- 只读连接状态、模型列表和元数据查询可直接进行；会触发推理或加载模型的“连接验证”也须先确认。用户已反馈过显存不足导致意外退出，保留此项操作门槛。

Read `development.md`, `docs/development-standards.md` and `docs/implementation-plan.md` before implementing a milestone. Actual completion and commands belong in `docs/milestone-log.md`.

- Deliver the current phase as a working, verified slice. Stop at its documented acceptance gate.
- Run commands from this independent repository; confirm `git rev-parse --show-toplevel` before staging or pushing.
- Use the repository's configured credential manager for normal pushes. Keep `Gittoken.txt` ignored and out of output, commits and URLs.
- Use npm in `apps/desktop`, commit its lockfile and Cargo.lock. `rust-toolchain.toml` selects MSVC for this project without changing the user's global default.
- Keep UI content honest: label placeholders, retain errors and drafts, and record only actual execution events.
- Build task-oriented layouts with lists, forms, timelines and a focused canvas; use the existing semantic CSS tokens.
- Add privileged Tauri capabilities only for an implemented, documented need. Frontend code does not read files, databases or model services directly.
- Record commands actually executed and distinguish browser checks, native WebView2 checks and unverified manual checks.
- UI changes follow `design.md`, based on the user-supplied Full Prototype HTML. This replaces V3's visual direction for chat, knowledge, graph, settings and dialogs. Use the unified styles, preserve every existing operation and verify actual screenshots; graph camera controls remain interruptible.
