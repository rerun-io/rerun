import { error_code, expect, test } from "./fixtures";

test("get_state reports the open recording and its timelines", async ({ viewer }) => {
  const state = await viewer.evaluate(async () => {
    const state = await window.viewer.get_state();
    const recording = state.recordings[0];
    return {
      recordings: state.recordings.length,
      active_is_open: state.active_store_id === recording.store_id,
      location: recording.location,
      timelines: recording.timelines.map((timeline) => ({
        name: timeline.name,
        time_type: timeline.time_type,
        has_range: timeline.time_range !== undefined && timeline.time_range.start <= timeline.time_range.end,
      })),
      views: state.views.length,
      viewer_version: state.viewer_version,
    };
  });

  expect(state.recordings).toBe(1);
  expect(state.active_is_open).toBe(true);
  expect(state.location).toMatch(/^about:recording\?/);
  expect(state.timelines.length).toBeGreaterThan(0);
  for (const timeline of state.timelines) {
    expect(["sequence", "duration", "timestamp"]).toContain(timeline.time_type);
    expect(timeline.has_range).toBe(true);
  }
  expect(state.views).toBeGreaterThan(0);
  expect(state.viewer_version).toBeTruthy();
});

test("set_time seeks, switches timeline, and plays, pauses, or follows", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const before = await window.viewer.get_state();
    const recording = before.recordings[0];
    const timeline = recording.timelines.find((timeline) => timeline.time_type === "sequence") ?? recording.timelines[0];
    const range = timeline.time_range!;
    const at = (time: bigint) =>
      timeline.time_type === "sequence"
        ? { sequence: time }
        : timeline.time_type === "timestamp"
          ? { timestamp_nanos: time }
          : { duration_nanos: time };

    await window.viewer.set_time({ timeline: timeline.name, ...at(range.start), play_state: "paused" });
    const after_seek = (await window.viewer.get_state()).recordings[0];

    await window.viewer.set_time({ play_state: "playing" });
    const playing = (await window.viewer.get_state()).recordings[0].play_state;

    await window.viewer.set_time({ play_state: "following" });
    const following = (await window.viewer.get_state()).recordings[0];

    await window.viewer.set_time({ play_state: "paused" });
    const paused = (await window.viewer.get_state()).recordings[0].play_state;

    return {
      after_seek_timeline: after_seek.current_time?.timeline === timeline.name,
      after_seek_time: String(after_seek.current_time?.time),
      start: String(range.start),
      after_seek_play_state: after_seek.play_state,
      playing,
      following_play_state: following.play_state,
      following_time: String(following.current_time?.time),
      end: String(range.end),
      paused,
    };
  });

  expect(result.after_seek_timeline).toBe(true);
  expect(result.after_seek_time).toBe(result.start);
  expect(result.after_seek_play_state).toBe("paused");
  expect(result.playing).toBe("playing");
  expect(result.following_play_state).toBe("following");
  expect(result.following_time).toBe(result.end);
  expect(result.paused).toBe("paused");
});

test("set_time rejects what it cannot do", async ({ viewer }) => {
  expect(await error_code(viewer, 'viewer.set_time({ timeline: "no such timeline", sequence: 0 })')).toBe(
    "INVALID_ARGUMENT",
  );
  expect(await error_code(viewer, 'viewer.set_time({ recording: "Recording:nope:nope", sequence: 0 })')).toBe(
    "INVALID_ARGUMENT",
  );
  expect(await error_code(viewer, 'viewer.set_time({ sequence: 0, play_state: "following" })')).toBe(
    "INVALID_ARGUMENT",
  );
  // Two times at once never reaches the viewer.
  expect(await error_code(viewer, "viewer.set_time({ sequence: 0, duration_nanos: 0 })")).toMatch(
    /^not a ViewerControlError/,
  );
});

test("get_location and set_location go back to where the viewer was", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const location = await window.viewer.get_location();
    await window.viewer.set_location("about:settings");
    const elsewhere = await window.viewer.get_location();
    await window.viewer.set_location(location);
    return { location, elsewhere, again: await window.viewer.get_location() };
  });
  expect(result.location).toMatch(/^about:recording\?/);
  expect(result.elsewhere).toBe("about:settings");
  expect(result.again).toBe(result.location);

  expect(await error_code(viewer, 'viewer.set_location("not a location")')).toBe("INVALID_ARGUMENT");
  expect(await error_code(viewer, 'viewer.set_location("about:recording?app_id=nope&recording_id=nope")')).toBe(
    "NOT_FOUND",
  );
});

test("get_logs returns only what is new after a sequence number", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const all = await window.viewer.get_logs();
    const last = all.at(-1)?.sequence;
    const newer = last === undefined ? [] : await window.viewer.get_logs(last);
    return {
      levels: all.map((entry) => entry.level),
      newer_than_last: newer.filter((entry) => last !== undefined && entry.sequence <= last).length,
    };
  });
  for (const level of result.levels) {
    expect(["INFO", "WARN", "ERROR"]).toContain(level);
  }
  expect(result.newer_than_last).toBe(0);
});
