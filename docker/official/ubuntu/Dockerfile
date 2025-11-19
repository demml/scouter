# Start with the base image
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=en_US.UTF-8
ENV LANGUAGE=en_US.UTF-8
ENV PROJECT_HOME=/app

ARG USER_NAME=scouter_user

RUN apt-get update --no-install-recommends \
  && apt-get install --no-install-recommends --yes \
    ca-certificates tzdata curl **tini** \
  && groupadd ${USER_NAME} \
  && useradd --system --no-create-home --gid ${USER_NAME} ${USER_NAME} \
  && mkdir -p ${PROJECT_HOME} \
  && chown -R ${USER_NAME}:${USER_NAME} ${PROJECT_HOME} \
  \
  && rm -rf /var/lib/apt/lists/* \
  && apt-get autoremove -y \
  && apt-get clean

# Set the working directory
WORKDIR ${PROJECT_HOME}


ARG SCOUTER_SERVER_BINARY

COPY ${SCOUTER_SERVER_BINARY} /usr/local/bin/scouter-server

RUN chown ${USER_NAME}:${USER_NAME} /usr/local/bin/scouter-server

USER ${USER_NAME}

ENTRYPOINT ["/usr/bin/tini", "--"]

CMD ["/usr/local/bin/scouter-server"]