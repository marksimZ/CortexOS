# The preparation script verifies the existing tag's exact Image ID before build.
FROM python:3.13-slim
COPY --chmod=0555 claude /opt/claude/bin/claude
ENV PATH=/opt/claude/bin:/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin
ENV HOME=/home/cortexos CLAUDE_CONFIG_DIR=/home/cortexos/.claude DISABLE_AUTOUPDATER=1 DISABLE_UPDATES=1 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
LABEL com.cortexos.harness="claude-code" com.cortexos.claude.version="2.1.309" com.cortexos.claude.sha256="4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658" com.cortexos.runtime.policy="claude-runtime-1"
USER 65534:65534
