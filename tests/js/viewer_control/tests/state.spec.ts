import { errorCode, expect, test } from "./fixtures";

test("get_state reports the open recording and its timelines", async ({ viewer }) => {
  const state = await viewer.evaluate(async () => {
    const state = await window.viewer.getState();
    const recording = state.recordings[0];
    return {
      recordings: state.recordings.length,
      activeIsOpen: state.activeStoreId === recording.storeId,
      location: recording.location,
      timelines: recording.timelines.map((timeline) => ({
        name: timeline.name,
        timeType: timeline.timeType,
        hasRange:
          timeline.timeRange !== undefined && timeline.timeRange.start <= timeline.timeRange.end,
      })),
      views: state.views.length,
      viewerVersion: state.viewerVersion,
    };
  });

  expect(state.recordings).toBe(1);
  expect(state.activeIsOpen).toBe(true);
  expect(state.location).toMatch(/^about:recording\?/);
  expect(state.timelines.length).toBeGreaterThan(0);
  for (const timeline of state.timelines) {
    expect(["sequence", "duration", "timestamp"]).toContain(timeline.timeType);
    expect(timeline.hasRange).toBe(true);
  }
  expect(state.views).toBeGreaterThan(0);
  expect(state.viewerVersion).toBeTruthy();
});

test("set_time seeks, switches timeline, and plays, pauses, or follows", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const before = await window.viewer.getState();
    const recording = before.recordings[0];
    const timeline =
      recording.timelines.find((timeline) => timeline.timeType === "sequence") ??
      recording.timelines[0];
    const range = timeline.timeRange!;
    const at = (time: bigint) =>
      timeline.timeType === "sequence"
        ? { sequence: time }
        : timeline.timeType === "timestamp"
          ? { timestampNanos: time }
          : { durationNanos: time };

    await window.viewer.setTime({
      timeline: timeline.name,
      ...at(range.start),
      playState: "paused",
    });
    const afterSeek = (await window.viewer.getState()).recordings[0];

    await window.viewer.setTime({ playState: "playing" });
    const playing = (await window.viewer.getState()).recordings[0].playState;

    await window.viewer.setTime({ playState: "following" });
    const following = (await window.viewer.getState()).recordings[0];

    await window.viewer.setTime({ playState: "paused" });
    const paused = (await window.viewer.getState()).recordings[0].playState;

    return {
      afterSeekTimeline: afterSeek.currentTime?.timeline === timeline.name,
      afterSeekTime: String(afterSeek.currentTime?.time),
      start: String(range.start),
      afterSeekPlayState: afterSeek.playState,
      playing,
      followingPlayState: following.playState,
      followingTime: String(following.currentTime?.time),
      end: String(range.end),
      paused,
    };
  });

  expect(result.afterSeekTimeline).toBe(true);
  expect(result.afterSeekTime).toBe(result.start);
  expect(result.afterSeekPlayState).toBe("paused");
  expect(result.playing).toBe("playing");
  expect(result.followingPlayState).toBe("following");
  expect(result.followingTime).toBe(result.end);
  expect(result.paused).toBe("paused");
});

test("set_time rejects what it cannot do", async ({ viewer }) => {
  expect(
    await errorCode(viewer, 'viewer.setTime({ timeline: "no such timeline", sequence: 0 })'),
  ).toBe("INVALID_ARGUMENT");
  expect(
    await errorCode(viewer, 'viewer.setTime({ recording: "Recording:nope:nope", sequence: 0 })'),
  ).toBe("INVALID_ARGUMENT");
  expect(await errorCode(viewer, 'viewer.setTime({ sequence: 0, playState: "following" })')).toBe(
    "INVALID_ARGUMENT",
  );
  // Two times at once never reaches the viewer.
  expect(await errorCode(viewer, "viewer.setTime({ sequence: 0, durationNanos: 0 })")).toMatch(
    /^not a ViewerControlError/,
  );
});

test("get_location and set_location go back to where the viewer was", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const location = await window.viewer.getLocation();
    await window.viewer.setLocation("about:settings");
    const elsewhere = await window.viewer.getLocation();
    await window.viewer.setLocation(location);
    return { location, elsewhere, again: await window.viewer.getLocation() };
  });
  expect(result.location).toMatch(/^about:recording\?/);
  expect(result.elsewhere).toBe("about:settings");
  expect(result.again).toBe(result.location);

  expect(await errorCode(viewer, 'viewer.setLocation("not a location")')).toBe("INVALID_ARGUMENT");
  expect(
    await errorCode(viewer, 'viewer.setLocation("about:recording?app_id=nope&recording_id=nope")'),
  ).toBe("NOT_FOUND");
});

test("get_logs returns only what is new after a sequence number", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const all = await window.viewer.getLogs();
    const last = all.at(-1)?.sequence;
    const newer = last === undefined ? [] : await window.viewer.getLogs(last);
    return {
      levels: all.map((entry) => entry.level),
      newerThanLast: newer.filter((entry) => last !== undefined && entry.sequence <= last).length,
    };
  });
  for (const level of result.levels) {
    expect(["INFO", "WARN", "ERROR"]).toContain(level);
  }
  expect(result.newerThanLast).toBe(0);
});
