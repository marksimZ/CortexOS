ARG BASE
FROM ${BASE}
COPY wheels /opt/wheels
COPY requirements.lock /opt/requirements.lock
RUN python -m pip install --no-index --no-cache-dir --only-binary=:all: --require-hashes --find-links=/opt/wheels -r /opt/requirements.lock && python -m pip check
ENV HOME=/home/cortexos PYTHONDONTWRITEBYTECODE=1 AIDER_ANALYTICS=false AIDER_CHECK_UPDATE=false AIDER_SHOW_RELEASE_NOTES=false GIT_CONFIG_NOSYSTEM=1
LABEL com.cortexos.harness="aider" com.cortexos.aider.version="0.86.2" com.cortexos.runtime.policy="aider-runtime-1"
USER 65534:65534
