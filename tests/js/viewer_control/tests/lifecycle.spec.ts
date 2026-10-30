import { error_code, expect, test } from "./fixtures";

test("a started viewer is ready and has a canvas, and a stopped one refuses calls", async ({ viewer }) => {
  expect(await viewer.evaluate(() => window.viewer.ready)).toBe(true);
  expect(await viewer.evaluate(() => window.viewer.canvas instanceof HTMLCanvasElement)).toBe(true);

  await viewer.evaluate(() => window.viewer.stop());
  expect(await viewer.evaluate(() => window.viewer.ready)).toBe(false);
  expect(await viewer.evaluate(() => document.querySelector("#viewer canvas"))).toBeNull();
  expect(await error_code(viewer, "viewer.get_state()")).toBe("FAILED_PRECONDITION");

  // The same `WebViewer` can start again.
  await viewer.evaluate(() => window.viewer.start(null, document.getElementById("viewer")!, { hide_welcome_screen: true }));
  expect(await viewer.evaluate(async () => (await window.viewer.get_state()).recordings.length)).toBe(0);
});

test("panel overrides and credentials are accepted", async ({ viewer }) => {
  await viewer.evaluate(() => {
    window.viewer.override_panel_state("blueprint", "hidden");
    window.viewer.override_panel_state("time", "collapsed");
    window.viewer.override_panel_state("selection", null);
    window.viewer.toggle_panel_overrides(false);
    window.viewer.toggle_panel_overrides(true);
    window.viewer.set_credentials("not-a-real-token", "nobody@example.com");
  });
  expect(await viewer.evaluate(() => window.viewer.ready)).toBe(true);
});

test("a failed call rejects with a ViewerControlError carrying a code", async ({ viewer }) => {
  const error = await viewer.evaluate(async () => {
    try {
      await window.viewer.set_location("not a location");
      return null;
    } catch (err) {
      return {
        instance: err instanceof window.rerun.ViewerControlError,
        name: (err as Error).name,
        code: (err as InstanceType<typeof window.rerun.ViewerControlError>).code,
        message: (err as Error).message,
      };
    }
  });
  expect(error).toMatchObject({ instance: true, name: "ViewerControlError", code: "INVALID_ARGUMENT" });
  expect(error?.message).toBeTruthy();
});
