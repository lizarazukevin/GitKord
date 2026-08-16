FROM prom/prometheus:latest
COPY prom.yml /etc/prometheus/prom.yml
CMD ["--config.file=/etc/prometheus/prom.yml", "--storage.tsdb.path=/prometheus"]