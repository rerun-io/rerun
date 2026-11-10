import { errorCode, expect, test } from "./fixtures";

test("get_recording_schema describes the entities, or only their paths", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const full = await window.viewer.getRecordingSchema();
    const paths = await window.viewer.getRecordingSchema({ pathsOnly: true });
    const first = full.entities[0].entityPath;
    const one = await window.viewer.getRecordingSchema({ entityPath: first });
    return {
      sameRecording: full.recording === (await window.viewer.getState()).activeStoreId,
      timelines: full.timelines.length,
      entities: full.entities.length,
      components: full.entities.flatMap((entity) => entity.components).length,
      pathComponents: paths.entities.flatMap((entity) => entity.components).length,
      oneUnderFirst: one.entities.every((entity) => entity.entityPath.startsWith(first)),
    };
  });

  expect(result.sameRecording).toBe(true);
  expect(result.timelines).toBeGreaterThan(0);
  expect(result.entities).toBeGreaterThan(0);
  expect(result.components).toBeGreaterThan(0);
  expect(result.pathComponents).toBe(0);
  expect(result.oneUnderFirst).toBe(true);

  expect(
    await errorCode(viewer, 'viewer.getRecordingSchema({ entityPath: "/no/such/entity" })'),
  ).toBe("NOT_FOUND");
  expect(
    await errorCode(viewer, 'viewer.getRecordingSchema({ recording: "Recording:nope:nope" })'),
  ).toBe("NOT_FOUND");
});

test("dataset and table schemas need a dataset or a table", async ({ viewer }) => {
  expect(await errorCode(viewer, "viewer.getDatasetSchema()")).toBe("FAILED_PRECONDITION");
  expect(await errorCode(viewer, "viewer.getDatasetAssetsSchema()")).toBe("FAILED_PRECONDITION");
  expect(await errorCode(viewer, "viewer.getTableSchema()")).toBe("FAILED_PRECONDITION");
});

test("get_blueprint and set_blueprint round-trip a recording's blueprint", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const before = await window.viewer.getBlueprint({ kind: "recording" });
    const written = await window.viewer.setBlueprint(before.blueprint, { kind: "recording" });
    const after = await window.viewer.getBlueprint({ kind: "recording" });
    const onScreen = await window.viewer.getBlueprint();
    return {
      recording: before.recording === (await window.viewer.getState()).activeStoreId,
      entries: Object.keys(before.blueprint).length,
      writtenId: written.blueprintId === before.blueprintId,
      samePaths:
        JSON.stringify(Object.keys(after.blueprint).sort()) ===
        JSON.stringify(Object.keys(before.blueprint).sort()),
      onScreenId: onScreen.blueprintId === before.blueprintId,
    };
  });
  expect(result.recording).toBe(true);
  expect(result.entries).toBeGreaterThan(0);
  expect(result.writtenId).toBe(true);
  expect(result.samePaths).toBe(true);
  expect(result.onScreenId).toBe(true);

  expect(
    await errorCode(
      viewer,
      'viewer.setBlueprint({ "/": { "NoSuchArchetype": {} } }, { kind: "recording" })',
    ),
  ).toBe("INVALID_ARGUMENT");
});

test("get_transform_graph reports the active recording's frames", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const { recording, graph } = await window.viewer.getTransformGraph({
      frameFilter: "all",
      edgeFilter: "all",
      includeTransforms: true,
    });
    return {
      recording: recording === (await window.viewer.getState()).activeStoreId,
      hasFrames: Array.isArray(graph.frames),
    };
  });
  expect(result.recording).toBe(true);
  expect(result.hasFrames).toBe(true);
});

test("take_screenshot returns a PNG, scaled to max_size", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const size = async (png: Uint8Array) => {
      const bitmap = await createImageBitmap(
        new Blob([new Uint8Array(png)], { type: "image/png" }),
      );
      return Math.max(bitmap.width, bitmap.height);
    };
    const full = await window.viewer.takeScreenshot();
    const small = await window.viewer.takeScreenshot({ maxSize: 64 });
    const viewId = (await window.viewer.getState()).views[0].viewId;
    const view = await window.viewer.takeScreenshot({ viewId, maxSize: 64 });
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

  expect(await errorCode(viewer, 'viewer.takeScreenshot({ view_id: "not a uuid" })')).toBe(
    "INVALID_ARGUMENT",
  );
  expect(
    await errorCode(
      viewer,
      'viewer.takeScreenshot({ view_id: "00000000-0000-0000-0000-000000000000" })',
    ),
  ).toBe("NOT_FOUND");
});

test("highlight_rect outlines a rectangle, and null dismisses it", async ({ viewer }) => {
  expect(
    await errorCode(viewer, 'viewer.highlightRect({ x: 10, y: 10, w: 100, h: 50 }, "here")'),
  ).toBe("resolved");
  expect(await errorCode(viewer, "viewer.highlightRect(null)")).toBe("resolved");
  expect(await errorCode(viewer, "viewer.highlightRect({ x: 10, y: 10, w: 0, h: 50 })")).toBe(
    "INVALID_ARGUMENT",
  );
});

test("list_commands describes the command palette, and run_command runs one", async ({
  viewer,
}) => {
  const result = await viewer.evaluate(async () => {
    const all = await window.viewer.listCommands();
    const runnable = all.find(
      (command) => command.available && !command.destructive && !command.blockingNativeModal,
    );
    const named = await window.viewer.listCommands({ ids: [all[1].id, all[0].id] });
    const ran = runnable && (await window.viewer.runCommand(runnable.id));
    return {
      count: all.length,
      described: all.every((command) => command.id && command.title && command.scope),
      sorted: all.every((command, i) => i === 0 || all[i - 1].id <= command.id),
      named: named.map((command) => command.id),
      expectedNamed: [all[1].id, all[0].id],
      runnable: runnable?.id,
      ran: ran?.id,
    };
  });
  expect(result.count).toBeGreaterThan(0);
  expect(result.described).toBe(true);
  expect(result.sorted).toBe(true);
  expect(result.named).toEqual(result.expectedNamed);
  expect(result.runnable).toBeTruthy();
  expect(result.ran).toBe(result.runnable);

  expect(await errorCode(viewer, 'viewer.listCommands({ ids: ["no.such.command"] })')).toBe(
    "NOT_FOUND",
  );
  expect(await errorCode(viewer, 'viewer.runCommand("no.such.command")')).toBe("NOT_FOUND");
});
