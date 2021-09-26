# Adapted from: https://kerkour.com/blog/deploy-rust-on-heroku-with-docker/

####################################################################################################
## Builder
####################################################################################################
FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev
RUN update-ca-certificates

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


WORKDIR /tgbot

COPY ./ .

RUN cargo build --target x86_64-unknown-linux-musl --release

####################################################################################################
## Final image
####################################################################################################
FROM alpine:latest

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /tgbot

# Copy our build
COPY --from=builder /tgbot/target/x86_64-unknown-linux-musl/release/telegram-shibe-bot ./

# Use an unprivileged user.
USER tgbot:tgbot

CMD ["/tgbot/telegram-shibe-bot"]
