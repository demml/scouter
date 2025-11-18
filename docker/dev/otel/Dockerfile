FROM otel/opentelemetry-collector:latest

COPY --chown=otel:otel docker/dev/otel/otel-collector-config.yaml /etc/otelcol/config.yaml

EXPOSE 4317 4318

CMD ["--config=/etc/otelcol/config.yaml"]