import { group, postgres, volume } from "railway/iac";

export const postgresqlVolume = volume("postgresql-volume", {
  alerts: { usage: { "100": {}, "80": {}, "95": {} } },
  allowOnlineResize: true,
  region: "us-east4-eqdc4a",
  sizeMB: 5000,
});

export const Postgres = postgres("PostgreSQL", { region: "us-east4-eqdc4a" });
Postgres.networking = {
  privateNetworkEndpoint: "postgresql",
  tcpProxies: { "5432": {} },
};

export const PostgreSQL = group("PostgreSQL", [Postgres]);
