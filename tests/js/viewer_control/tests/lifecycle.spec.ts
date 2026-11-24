import { errorCode, expect, test } from "./fixtures";

test("a started viewer is ready and has a canvas, and a stopped one refuses calls", async ({
  viewer,
}) => {
  expect(await viewer.evaluate(() => window.viewer.ready)).toBe(true);
  expect(await viewer.evaluate(() => window.viewer.canvas instanceof HTMLCanvasElement)).toBe(true);

  await viewer.evaluate(() => window.viewer.stop());
  expect(await viewer.evaluate(() => window.viewer.ready)).toBe(false);
  expect(await viewer.evaluate(() => document.querySelector("#viewer canvas"))).toBeNull();
  expect(await errorCode(viewer, "viewer.getState()")).toBe("FAILED_PRECONDITION");

  // The same `WebViewer` can start again.
  await viewer.evaluate(() =>
    window.viewer.start(null, document.getElementById("viewer")!, { hideWelcomeScreen: true }),
  );
  expect(
    await viewer.evaluate(async () => (await window.viewer.getState()).recordings.length),
  ).toBe(0);
});

test("panel overrides and credentials are accepted", async ({ viewer }) => {
  await viewer.evaluate(() => {
    window.viewer.overridePanelState("blueprint", "hidden");
    window.viewer.overridePanelState("time", "collapsed");
    window.viewer.overridePanelState("selection", null);
    window.viewer.togglePanelOverrides(false);
    window.viewer.togglePanelOverrides(true);
    window.viewer.setCredentials("not-a-real-token", "nobody@example.com");
  });
  expect(await viewer.evaluate(() => window.viewer.ready)).toBe(true);
});

test("a failed call rejects with a ViewerControlError carrying a code", async ({ viewer }) => {
  const error = await viewer.evaluate(async () => {
    try {
      await window.viewer.setLocation("not a location");
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
  expect(error).toMatchObject({
    instance: true,
    name: "ViewerControlError",
    code: "INVALID_ARGUMENT",
  });
  expect(error?.message).toBeTruthy();
});
