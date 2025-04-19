FROM debian:bullseye-slim as certs
RUN apt-get update && apt-get install -y ca-certificates && update-ca-certificates

FROM gcr.io/distroless/cc-debian11
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

ARG SCOUTER_SERVER_BINARY

COPY ${SCOUTER_SERVER_BINARY} /scouter-server
USER nonroot:nonroot
ENTRYPOINT ["/scouter-server"]