FROM rust:1.77-slim-bookworm AS builder

RUN apt update
RUN apt upgrade -y
# yes we do need g++ and clang
RUN apt install  g++ clang pkg-config libvips libvips-dev -y
COPY server /server
WORKDIR /server
RUN cargo build --release --bin server

FROM ubuntu:22.04 as libbuilder

FROM debian:sid-20240330-slim

RUN mkdir /app
RUN apt update
RUN apt upgrade -y
RUN apt install libvips libheif1 libheif-plugin-svtenc svt-av1 libimage-exiftool-perl ffmpeg -y

COPY --from=builder /server/target/release /app
ENTRYPOINT /app/server -c /config.toml
