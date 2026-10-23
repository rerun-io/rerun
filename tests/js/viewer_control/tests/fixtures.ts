import { expect, test as base, type Page } from "@playwright/test";

/** A page whose `window.viewer` has loaded `example.rrd`, and that fails the test on a page error. */
export const test = base.extend<{ viewer: Page }>({
  viewer: async ({ page }, use) => {
    const errors: string[] = [];
    page.on("pageerror", (err) => errors.push(err.message));

    // Forward the browser's warnings and errors, so that a failure on CI shows why.
    // Not everything: the viewer logs a lot at debug level.
    page.on("console", (msg) => {
      if (msg.type() === "warning" || msg.type() === "error") {
        console.log(`[browser ${msg.type()}] ${msg.text()}`);
      }
    });

    await page.goto("/");
    // Loading the Wasm and the recording takes a while on a software rasterizer.
    // Wait for the whole recording and the views of its default blueprint, not just its first chunk.
    await expect
      .poll(
        () =>
          page.evaluate(async () => {
            if (!window.viewer?.ready) {
              return false;
            }
            const state = await window.viewer.get_state();
            return (
              state.active_store_id !== undefined &&
              0 < (state.recordings[0]?.timelines.length ?? 0) &&
              state.loading.length === 0 &&
              0 < state.views.length
            );
          }),
        { timeout: 120_000 },
      )
      .toBe(true);

    await use(page);

    expect(errors).toEqual([]);
  },
});

export { expect };

/**
 * The `code` of the `ViewerControlError` that `call` rejects with, or `"resolved"` if it doesn't reject.
 *
 * Fails if `call` rejects with anything else.
 */
export async function error_code(page: Page, call: string): Promise<string> {
  return page.evaluate(async (call) => {
    const run = new Function("viewer", "rerun", `return (async () => (${call}))();`);
    try {
      await run(window.viewer, window.rerun);
      return "resolved";
    } catch (err) {
      if (err instanceof window.rerun.ViewerControlError && err.name === "ViewerControlError") {
        return err.code;
      }
      return `not a ViewerControlError: ${err}`;
    }
  }, call);
}
