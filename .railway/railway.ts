import { defineRailway, project } from "railway/iac";
import { GrafanaGraphs, grafanaGraphsVolume } from "./services/grafana/grafana.ts";
import { LokiLogs, lokiLogsVolume } from "./services/loki/loki.ts";
import { RabbitMQ, rabbitmqVolume } from "./services/rabbitmq/rabbitmq.ts";
import { PrometheusMetrics, prometheusMetricsVolume } from "./services/prometheus/prometheus.ts";
import { GitKord } from "./services/gitkord.ts";
import { PostgreSQL, postgresqlVolume } from "./services/postgresql/postgres.ts";

export default defineRailway(() => {
  return project("GitKord Backend", {
    resources: [
      grafanaGraphsVolume,
      lokiLogsVolume,
      postgresqlVolume,
      prometheusMetricsVolume,
      rabbitmqVolume,
      GitKord,
      GrafanaGraphs,
      LokiLogs,
      PrometheusMetrics,
      PostgreSQL,
      RabbitMQ,
    ],
  });
});
