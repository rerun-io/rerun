import { expect, test } from "./fixtures";

test("on, once, and off deliver playback events", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const next = (event: "play" | "pause") =>
      new Promise((resolve) => window.viewer.once(event, resolve));

    // The viewer plays the recording on its own after loading: stop that before listening.
    await window.viewer.setTime({ playState: "paused" });
    while ((await window.viewer.getState()).recordings[0].playState !== "paused") {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    // Events are dispatched in a `setTimeout`, so this one runs after the `pause` event of the above.
    await new Promise((resolve) => setTimeout(resolve, 0));

    const seen: string[] = [];
    let onceCount = 0;
    let offCount = 0;

    const offPlay = window.viewer.on("play", () => seen.push("play"));
    window.viewer.on("pause", () => seen.push("pause"));
    window.viewer.once("play", () => (onceCount += 1));
    const counted = () => (offCount += 1);
    window.viewer.on("play", counted);
    window.viewer.off("play", counted);

    let event = next("play");
    await window.viewer.setTime({ playState: "playing" });
    await event;
    event = next("pause");
    await window.viewer.setTime({ playState: "paused" });
    await event;
    offPlay();
    event = next("play");
    await window.viewer.setTime({ playState: "playing" });
    await event;

    return { seen, onceCount, offCount };
  });

  expect(result.seen).toEqual(["play", "pause"]);
  expect(result.onceCount).toBe(1);
  expect(result.offCount).toBe(0);
});

test("seeking and switching timeline emit time_update and timeline_change", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const recording = (await window.viewer.getState()).recordings[0];
    const timelines: string[] = [];
    let timeUpdates = 0;
    window.viewer.on("timeline_change", (event) => timelines.push(event.timeline));
    window.viewer.on("time_update", () => (timeUpdates += 1));

    await window.viewer.setTime({ playState: "paused" });
    for (const timeline of recording.timelines) {
      await window.viewer.setTime({ timeline: timeline.name });
    }
    const first = recording.timelines[0];
    const end = first.timeRange!.end;
    const at =
      first.timeType === "sequence"
        ? { sequence: end }
        : first.timeType === "timestamp"
          ? { timestampNanos: end }
          : { durationNanos: end };
    await window.viewer.setTime({ timeline: first.name, ...at });

    // The events arrive with the frames that apply the changes, which can be slow on CI.
    const names = recording.timelines.map((timeline) => timeline.name);
    const deadline = Date.now() + 30_000;
    while (
      Date.now() < deadline &&
      (timeUpdates === 0 || names.slice(1).some((name) => !timelines.includes(name)))
    ) {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    return { timelines, timeUpdates, names };
  });

  for (const name of result.names.slice(1)) {
    expect(result.timelines).toContain(name);
  }
  expect(result.timeUpdates).toBeGreaterThan(0);
});
