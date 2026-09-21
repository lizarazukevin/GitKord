FROM prom/prometheus:v3.14.0
COPY prom.yml /etc/prometheus/prom-config.yml
CMD ["--config.file=/etc/prometheus/prom-config.yml", "--storage.tsdb.path=/prometheus"]