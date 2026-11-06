# Rustの道具は、shokkunrf/autopilot-sandboxのworkspaces/autopilot/.devcontainer/Dockerfile、app/rust-toolchain.toml、.github/workflows/ci.ymlと同じ版にする
FROM ghcr.io/shokkunrf/devcontainer-images/base:4.0.0

ARG MUSL_TOOLS_VERSION=1.2.5-3.1~deb13u1
RUN apt-get update \
 && apt-get install -y --no-install-recommends "musl-tools=${MUSL_TOOLS_VERSION}" \
 && rm -rf /var/lib/apt/lists/*

# toolchainはrootの所有にし、コンテナの中で入れ替えさせない
ARG RUSTUP_VERSION=1.29.2
ARG RUSTUP_SHA256=5be191e8c70a5007074254cb74d78dbfdd25cd5a61c27de7b8e5410122ec24f4
ARG RUST_VERSION=1.99.1
ENV RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl -fsSL -o /tmp/rustup-init "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init" \
 && echo "${RUSTUP_SHA256}  /tmp/rustup-init" | sha256sum -c - \
 && chmod +x /tmp/rustup-init \
 && CARGO_HOME=/usr/local/cargo /tmp/rustup-init -y --no-modify-path --profile minimal \
      --default-toolchain "${RUST_VERSION}" --component rustfmt,clippy --target x86_64-unknown-linux-musl \
 && rm /tmp/rustup-init

ARG CARGO_DENY_VERSION=0.20.2
ARG CARGO_DENY_SHA256=9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f
RUN curl -fsSL -o /tmp/cargo-deny.tar.gz "https://github.com/EmbarkStudios/cargo-deny/releases/download/${CARGO_DENY_VERSION}/cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
 && echo "${CARGO_DENY_SHA256}  /tmp/cargo-deny.tar.gz" | sha256sum -c - \
 && tar -xzf /tmp/cargo-deny.tar.gz -C /tmp \
 && install -m 0755 "/tmp/cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl/cargo-deny" /usr/local/cargo/bin/cargo-deny \
 && rm -rf /tmp/cargo-deny.tar.gz "/tmp/cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl"
