FROM rustlang/rust:nightly as build

RUN rustc --version

RUN apt-get update && \
    apt-get install -y --no-install-recommends dumb-init &&  \
    rm -rf /var/lib/apt/lists/*

# Build our Rust app
COPY Cargo.toml ./
COPY resources resources
COPY src src
COPY test_data test_data
# use --release so that compiled dependencies are shared:
RUN cargo test --release
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

ENV PORT=8080
EXPOSE $PORT
CMD ["sh", "-c", "ROCKET_PORT=$PORT /app/goout-calendar"]
