# The runner and the four toolchains it shells out to.
#
# One image rather than one per language: a handler is a few hundred bytes and
# the toolchains are the image, so splitting them would multiply cold starts
# without shrinking anything that matters.

FROM rust:1.83-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo build --release -p valang-runner

FROM debian:bookworm-slim

# Go and Python from the distribution; Node from NodeSource, because type
# stripping needs 22 or later and bookworm ships 18.
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl golang-go python3 \
  && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
  && apt-get install -y --no-install-recommends nodejs \
  && rm -rf /var/lib/apt/lists/*

# Rust, for handlers written in it. The registry cache is vendored at build
# time so a handler compiles with the network off.
COPY --from=build /usr/local/cargo /usr/local/cargo
COPY --from=build /usr/local/rustup /usr/local/rustup
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH

COPY --from=build /src/target/release/valang-runner /usr/local/bin/valang-runner

ENV PORT=8787
EXPOSE 8787
CMD ["valang-runner"]
