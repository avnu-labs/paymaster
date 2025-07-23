FROM amazon/aws-otel-collector

WORKDIR /app
COPY otel-collector.yaml /etc/otel-collector-config.yaml

ENTRYPOINT ["/awscollector"]
CMD ["--config=/etc/otel-collector-config.yaml"]