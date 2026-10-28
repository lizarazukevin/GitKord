// This script uses GitHub's Octokit SDK to make API requests.
// For more information, see https://docs.github.com/en/rest/guides/scripting-with-the-rest-api-and-javascript
import { App } from "octokit";

const LOOKBACK_MS = 30 * 60 * 1000;
const API_VERSION = "2026-03-10";

const { APP_ID, PRIVATE_KEY } = process.env;

if (!APP_ID || !PRIVATE_KEY) {
  throw new Error("APP_ID and PRIVATE_KEY are required");
}

const app = new App({
  appId: APP_ID,
  privateKey: PRIVATE_KEY,
});

const octokit = app.octokit;

async function main() {
  const cutoff = Date.now() - LOOKBACK_MS;
  const deliveriesByGuid = new Map();

  const iterator = octokit.paginate.iterator("GET /app/hook/deliveries", {
    per_page: 100,
    headers: {
      "X-GitHub-Api-Version": API_VERSION,
    },
  });

  // Fetch all attempts within the rolling 30-minute window
  for await (const { data } of iterator) {
    if (data.length === 0) break;

    let reachedCutoff = false;

    for (const delivery of data) {
      if (new Date(delivery.delivered_at).getTime() <= cutoff) {
        reachedCutoff = true;
        break;
      }

      const attempts = deliveriesByGuid.get(delivery.guid);

      if (attempts) {
        attempts.push(delivery);
      } else {
        deliveriesByGuid.set(delivery.guid, [delivery]);
      }
    }

    if (reachedCutoff) break;
  }

  let retried = 0;
  let skipped = 0;

  for (const [guid, attempts] of deliveriesByGuid) {
    // Successful deliveries do not require a retry
    if (attempts.some((attempt) => attempt.status === "OK")) {
      skipped++;
      continue;
    }

    // Results are newest-first, retrying latest failed attempt
    const latest = attempts[0];

    try {
      await octokit.request("POST /app/hook/deliveries/{delivery_id}/attempts", {
        delivery_id: latest.id,
        headers: {
          "X-GitHub-Api-Version": API_VERSION,
        },
      });

      retried++;
      console.log(`Redelivery requested: ${guid}`);
    } catch (error) {
      console.error(
        `Redelivery failed for ${guid}:`,
        error.response?.data?.message ?? error.message,
      );
      throw error;
    }
  }

  console.log(
    `Finished: ${retried} redelivery requests, ` +
      `${skipped} successful deliveries skipped, ` +
      `${deliveriesByGuid.size} unique deliveries scanned.`,
  );
}

main().catch((error) => {
  console.error("Webhook recovery failed:", error.message);
  process.exitCode = 1;
});
