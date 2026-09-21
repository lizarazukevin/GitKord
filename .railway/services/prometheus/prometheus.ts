import { github, preserve, service, volume } from "railway/iac";

export const prometheusMetricsVolume = volume("prometheus-metrics-volume", {
  alerts: { usage: { "100": {}, "80": {}, "95": {} } },
  allowOnlineResize: true,
  region: "us-east4-eqdc4a",
  sizeMB: 5000,
});

export const PrometheusMetrics = service("Prometheus Metrics", {
  source: github("lizarazukevin/GitKord", {
    checkSuites: false,
    rootDirectory: "/.railway/services/prometheus",
  }),
  healthcheck: "/-/healthy",
  replicas: { "us-east4-eqdc4a": 1 },
  networking: { privateNetworkEndpoint: "prometheus" },
  volumeMounts: { "/prometheus": prometheusMetricsVolume },
  env: { PORT: preserve(), RAILWAY_RUN_UID: "0" },
});
