import { github, service } from "railway/iac";

export const GitKordFrontend = service("GitKordFrontend", {
  source: github("lizarazukevin/GitKordFrontend", { checkSuites: false }),
  replicas: { "us-east4-eqdc4a": 1 },
  domains: ["gitkord.com", "www.gitkord.com"],
  networking: { privateNetworkEndpoint: "gitkordfrontend" },
});
