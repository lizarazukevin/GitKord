ARG VERSION=v3.2.1
FROM prom/prometheus:${VERSION}
COPY prom.yml /etc/prometheus/prom.yml
CMD ["--config.file=/etc/prometheus/prom.yml", "--storage.tsdb.path=/prometheus"]