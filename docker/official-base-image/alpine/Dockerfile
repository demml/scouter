FROM rust:1.83.0 AS builder

ARG PYO3_CROSS_LIB_DIR=/usr/lib
ARG PYO3_CROSS=1
ARG FEATURE

ENV PYO3_CROSS_LIB_DIR=$PYO3_CROSS_LIB_DIR
ENV PYO3_CROSS=$PYO3_CROSS

RUN apt update && apt install -y cmake && \
    update-ca-certificates

WORKDIR /app
COPY kafka .

RUN RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-gnu -p scouter-server --features $FEATURE
RUN #RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-gnu -p scouter-server --features kafka


FROM alpine
COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/scouter-server /scouter-server
CMD ["/scouter-server"]