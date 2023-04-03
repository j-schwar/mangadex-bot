FROM rust:1.68.2-alpine as builder

RUN apk add --no-cache musl-dev
RUN apk add --no-cache openssl-dev
WORKDIR /opt
RUN USER=root cargo new --bin mangadex-bot
WORKDIR /opt/mangadex-bot
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release
RUN rm src/*.rs
RUN rm ./target/release/deps/mangadex_bot*
ADD ./src ./src
RUN cargo build --release


FROM scratch

WORKDIR /opt/mangadex-bot
COPY --from=builder /opt/mangadex-bot/target/release/mangadex-bot .

CMD ["/opt/mangadex-bot/mangadex-bot"]
