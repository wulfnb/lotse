# Disposable test tooling; never used as the application/release image.
ARG WEGA_BASE=wega:local
FROM ${WEGA_BASE}
USER root
RUN apt-get update -qq \
    && apt-get install -y --no-install-recommends kcov \
    && rm -rf /var/lib/apt/lists/*
HEALTHCHECK NONE
ENTRYPOINT []
