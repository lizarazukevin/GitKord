import { github, preserve, service, volume } from "railway/iac";

export const lokiLogsVolume = volume("loki-logs-volume", {
  alerts: { usage: { "100": {}, "80": {}, "95": {} } },
  allowOnlineResize: true,
  region: "us-east4-eqdc4a",
  sizeMB: 5000,
});

export const LokiLogs = service("Loki Logs", {
  source: github("lizarazukevin/GitKord", {
    checkSuites: false,
    rootDirectory: ".railway/services/loki",
  }),
  healthcheck: "/ready",
  replicas: { "us-east4-eqdc4a": 1 },
  networking: { privateNetworkEndpoint: "loki" },
  volumeMounts: { "/loki": lokiLogsVolume },
  env: { PORT: preserve(), RAILWAY_RUN_UID: "0" },
});
