import { github, preserve, service } from "railway/iac";

export const GitKordFrontend = service("GitKordFrontend", {
  source: github("lizarazukevin/GitKordFrontend", { checkSuites: false }),
  replicas: { "us-east4-eqdc4a": 1 },
  domains: ["gitkord.com", "www.gitkord.com"],
  networking: { privateNetworkEndpoint: "gitkordfrontend" },
  env: {
    VITE_GITKORD_BOT_INVITE_LINK: preserve(),
    VITE_GITKORD_GITHUB_APP_LINK: preserve(),
    VITE_GITKORD_GITHUB_LINK: preserve(),
    VITE_NEWSLETTER_SIGNUP_LINK: preserve(),
    VITE_SUPPORT_MAILTO: preserve(),
  },
});
