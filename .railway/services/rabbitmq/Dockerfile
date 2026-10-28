FROM rabbitmq:4.3.6-management-alpine

# Railway terminal explicit path to expose Erlang and RabbitMQ binaries
RUN echo 'export PATH="/opt/erlang/bin:/opt/rabbitmq/sbin:$PATH"' >> /etc/profile

COPY rabbitmq.conf /etc/rabbitmq/rabbitmq.conf
# Topology-only template. rabbitmq-init.sh injects the broker user (password
# from RABBITMQ_DEFAULT_PASS) into this file at container startup, so that
# credentials never end up baked into the image.
COPY definitions.json /etc/rabbitmq/definitions.json
COPY enabled_plugins /etc/rabbitmq/enabled_plugins

# Seed the definitions file from the environment, then defer to the stock
# upstream entrypoint/docker-entrypoint.sh (which sets up ports, loads
# management.load_definitions, and starts the broker as the `rabbitmq` user).
COPY rabbitmq-init.sh /usr/local/bin/rabbitmq-init.sh
RUN chmod +x /usr/local/bin/rabbitmq-init.sh
ENTRYPOINT ["/usr/local/bin/rabbitmq-init.sh"]
CMD ["rabbitmq-server"]
