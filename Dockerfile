FROM rustlang/rust:nightly as build

RUN apt-get update && \
    apt-get install -y --no-install-recommends dumb-init &&  \
    rm -rf /var/lib/apt/lists/*

# Build dependencies-only as a separate layer to speed-up repeated builds
RUN cargo install cargo-build-deps
WORKDIR /buildenv
RUN USER=root cargo init
COPY Cargo.toml ./
RUN cargo build-deps --release

# Build our actual Rust app
COPY src src
RUN cargo install --path . --root /install

# Production (release) image
FROM buildpack-deps:curl

# Use dumb-init to correctly handle signals in PID 1
COPY --from=build /usr/bin/dumb-init /usr/bin/
ENTRYPOINT ["/usr/bin/dumb-init", "--"]

WORKDIR /app
COPY --from=build /install/bin/goout-calendar /app/
COPY Rocket.toml /app/
COPY resources /app/resources

EXPOSE 80
CMD ["/app/goout-calendar"]
