import { error_code, expect, test } from "./fixtures";

test("open names the recording it opened, and close by URL closes it", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const url = new URL("/example.rrd", location.href).href;
    const open_store_id = (await window.viewer.get_state()).recordings[0].store_id;

    const opened = await window.viewer.open(url);
    const closed = await window.viewer.close(url);
    const left = (await window.viewer.get_state()).recordings.length;

    return { open_store_id, opened, closed, left };
  });

  expect(result.opened.store_ids).toEqual([result.open_store_id]);
  expect(result.opened.loading).toEqual([]);
  expect(result.closed).toEqual([result.open_store_id]);
  expect(result.left).toBe(0);

  // Nothing is loaded from it anymore.
  expect(await error_code(viewer, 'viewer.close(new URL("/example.rrd", location.href).href)')).toBe("NOT_FOUND");
});

test("close selects by store id, the active recording, or all of them", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const url = new URL("/example.rrd", location.href).href;
    const store_id = (await window.viewer.get_state()).recordings[0].store_id;

    const by_store_id = await window.viewer.close({ store_ids: [store_id] });
    await window.viewer.open(url);
    const current = await window.viewer.close("current");
    await window.viewer.open(url);
    const all = await window.viewer.close("all");

    return { store_id, by_store_id, current, all };
  });

  expect(result.by_store_id).toEqual([result.store_id]);
  expect(result.current).toEqual([result.store_id]);
  expect(result.all).toEqual([result.store_id]);

  expect(await error_code(viewer, 'viewer.close({ store_ids: ["Recording:nope:nope"] })')).toBe("NOT_FOUND");
  // A selector that is not `true` never reaches the viewer, rather than closing everything.
  expect(await error_code(viewer, "viewer.close({ all: false })")).toMatch(/^not a ViewerControlError/);
});

test("open rejects a recording location it cannot load", async ({ viewer }) => {
  expect(await error_code(viewer, 'viewer.open("about:recording?app_id=nope&recording_id=nope")')).toBe("NOT_FOUND");
});

test("save_recording and save_blueprint stream .rrd bytes", async ({ viewer }) => {
  const magic = await viewer.evaluate(async () => {
    const head = async (stream: ReadableStream<Uint8Array>) =>
      new TextDecoder().decode(new Uint8Array(await new Response(stream).arrayBuffer()).slice(0, 4));
    return {
      recording: await head(window.viewer.save_recording()),
      blueprint: await head(window.viewer.save_blueprint()),
    };
  });
  expect(magic.recording).toMatch(/^RRF/);
  expect(magic.blueprint).toMatch(/^RRF/);
});

test("open_channel streams a recording into the viewer", async ({ viewer }) => {
  const count = await viewer.evaluate(async () => {
    await window.viewer.close("all");
    const bytes = new Uint8Array(await (await fetch("/example.rrd")).arrayBuffer());
    const channel = window.viewer.open_channel("test channel");
    channel.send_rrd(bytes);
    channel.close();

    for (let attempt = 0; attempt < 100; attempt += 1) {
      const recordings = (await window.viewer.get_state()).recordings;
      if (0 < recordings.length) {
        return recordings.length;
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    return 0;
  });
  expect(count).toBe(1);
});
