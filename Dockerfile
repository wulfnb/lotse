# syntax=docker/dockerfile:1.7
ARG RUST_IMAGE=rust:1.98.0-slim-trixie
FROM ${RUST_IMAGE} AS builder
ARG TARGETARCH
ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_INCREMENTAL=0 \
    CARGO_PROFILE_RELEASE_DEBUG=0 \
    CARGO_PROFILE_RELEASE_OPT_LEVEL=2 \
    CARGO_PROFILE_RELEASE_LTO=false \
    RUSTFLAGS="-C link-arg=-fuse-ld=lld"
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential clang libclang-dev lld cmake pkg-config python3 git curl ca-certificates \
    autoconf automake libtool m4 gperf libdbus-1-dev libegl1-mesa-dev \
    libfreetype6-dev libfontconfig1-dev libgles2-mesa-dev libglib2.0-dev \
    libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev \
    libgstrtspserver-1.0-dev libges-1.0-dev libharfbuzz-dev liblzma-dev \
    libudev-dev libunwind-dev libx11-dev libxcb-render0-dev libxcb-shape0-dev \
    libxcb-xfixes0-dev libxkbcommon-dev libxkbcommon-x11-dev libxmu-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY vendor ./vendor
COPY src ./src
COPY assets/icons/browser-icon-256.png ./assets/icons/browser-icon-256.png
ARG BUILD_PROFILE=release
ARG BUILD_JOBS=4
ARG BUILD_FEATURES=desktop
RUN --mount=type=cache,id=servo-registry-${TARGETARCH},target=/usr/local/cargo/registry \
    --mount=type=cache,id=servo-git-${TARGETARCH},target=/usr/local/cargo/git \
    --mount=type=cache,id=servo-target-${TARGETARCH}-${BUILD_PROFILE},target=/src/target \
    touch src/main.rs assets/icons/browser-icon-256.png && \
    cargo build --locked --features "${BUILD_FEATURES}" --profile "${BUILD_PROFILE}" --jobs "${BUILD_JOBS}" && \
    mkdir -p /out && \
    if [ "${BUILD_PROFILE}" = dev ]; then cp target/debug/servo-browser /out/; \
    else cp "target/${BUILD_PROFILE}/servo-browser" /out/; fi && \
    strip /out/servo-browser

FROM debian:trixie-slim AS desktop
ENV DEBIAN_FRONTEND=noninteractive
ENV BROWSER_METRICS_SCOPE=container
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl tini gosu supervisor xvfb x11vnc x11-utils openbox \
    novnc websockify nginx-light openssl apache2-utils python3-cryptography \
    libegl1 libgl1 libgl1-mesa-dri libgles2 libgbm1 libvulkan1 \
    libx11-6 libxcursor1 libxi6 libxrandr2 libxinerama1 libxkbcommon0 libxkbcommon-x11-0 \
    libdbus-1-3 libudev1 libfontconfig1 libfreetype6 libharfbuzz0b libunwind8 \
    gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
    gstreamer1.0-libav gstreamer1.0-nice libgstreamer-gl1.0-0 libgstrtspserver-1.0-0 libges-1.0-0 \
    fonts-dejavu-core fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji \
    && rm -rf /var/lib/apt/lists/* /etc/nginx/sites-enabled/default \
    && groupadd --gid 1000 browser && useradd --uid 1000 --gid browser --home-dir /config --shell /bin/sh browser \
    && mkdir -p /opt/servo/viewer /config /run/servo \
    && chown browser:browser /config /run/servo
COPY docker/viewer/ /opt/servo/viewer/
COPY docker/auth-service.py /opt/servo/auth-service.py
COPY docker/nginx.conf /etc/nginx/nginx.conf
COPY docker/supervisord.conf /etc/servo-supervisord.conf
COPY docker/openbox.xml /etc/servo-openbox.xml
COPY --chmod=755 docker/entrypoint.sh docker/start-browser.sh /usr/local/bin/
ENV DISPLAY=:99 \
    XDG_RUNTIME_DIR=/run/servo \
    XDG_CONFIG_HOME=/config/.config \
    XDG_CACHE_HOME=/config/.cache \
    XDG_DATA_HOME=/config/.local/share \
    WINIT_UNIX_BACKEND=x11 \
    BROWSER_RESOLUTION=1440x900 \
    BROWSER_START_URL=about:blank \
    BROWSER_DOWNLOAD_DIR=/config/Downloads
WORKDIR /config
EXPOSE 7443
VOLUME ["/config"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=45s --retries=3 \
    CMD /usr/local/bin/entrypoint.sh healthcheck
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]

FROM desktop AS runtime
LABEL org.opencontainers.image.title="Lotse Browser" \
      org.opencontainers.image.description="Native Rust browser powered by Servo, with a secure web viewer"
COPY --from=builder /out/servo-browser /usr/local/bin/servo-browser
