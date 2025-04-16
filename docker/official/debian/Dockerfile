FROM debian:stable-slim

ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=en_US.UTF-8
ENV LANGUAGE=en_US.UTF-8

RUN apt-get update --no-install-recommends \
  && apt-get install --no-install-recommends --yes \
  ca-certificates tzdata curl \
  && rm -rf /var/lib/apt/lists/* \
  && apt-get autoremove \
  && apt-get clean

ARG SCOUTER_SERVER_BINARY

COPY ${SCOUTER_SERVER_BINARY} /scouter-server

RUN chmod +x /scouter-server

CMD ["/scouter-server"]