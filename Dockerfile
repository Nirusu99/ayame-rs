FROM rust:1.59-slim-buster as builder

ENV SQLX_OFFLINE=true
ENV DATABASE_URL=sqlite:database/database.sqlite

RUN apt-get update -y \
        && apt-get install -y libssl-dev pkg-config libopus-dev

RUN USER=root cargo new --bin ayame-rs

WORKDIR /ayame-rs

COPY . .

RUN cargo build --release

FROM debian:buster-slim

ARG APP=/usr/src/app

ENV APP_USER=appuser

WORKDIR ${APP}

RUN apt-get update -y \
        && apt-get install -y libssl-dev pkg-config libopus-dev ffmpeg python3 python3-pip ca-certificates \
        && rm -rf /var/lib/apt/lists/*
RUN groupadd $APP_USER
RUN useradd -g $APP_USER $APP_USER

RUN USER=$APP_USER python3 -m pip install --force-reinstall https://github.com/yt-dlp/yt-dlp/archive/master.zip

COPY --from=builder /ayame-rs/target/release/ayame-rs .
COPY --from=builder /ayame-rs/config.toml .
COPY --from=builder /ayame-rs/database .

RUN mkdir database

RUN chown -R $APP_USER:$APP_USER .

USER $APP_USER

CMD ["./ayame-rs"]


