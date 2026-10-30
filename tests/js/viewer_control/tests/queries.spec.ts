import { error_code, expect, test } from "./fixtures";

test("get_recording_schema describes the entities, or only their paths", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const full = await window.viewer.get_recording_schema();
    const paths = await window.viewer.get_recording_schema({ paths_only: true });
    const first = full.entities[0].entity_path;
    const one = await window.viewer.get_recording_schema({ entity_path: first });
    return {
      same_recording: full.recording === (await window.viewer.get_state()).active_store_id,
      timelines: full.timelines.length,
      entities: full.entities.length,
      components: full.entities.flatMap((entity) => entity.components).length,
      path_components: paths.entities.flatMap((entity) => entity.components).length,
      one_under_first: one.entities.every((entity) => entity.entity_path.startsWith(first)),
    };
  });

  expect(result.same_recording).toBe(true);
  expect(result.timelines).toBeGreaterThan(0);
  expect(result.entities).toBeGreaterThan(0);
  expect(result.components).toBeGreaterThan(0);
  expect(result.path_components).toBe(0);
  expect(result.one_under_first).toBe(true);

  expect(await error_code(viewer, 'viewer.get_recording_schema({ entity_path: "/no/such/entity" })')).toBe(
    "NOT_FOUND",
  );
  expect(await error_code(viewer, 'viewer.get_recording_schema({ recording: "Recording:nope:nope" })')).toBe(
    "NOT_FOUND",
  );
});

test("dataset and table schemas need a dataset or a table", async ({ viewer }) => {
  expect(await error_code(viewer, "viewer.get_dataset_schema()")).toBe("FAILED_PRECONDITION");
  expect(await error_code(viewer, "viewer.get_dataset_assets_schema()")).toBe("FAILED_PRECONDITION");
  expect(await error_code(viewer, "viewer.get_table_schema()")).toBe("FAILED_PRECONDITION");
});

test("get_blueprint and set_blueprint round-trip a recording's blueprint", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const before = await window.viewer.get_blueprint({ kind: "recording" });
    const written = await window.viewer.set_blueprint(before.blueprint, { kind: "recording" });
    const after = await window.viewer.get_blueprint({ kind: "recording" });
    const on_screen = await window.viewer.get_blueprint();
    return {
      recording: before.recording === (await window.viewer.get_state()).active_store_id,
      entries: Object.keys(before.blueprint).length,
      written_id: written.blueprint_id === before.blueprint_id,
      same_paths: JSON.stringify(Object.keys(after.blueprint).sort()) === JSON.stringify(Object.keys(before.blueprint).sort()),
      on_screen_id: on_screen.blueprint_id === before.blueprint_id,
    };
  });
  expect(result.recording).toBe(true);
  expect(result.entries).toBeGreaterThan(0);
  expect(result.written_id).toBe(true);
  expect(result.same_paths).toBe(true);
  expect(result.on_screen_id).toBe(true);

  expect(
    await error_code(viewer, 'viewer.set_blueprint({ "/": { "NoSuchArchetype": {} } }, { kind: "recording" })'),
  ).toBe("INVALID_ARGUMENT");
});

test("get_transform_graph reports the active recording's frames", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const { recording, graph } = await window.viewer.get_transform_graph({
      frame_filter: "all",
      edge_filter: "all",
      include_transforms: true,
    });
    return {
      recording: recording === (await window.viewer.get_state()).active_store_id,
      has_frames: Array.isArray(graph.frames),
    };
  });
  expect(result.recording).toBe(true);
  expect(result.has_frames).toBe(true);
});

test("take_screenshot returns a PNG, scaled to max_size", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const size = async (png: Uint8Array) => {
      const bitmap = await createImageBitmap(new Blob([new Uint8Array(png)], { type: "image/png" }));
      return Math.max(bitmap.width, bitmap.height);
    };
    const full = await window.viewer.take_screenshot();
    const small = await window.viewer.take_screenshot({ max_size: 64 });
    const view_id = (await window.viewer.get_state()).views[0].view_id;
    const view = await window.viewer.take_screenshot({ view_id, max_size: 64 });
    return {
      magic: Array.from(full.slice(1, 4), (byte) => String.fromCharCode(byte)).join(""),
      full: await size(full),
      small: await size(small),
      view: await size(view),
    };
  });
  expect(result.magic).toBe("PNG");
  expect(result.full).toBeGreaterThan(64);
  expect(result.small).toBe(64);
  expect(result.view).toBe(64);

  expect(await error_code(viewer, 'viewer.take_screenshot({ view_id: "not a uuid" })')).toBe("INVALID_ARGUMENT");
  expect(
    await error_code(viewer, 'viewer.take_screenshot({ view_id: "00000000-0000-0000-0000-000000000000" })'),
  ).toBe("NOT_FOUND");
});

test("highlight_rect outlines a rectangle, and null dismisses it", async ({ viewer }) => {
  expect(await error_code(viewer, 'viewer.highlight_rect({ x: 10, y: 10, w: 100, h: 50 }, "here")')).toBe("resolved");
  expect(await error_code(viewer, "viewer.highlight_rect(null)")).toBe("resolved");
  expect(await error_code(viewer, "viewer.highlight_rect({ x: 10, y: 10, w: 0, h: 50 })")).toBe("INVALID_ARGUMENT");
});

test("list_commands describes the command palette, and run_command runs one", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const all = await window.viewer.list_commands();
    const runnable = all.find((command) => command.available && !command.destructive && !command.blocking_native_modal);
    const named = await window.viewer.list_commands({ ids: [all[1].id, all[0].id] });
    const ran = runnable && (await window.viewer.run_command(runnable.id));
    return {
      count: all.length,
      described: all.every((command) => command.id && command.title && command.scope),
      sorted: all.every((command, i) => i === 0 || all[i - 1].id <= command.id),
      named: named.map((command) => command.id),
      expected_named: [all[1].id, all[0].id],
      runnable: runnable?.id,
      ran: ran?.id,
    };
  });
  expect(result.count).toBeGreaterThan(0);
  expect(result.described).toBe(true);
  expect(result.sorted).toBe(true);
  expect(result.named).toEqual(result.expected_named);
  expect(result.runnable).toBeTruthy();
  expect(result.ran).toBe(result.runnable);

  expect(await error_code(viewer, 'viewer.list_commands({ ids: ["no.such.command"] })')).toBe("NOT_FOUND");
  expect(await error_code(viewer, 'viewer.run_command("no.such.command")')).toBe("NOT_FOUND");
});
