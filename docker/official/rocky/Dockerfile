# Start with the minimal Rocky Linux base image
FROM rockylinux:9-minimal

ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=en_US.UTF-8
ENV LANGUAGE=en_US.UTF-8
ENV PROJECT_HOME=/app
ARG USER_NAME=scouter_user


RUN microdnf update -y \
  && microdnf install -y \
    shadow-utils \
    tini \
    ca-certificates \
  \
  && groupadd --system ${USER_NAME} \
  && useradd --system --gid ${USER_NAME} --no-create-home ${USER_NAME} \
  \
  && mkdir -p ${PROJECT_HOME} \
  && chown -R ${USER_NAME}:${USER_NAME} ${PROJECT_HOME} \
  \
  && microdnf clean all \
  && rm -rf /var/cache/yum

WORKDIR ${PROJECT_HOME}

ARG SCOUTER_SERVER_BINARY


COPY ${SCOUTER_SERVER_BINARY} /usr/local/bin/scouter-server

RUN chown ${USER_NAME}:${USER_NAME} /usr/local/bin/scouter-server

USER ${USER_NAME}

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/scouter-server"]