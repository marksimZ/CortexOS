# Fixed local base. Build with --network=none --pull=false after archive verification.
FROM python:3.13-slim
COPY --chmod=0555 bin/codex /opt/codex/bin/codex
COPY --chmod=0555 codex-resources/bwrap /opt/codex/codex-resources/bwrap
COPY --chmod=0555 codex-path/rg /opt/codex/codex-path/rg
COPY --chmod=0444 codex-package.json /opt/codex/codex-package.json
ENV PATH=/opt/codex/bin:/opt/codex/codex-path:/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin
ENV HOME=/home/cortexos CODEX_HOME=/home/cortexos/.codex
LABEL com.cortexos.harness="codex" com.cortexos.codex.version="0.166.0" com.cortexos.runtime.policy="codex-runtime-1"
# Runtime still uses the existing non-root/read-only/network-none container policy.
USER 65534:65534
