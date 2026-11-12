import { errorCode, expect, test } from "./fixtures";

test("open names the recording it opened, and close by URL closes it", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const url = new URL("/example.rrd", location.href).href;
    const openStoreId = (await window.viewer.getState()).recordings[0].storeId;

    const opened = await window.viewer.open(url);
    const closed = await window.viewer.close(url);
    const left = (await window.viewer.getState()).recordings.length;

    return { openStoreId, opened, closed, left };
  });

  expect(result.opened.storeIds).toEqual([result.openStoreId]);
  expect(result.opened.loading).toEqual([]);
  expect(result.closed).toEqual([result.openStoreId]);
  expect(result.left).toBe(0);

  // Nothing is loaded from it anymore.
  expect(await errorCode(viewer, 'viewer.close(new URL("/example.rrd", location.href).href)')).toBe(
    "NOT_FOUND",
  );
});

test("close selects by store id, the active recording, or all of them", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const url = new URL("/example.rrd", location.href).href;
    const storeId = (await window.viewer.getState()).recordings[0].storeId;

    const byStoreId = await window.viewer.close({ storeIds: [storeId] });
    await window.viewer.open(url);
    const current = await window.viewer.close("current");
    await window.viewer.open(url);
    const all = await window.viewer.close("all");

    return { storeId, byStoreId, current, all };
  });

  expect(result.byStoreId).toEqual([result.storeId]);
  expect(result.current).toEqual([result.storeId]);
  expect(result.all).toEqual([result.storeId]);

  expect(await errorCode(viewer, 'viewer.close({ storeIds: ["Recording:nope:nope"] })')).toBe(
    "NOT_FOUND",
  );
  // A selector that is not `true` never reaches the viewer, rather than closing everything.
  expect(await errorCode(viewer, "viewer.close({ all: false })")).toMatch(
    /^not a ViewerControlError/,
  );
});

test("open rejects a recording location it cannot load", async ({ viewer }) => {
  expect(
    await errorCode(viewer, 'viewer.open("about:recording?app_id=nope&recording_id=nope")'),
  ).toBe("NOT_FOUND");
});

test("save_recording and save_blueprint stream .rrd bytes", async ({ viewer }) => {
  const magic = await viewer.evaluate(async () => {
    const head = async (stream: ReadableStream<Uint8Array>) =>
      new TextDecoder().decode(
        new Uint8Array(await new Response(stream).arrayBuffer()).slice(0, 4),
      );
    return {
      recording: await head(window.viewer.saveRecording()),
      blueprint: await head(window.viewer.saveBlueprint()),
    };
  });
  expect(magic.recording).toMatch(/^RRF/);
  expect(magic.blueprint).toMatch(/^RRF/);
});

test("open_channel streams a recording into the viewer", async ({ viewer }) => {
  const count = await viewer.evaluate(async () => {
    await window.viewer.close("all");
    const bytes = new Uint8Array(await (await fetch("/example.rrd")).arrayBuffer());
    const channel = window.viewer.openChannel("test channel");
    channel.sendRrd(bytes);
    channel.close();

    for (let attempt = 0; attempt < 100; attempt += 1) {
      const recordings = (await window.viewer.getState()).recordings;
      if (0 < recordings.length) {
        return recordings.length;
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    return 0;
  });
  expect(count).toBe(1);
});
