# Disposable test tooling; never used as the application/release image.
ARG LOTSE_BASE=servo-browser:local
FROM ${LOTSE_BASE}
USER root
RUN apt-get update -qq \
    && apt-get install -y --no-install-recommends kcov \
    && rm -rf /var/lib/apt/lists/*
HEALTHCHECK NONE
ENTRYPOINT []
