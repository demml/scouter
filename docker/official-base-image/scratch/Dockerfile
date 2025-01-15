FROM rust:1.83.0 AS builder

RUN apt update && update-ca-certificates


FROM scratch

ARG SCOUTER_SERVER_BINARY

COPY ${SCOUTER_SERVER_BINARY} /scouter-server

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

CMD ["/scouter-server"]