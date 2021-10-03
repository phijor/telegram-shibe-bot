# Adapted from:
# * https://kerkour.com/blog/deploy-rust-on-heroku-with-docker/
# * https://github.com/ravenexp/crates-io-proxy/blob/master/Dockerfile
# * https://crates.io/crates/cargo-build-dependencies

####################################################################################################
## Builder
####################################################################################################
FROM docker.io/library/rust:alpine AS builder

# Create appuser
ENV USER=tgbot
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

# Set up musl dev environment
RUN rustup target add x86_64-unknown-linux-musl
RUN apk add musl-dev
RUN update-ca-certificates

# Install cargo-build-dependencies to cache build artifacts of all dependencies.
RUN cargo install cargo-build-dependencies

# Create skeleton cargo project
RUN cd /tmp && USER=root cargo new --bin telegram-shibe-bot

WORKDIR /tmp/telegram-shibe-bot

# Compile all dependencies in their own layer
COPY Cargo.toml Cargo.lock ./
RUN cargo build-dependencies --target=x86_64-unknown-linux-musl --release

# Compile app using cached dependencies
COPY ./src ./src
RUN cargo build --target=x86_64-unknown-linux-musl --release

####################################################################################################
## Final image
####################################################################################################
FROM docker.io/library/alpine:latest

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

# Copy our build
COPY --from=builder \
    /tmp/telegram-shibe-bot/target/x86_64-unknown-linux-musl/release/telegram-shibe-bot \
    /usr/local/bin/

# Use an unprivileged user.
USER tgbot:tgbot

CMD ["/usr/local/bin/telegram-shibe-bot"]
