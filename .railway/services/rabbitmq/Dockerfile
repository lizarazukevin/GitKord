FROM rabbitmq:4.3.6-management-alpine

# Railway terminal explicit path to expose Erlang and RabbitMQ binaries
RUN echo 'export PATH="/opt/erlang/bin:/opt/rabbitmq/sbin:$PATH"' >> /etc/profile

COPY rabbitmq.conf /etc/rabbitmq/rabbitmq.conf
COPY definitions.json /etc/rabbitmq/definitions.json
COPY enabled_plugins /etc/rabbitmq/enabled_plugins
