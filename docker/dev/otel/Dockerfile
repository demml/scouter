FROM otel/opentelemetry-collector-contrib:0.139.0

COPY --chown=otel:otel docker/dev/otel/otel-collector-config.yaml /etc/otelcol-contrib/config.yaml

EXPOSE 4317 4318