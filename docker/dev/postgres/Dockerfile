# Create a postgres image with pg_partman

FROM postgres:16.3-bookworm AS builder

RUN apt-get update \
    && apt-get install -y \
	ca-certificates \
	clang \
	curl \
	gcc \
	git \
	libssl-dev \
	make \
	pkg-config \
	postgresql-server-dev-16 \
	postgresql-16-cron

WORKDIR /scouter

RUN git clone https://github.com/pgpartman/pg_partman.git && \
    cd pg_partman && \
    make && make install && \
    cd .. && rm -rf pg_partman

RUN apt-get update && apt-get install -y --fix-missing libcurl4-openssl-dev liblz4-dev libzstd-dev autoconf

FROM postgres:16.3-bookworm
COPY --from=builder /usr/share/postgresql/16/extension /usr/share/postgresql/16/extension
COPY --from=builder /usr/lib/postgresql/16/lib /usr/lib/postgresql/16/lib