import { expect, test } from "./fixtures";

test("on, once, and off deliver playback events", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const next = (event: "play" | "pause") => new Promise((resolve) => window.viewer.once(event, resolve));

    // The viewer plays the recording on its own after loading: stop that before listening.
    await window.viewer.set_time({ play_state: "paused" });
    while ((await window.viewer.get_state()).recordings[0].play_state !== "paused") {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    // Events are dispatched in a `setTimeout`, so this one runs after the `pause` event of the above.
    await new Promise((resolve) => setTimeout(resolve, 0));

    const seen: string[] = [];
    let once_count = 0;
    let off_count = 0;

    const off_play = window.viewer.on("play", () => seen.push("play"));
    window.viewer.on("pause", () => seen.push("pause"));
    window.viewer.once("play", () => (once_count += 1));
    const counted = () => (off_count += 1);
    window.viewer.on("play", counted);
    window.viewer.off("play", counted);

    let event = next("play");
    await window.viewer.set_time({ play_state: "playing" });
    await event;
    event = next("pause");
    await window.viewer.set_time({ play_state: "paused" });
    await event;
    off_play();
    event = next("play");
    await window.viewer.set_time({ play_state: "playing" });
    await event;

    return { seen, once_count, off_count };
  });

  expect(result.seen).toEqual(["play", "pause"]);
  expect(result.once_count).toBe(1);
  expect(result.off_count).toBe(0);
});

test("seeking and switching timeline emit time_update and timeline_change", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const recording = (await window.viewer.get_state()).recordings[0];
    const timelines: string[] = [];
    let time_updates = 0;
    window.viewer.on("timeline_change", (event) => timelines.push(event.timeline));
    window.viewer.on("time_update", () => (time_updates += 1));

    await window.viewer.set_time({ play_state: "paused" });
    for (const timeline of recording.timelines) {
      await window.viewer.set_time({ timeline: timeline.name });
    }
    const first = recording.timelines[0];
    const end = first.time_range!.end;
    const at =
      first.time_type === "sequence"
        ? { sequence: end }
        : first.time_type === "timestamp"
          ? { timestamp_nanos: end }
          : { duration_nanos: end };
    await window.viewer.set_time({ timeline: first.name, ...at });

    // The events arrive with the frames that apply the changes, which can be slow on CI.
    const names = recording.timelines.map((timeline) => timeline.name);
    const deadline = Date.now() + 30_000;
    while (Date.now() < deadline && (time_updates === 0 || names.slice(1).some((name) => !timelines.includes(name)))) {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    return { timelines, time_updates, names };
  });

  for (const name of result.names.slice(1)) {
    expect(result.timelines).toContain(name);
  }
  expect(result.time_updates).toBeGreaterThan(0);
});
