# Reviewable dependency preparation only; run after the user's download approval.
# Parent CLI, signed release, base and original runtime remain unchanged.
# Check this local tag's exact Image ID against runtime.json before build; pull=false.
FROM cortexos-claude:2.1.309
USER 0:0
RUN printf '%s\n' \
    'Types: deb' \
    'URIs: https://deb.debian.org/debian' \
    'Suites: trixie trixie-updates' \
    'Components: main' \
    'Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg' \
    '' \
    'Types: deb' \
    'URIs: https://deb.debian.org/debian-security' \
    'Suites: trixie-security' \
    'Components: main' \
    'Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg' \
    > /etc/apt/sources.list.d/debian.sources \
    && mkdir -p /opt/claude/evidence \
    && apt-get -o Acquire::Retries=0 -o Acquire::https::Timeout=30 update \
    && apt-get -o Acquire::Retries=0 -o Acquire::https::Timeout=30 \
       --no-install-recommends --download-only -y install \
       bubblewrap=0.12.0-1~deb13u1 socat=1.8.0.3-1+deb13u1 \
    && sha256sum /var/cache/apt/archives/*.deb > /opt/claude/evidence/debs.sha256 \
    && apt-cache policy bubblewrap socat libwrap0 > /opt/claude/evidence/package-sources.txt \
    && apt-get --no-install-recommends --no-download -y install \
       bubblewrap=0.12.0-1~deb13u1 socat=1.8.0.3-1+deb13u1 \
    && dpkg-query -W > /opt/claude/evidence/packages.tsv \
    && rm -rf /var/lib/apt/lists/*
LABEL com.cortexos.claude.dependencies="debian-trixie-bwrap-socat-1"
USER 65534:65534
