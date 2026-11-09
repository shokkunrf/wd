FROM ghcr.io/shokkunrf/devcontainer-images/base:3

RUN apt-get update \
 && apt-get install -y --no-install-recommends musl-tools shellcheck shfmt \
 && rm -rf /var/lib/apt/lists/*

ARG CARGO_DENY_VERSION=0.20.2
ARG CARGO_DENY_SHA256=9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f
RUN curl -fsSL -o /tmp/cargo-deny.tar.gz "https://github.com/EmbarkStudios/cargo-deny/releases/download/${CARGO_DENY_VERSION}/cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
 && echo "${CARGO_DENY_SHA256}  /tmp/cargo-deny.tar.gz" | sha256sum -c - \
 && tar -xzf /tmp/cargo-deny.tar.gz -C /tmp \
 && install -m 0755 "/tmp/cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl/cargo-deny" /usr/local/bin/cargo-deny \
 && rm -rf /tmp/cargo-deny.tar.gz "/tmp/cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl"
