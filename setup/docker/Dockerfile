# required on all stages..e.g. CAs and timezones should never be stale
ARG SYS_PACKAGES="ca-certificates tzdata"
# Any extra packages to install can go here
ARG EXTRA_SYS_PACKAGES="\
  curl \
  "

FROM ubuntu:22.04  AS builder

ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=en_US.UTF-8
ENV LANGUAGE=en_US.UTF-8
ENV PROJECT_HOME=scouter


ARG SCOUTER_SERVER_VERSION
ARG BUILD_TARGET
ARG SYS_PACKAGES
ARG EXTRA_SYS_PACKAGES

RUN apt-get update --no-install-recommends \
  && apt-get install --no-install-recommends --yes \
  ${SYS_PACKAGES} \
  ${EXTRA_SYS_PACKAGES} \
  && rm -rf /var/lib/apt/lists/* \
  && apt-get autoremove \
  && apt-get clean
  
WORKDIR ${PROJECT_HOME}
# Download x86_64-unknown-linux-gnu.tar.gz from latest github release for demml/scouter-server
RUN  curl -sL -o scouter.tar.gz https://github.com/demml/scouter-server/releases/download/${SCOUTER_SERVER_VERSION}/scouter-server-${BUILD_TARGET}.tar.gz \
  && tar -xzf scouter.tar.gz \
  && rm scouter.tar.gz

# Copy the scouter-server binary to /usr/local/bin from target/release
FROM builder AS prod

ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=en_US.UTF-8
ENV LANGUAGE=en_US.UTF-8
ENV PROJECT_HOME=scouter

ARG SYS_PACKAGES
ARG EXTRA_SYS_PACKAGES
ARG BUILD_TARGET

RUN apt-get update --no-install-recommends \
  && apt-get install --no-install-recommends --yes \
  ${SYS_PACKAGES} \
  ${EXTRA_SYS_PACKAGES} \
  && rm -rf /var/lib/apt/lists/* \
  && apt-get autoremove \
  && apt-get clean

ENV PROJECT_HOME=scouter

WORKDIR ${PROJECT_HOME}

COPY --from=builder "${PROJECT_HOME}/target/${BUILD_TARGET}/release/scouter-server" .

CMD ["./scouter-server"]
