FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=en_US.UTF-8
ENV LANGUAGE=en_US.UTF-8
ENV PROJECT_HOME=scouter


RUN apt-get update --no-install-recommends \
  && apt-get install --no-install-recommends --yes \
  ca-certificates tzdata curl \
  && rm -rf /var/lib/apt/lists/* \
  && apt-get autoremove \
  && apt-get clean

WORKDIR /app

ARG SCOUTER_SERVER_BINARY

COPY ${SCOUTER_SERVER_BINARY} /scouter-server


CMD ["./scouter-server"]