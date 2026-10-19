import { fileURLToPath } from "node:url";

import { expect, test } from "@playwright/test";

/** A short recording of the DNA example, served locally so the test doesn't need the network. */
const RRD_PATH = fileURLToPath(new URL("../../../assets/example.rrd", import.meta.url));

test("controls playback and logs viewer events", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (err) => errors.push(err.message));

  // Forward the browser's warnings and errors, so that a failure on CI shows why the viewer didn't load.
  // Not everything: the viewer logs a lot at debug level.
  page.on("console", (msg) => {
    if (msg.type() === "warning" || msg.type() === "error") {
      console.log(`[browser ${msg.type()}] ${msg.text()}`);
    }
  });
  page.on("requestfailed", (req) => console.log(`[request failed] ${req.failure()?.errorText} ${req.url()}`));

  await page.route((url) => url.pathname === "/example.rrd", (route) => route.fulfill({ path: RRD_PATH }));
  await page.goto("/?url=/example.rrd");

  const play = page.locator("#play");
  const slider = page.locator("#time");
  const timeLabel = page.locator("#time-label");
  const log = page.locator("#log");

  // Loading the Wasm and the recording takes a while on a software rasterizer.
  await expect(slider).toBeEnabled({ timeout: 120_000 });
  await expect(log).toContainText("recording_open");

  // The viewer plays the recording on its own after loading, and pauses at its end.
  await expect(play).toHaveText("Play", { timeout: 30_000 });

  const min = await slider.evaluate((el: HTMLInputElement) => el.min);
  await slider.fill(min);
  await expect(timeLabel).toHaveText(min);

  await play.click();
  await expect(play).toHaveText("Pause");
  await play.click();
  await expect(play).toHaveText("Play");
  await expect(log).toHaveText(/\nplay\npause\n$/);

  const max = await slider.evaluate((el: HTMLInputElement) => el.max);
  await slider.fill(max);
  await expect(timeLabel).toHaveText(max);

  expect(errors).toEqual([]);
});
