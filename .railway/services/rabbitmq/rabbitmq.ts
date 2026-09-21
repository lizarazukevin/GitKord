import { github, preserve, service, volume } from "railway/iac";

export const rabbitmqVolume = volume("rabbitmq-volume", {
  alerts: { usage: { "100": {}, "80": {}, "95": {} } },
  allowOnlineResize: true,
  region: "us-east4-eqdc4a",
  sizeMB: 5000,
});

export const RabbitMQ = service("RabbitMQ", {
  source: github("lizarazukevin/GitKord", {
    checkSuites: false,
    rootDirectory: "/.railway/services/rabbitmq",
  }),
  healthcheck: "/api/health/checks/ready-to-serve-clients",
  replicas: { "us-east4-eqdc4a": 1 },
  networking: { privateNetworkEndpoint: "rabbitmq" },
  volumeMounts: { "/var/lib/rabbitmq": rabbitmqVolume },
  env: {
    RABBITMQ_DEFAULT_USER: preserve(),
    RABBITMQ_DEFAULT_PASS: preserve(),
    AMQP_PORT: preserve(),
    PORT: preserve(),
    RAILWAY_RUN_UID: "0",
    RABBITMQ_NODENAME: preserve(),
  },
});
