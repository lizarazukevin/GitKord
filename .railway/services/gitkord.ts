import { github, preserve, service } from "railway/iac";

export const GitKord = service("GitKord", {
  source: github("lizarazukevin/GitKord", { checkSuites: false }),
  healthcheck: "/healthz",
  healthcheckTimeout: 300,
  replicas: { "us-east4-eqdc4a": 2 },
  domains: ["api.gitkord.com"],
  networking: { privateNetworkEndpoint: "gitkord" },
  env: {
    ADMIN_TOKEN: preserve(),
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
