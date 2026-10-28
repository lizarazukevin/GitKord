#!/bin/bash
set -euo pipefail

# Generate the RabbitMQ definitions file from environment variables so the
# broker user's credentials are configurable per deployment (via
# RABBITMQ_DEFAULT_USER / RABBITMQ_DEFAULT_PASS) instead of being baked into
# the image.
#
# Why this is needed:
#   The official `rabbitmq` docker-entrypoint.sh only seeds a default user
#   from those env vars when NO definitions file is loaded. Since this image
#   loads /etc/rabbitmq/definitions.json (to create the queues/exchanges the
#   app depends on), that default seeding is skipped and the definitions file
#   is used verbatim. We therefore regenerate the user plus its vhost ACL here,
#   before the broker starts.
#
# Kept as a small separate entrypoint (instead of patching upstream) so the
# stock docker-entrypoint.sh stays intact and this wrapper stays reviewable.
json_escape() {
  local s="$1"
  # Escape the JSON metacharacters that must be escaped inside a string.
  s=${s//\\/\\\\}
  s=${s//\"/\\\"}
  # Keep the JSON string on a single line.
  s=${s//$'\n'/\\n}
  s=${s//$'\r'/\\r}
  s=${s//$'\t'/\\t}
  printf '%s' "$s"
}

if [ -n "${RABBITMQ_DEFAULT_USER:-}" ] && [ -n "${RABBITMQ_DEFAULT_PASS:-}" ]; then
  escaped_pass="$(json_escape "$RABBITMQ_DEFAULT_PASS")"
  escaped_user="$(json_escape "$RABBITMQ_DEFAULT_USER")"

  # The checked-in definitions.json is a topology-only template:  { <topology> }
  # Rebuild the document, emitting users / vhosts / permissions from the
  # environment (so the configured user is always granted full access on "/"),
  # then splice in the template's exchanges / queues / bindings. `printf %s`
  # is used (not awk -v) so backslash escapes in the password are preserved.
  {
    printf '{\n'
    printf '  "users": [\n'
    printf '    {\n'
    printf '      "name": "%s",\n' "$escaped_user"
    printf '      "password": "%s",\n' "$escaped_pass"
    printf '      "tags": ["administrator"]\n'
    printf '    }\n'
    printf '  ],\n'
    printf '  "vhosts": [{ "name": "/" }],\n'
    printf '  "permissions": [\n'
    printf '    {\n'
    printf '      "user": "%s",\n' "$escaped_user"
    printf '      "vhost": "/",\n'
    printf '      "configure": ".*",\n'
    printf '      "write": ".*",\n'
    printf '      "read": ".*"\n'
    printf '    }\n'
    printf '  ],\n'
    # Drop the template's leading "{" and trailing "}" and splice the topology
    # (exchanges, queues, bindings) in beneath the generated sections.
    sed '1d;$d' /etc/rabbitmq/definitions.json
    printf '}\n'
  } > /tmp/rabbitmq-definitions.json

  mv /tmp/rabbitmq-definitions.json /etc/rabbitmq/definitions.json
fi

# Hand off to the stock entrypoint, which performs the standard config setup
# (ports, management listener, management.load_definitions) and then switches
# to the unprivileged `rabbitmq` user to start the broker.
exec /usr/local/bin/docker-entrypoint.sh "$@"
