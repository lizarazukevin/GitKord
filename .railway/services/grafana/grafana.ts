import { github, preserve, service, volume } from "railway/iac";

export const grafanaGraphsVolume = volume("grafana-graphs-volume", {
  alerts: { usage: { "100": {}, "80": {}, "95": {} } },
  allowOnlineResize: true,
  region: "us-east4-eqdc4a",
  sizeMB: 5000,
});

export const GrafanaGraphs = service("Grafana Graphs", {
  source: github("lizarazukevin/GitKord", {
    checkSuites: false,
    rootDirectory: "/.railway/services/grafana",
  }),
  healthcheck: "/api/health",
  replicas: { "us-east4-eqdc4a": 1 },
  networking: { privateNetworkEndpoint: "grafana-graphs" },
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
