import {
  defineRailway,
  github,
  group,
  postgres,
  preserve,
  project,
  service,
  volume,
} from "railway/iac";

export default defineRailway(() => {
  // ── Volumes ──────────────────────────────────────────────────────────────
  const rabbitmqVolume = volume("rabbitmq-volume", {
    alerts: { usage: { "100": {}, "80": {}, "95": {} } },
    allowOnlineResize: true,
    region: "us-east4-eqdc4a",
    sizeMB: 5000,
  });
  const postgresVolume = volume("postgres-volume-rEUz", {
    alerts: { usage: { "100": {}, "80": {}, "95": {} } },
    allowOnlineResize: true,
    region: "us-east4-eqdc4a",
    sizeMB: 5000,
  });
  const prometheusMetricsVolume = volume("prometheus-metrics-volume", {
    alerts: { usage: { "100": {}, "80": {}, "95": {} } },
    allowOnlineResize: true,
    region: "us-east4-eqdc4a",
    sizeMB: 5000,
  });
  const grafanaGraphsVolume = volume("grafana-graphs-volume", {
    alerts: { usage: { "100": {}, "80": {}, "95": {} } },
    allowOnlineResize: true,
    region: "us-east4-eqdc4a",
    sizeMB: 5000,
  });
  const lokiLogsVolume = volume("loki-logs-volume", {
    alerts: { usage: { "100": {}, "80": {}, "95": {} } },
    allowOnlineResize: true,
    region: "us-east4-eqdc4a",
    sizeMB: 5000,
  });

  // ── infra/grafana ────────────────────────────────────────────────────────
  const GrafanaGraphs = service("Grafana Graphs", {
    source: github("lizarazukevin/GitKord", {
      checkSuites: false,
      rootDirectory: "/infra/grafana",
    }),
    healthcheck: "/api/health",
    replicas: { "us-east4-eqdc4a": 1 },
    networking: { privateNetworkEndpoint: "graphana-graphs" },
    volumeMounts: { "/var/lib/grafana": grafanaGraphsVolume },
    env: {
      GF_SECURITY_ADMIN_PASSWORD: preserve(),
      GF_SECURITY_ADMIN_USER: preserve(),
      LOG_ENDPOINT: preserve(),
      METRIC_ENDPOINT: preserve(),
      PORT: preserve(),
      RAILWAY_RUN_UID: "0",
    },
  });

  // ── infra/loki ───────────────────────────────────────────────────────────
  const LokiLogs = service("Loki Logs", {
    source: github("lizarazukevin/GitKord", {
      checkSuites: false,
      rootDirectory: "/infra/loki",
    }),
    healthcheck: "/ready",
    replicas: { "us-east4-eqdc4a": 1 },
    networking: { privateNetworkEndpoint: "loki" },
    volumeMounts: { "/loki": lokiLogsVolume },
    env: { PORT: preserve(), RAILWAY_RUN_UID: "0" },
  });

  // ── infra/rabbitmq ───────────────────────────────────────────────────────
  const RabbitMq = service("RabbitMq", {
    source: github("lizarazukevin/GitKord", {
      checkSuites: false,
      rootDirectory: "/infra/rabbitmq",
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

  // ── infra/prometheus ─────────────────────────────────────────────────────
  const PrometheusMetrics = service("Prometheus Metrics", {
    source: github("lizarazukevin/GitKord", {
      checkSuites: false,
      rootDirectory: "/infra/prometheus",
    }),
    healthcheck: "/-/healthy",
    replicas: { "us-east4-eqdc4a": 1 },
    networking: { privateNetworkEndpoint: "prometheus" },
    volumeMounts: { "/prometheus": prometheusMetricsVolume },
    env: { PORT: preserve(), RAILWAY_RUN_UID: "0" },
  });

  // ── GitKord ──────────────────────────────────────────────────────────────
  const GitKord = service("GitKord", {
    source: github("lizarazukevin/GitKord", { checkSuites: false }),
    healthcheck: "/healthz",
    healthcheckTimeout: 300,
    replicas: { "us-east4-eqdc4a": 1 },
    networking: { privateNetworkEndpoint: "gitkord" },
    env: {
      DATABASE_URL: preserve(),
      DISCORD_TOKEN: preserve(),
      GITHUB_APP_ID: preserve(),
      GITHUB_APP_PRIVATE_KEY: preserve(),
      GITHUB_TOKEN: preserve(),
      GITHUB_WEBHOOK_SECRET: preserve(),
      LOG_ENDPOINT: preserve(),
      METRIC_ENDPOINT: preserve(),
      PORT: preserve(),
      RABBITMQ_URL: preserve(),
    },
  });

  // ── Postgres ─────────────────────────────────────────────────────────────
  const Postgres = postgres("Postgres", { region: "us-east4-eqdc4a" });
  Postgres.networking = {
    privateNetworkEndpoint: "postgres",
    tcpProxies: { "5432": {} },
  };
  const PostgreSQL = group("PostgreSQL", [Postgres]);

  return project("GitKord Backend", {
    resources: [
      GrafanaGraphs,
      LokiLogs,
      RabbitMq,
      PrometheusMetrics,
      GitKord,
      rabbitmqVolume,
      postgresVolume,
      prometheusMetricsVolume,
      grafanaGraphsVolume,
      lokiLogsVolume,
      PostgreSQL,
    ],
  });
});
