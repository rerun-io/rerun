import { expect, test } from "./fixtures";

test("the deprecated getters and setters still work", async ({ viewer }) => {
  const result = await viewer.evaluate(async () => {
    const v = window.viewer;
    const id = v.get_active_recording_id()!;
    const timeline = v.get_active_timeline(id)!;
    const range = v.get_time_range(id, timeline)!;

    v.set_playing(id, false);
    v.set_current_time(id, timeline, range.min);
    v.set_active_timeline(id, timeline);
    v.set_active_recording_id(id);

    // The old setters return nothing to wait on: the viewer applies them on a later frame.
    const deadline = Date.now() + 30_000;
    while (Date.now() < deadline && (v.get_playing(id) || v.get_current_time(id, timeline) !== range.min)) {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }

    return {
      // The old API names a recording by its recording id, not its whole store id.
      id_matches: id === window.rerun.parse_store_id((await v.get_state()).active_store_id!).recording_id,
      timeline,
      range_ok: range.min <= range.max,
      playing: v.get_playing(id),
      time: v.get_current_time(id, timeline),
      min: range.min,
      unknown_recording_time_range: v.get_time_range("no such recording", timeline),
    };
  });
  expect(result.id_matches).toBe(true);
  expect(result.timeline).toBeTruthy();
  expect(result.range_ok).toBe(true);
  expect(result.playing).toBe(false);
  expect(result.time).toBe(result.min);
  expect(result.unknown_recording_time_range).toBeNull();
});
