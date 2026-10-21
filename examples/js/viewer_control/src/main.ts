import {
  WebViewer,
  type SetTimeOptions,
  type ViewerRecording,
  type ViewerTimeline,
} from "@rerun-io/web-viewer";

const DEFAULT_RRD_URL = "https://app.rerun.io/version/0.39.0/examples/dna.rrd";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const playButton = $<HTMLButtonElement>("play");
const timelineLabel = $("timeline");
const timeSlider = $<HTMLInputElement>("time");
const timeLabel = $("time-label");
const log = $("log");

// Resolve relative URLs, which the viewer doesn't accept.
const rrdUrl = new URL(new URLSearchParams(location.search).get("url") ?? DEFAULT_RRD_URL, location.href).href;

const viewer = new WebViewer();
await viewer.start(rrdUrl, $("viewer"), {
  width: "100%",
  height: "100%",
  hide_welcome_screen: true,
});

function logEvent(text: string) {
  log.textContent += `${text}\n`;
  log.scrollTop = log.scrollHeight;
}

function logError(err: unknown) {
  logEvent(`error: ${err}`);
}

/** Ask the viewer for its active recording, and the timeline its time cursor is on. */
async function getActive(): Promise<{ recording: ViewerRecording; timeline?: ViewerTimeline } | undefined> {
  const state = await viewer.get_state();
  const recording = state.recordings.find((recording) => recording.store_id === state.active_store_id);
  if (recording === undefined) {
    return undefined;
  }
  const timeline = recording.timelines.find((t) => t.name === recording.current_time?.timeline);
  return { recording, timeline };
}

/**
 * Show the viewer's state in the controls.
 *
 * The viewer answers requests in order, so the last answer to arrive is the latest state.
 */
function refresh() {
  showState().catch(logError);
}

/** Show the active recording's playback in the controls. */
async function showState() {
  const active = await getActive();
  if (active === undefined) {
    return;
  }
  const { recording, timeline } = active;

  playButton.disabled = false;
  playButton.textContent = recording.play_state === "paused" ? "Play" : "Pause";

  const cursor = recording.current_time;
  timelineLabel.textContent = cursor?.timeline ?? "No timeline";

  const range = timeline?.time_range;
  timeSlider.disabled = range === undefined;
  if (range !== undefined) {
    const time = String(cursor?.time ?? range.start);
    timeSlider.min = String(range.start);
    timeSlider.max = String(range.end);
    timeSlider.value = time;
    timeLabel.textContent = time;
  }
}

// Viewer → page
viewer.on("recording_open", (event) => {
  logEvent(`recording_open: ${event.application_id} (${event.source})`);
  refresh();
});
viewer.on("play", () => {
  logEvent("play");
  refresh();
});
viewer.on("pause", () => {
  logEvent("pause");
  refresh();
});
viewer.on("timeline_change", (event) => {
  logEvent(`timeline_change: ${event.timeline}`);
  refresh();
});
viewer.on("time_update", refresh);
viewer.on("selection_change", (event) => {
  logEvent(`selection_change: ${JSON.stringify(event.items)}`);
});

// Page → viewer
//
// Each handler asks for the active recording afresh,
// since switching between open recordings need not emit any event.

async function togglePlaying() {
  const active = await getActive();
  if (active !== undefined) {
    const play_state = active.recording.play_state === "paused" ? "playing" : "paused";
    await viewer.set_time({ recording: active.recording.store_id, play_state });
  }
}

async function seek(time: bigint) {
  const active = await getActive();
  if (active === undefined) {
    return;
  }

  // The slider works in the timeline's own unit: sequence indices, or nanoseconds.
  const at: SetTimeOptions =
    active.timeline?.time_type === "sequence"
      ? { sequence: time }
      : active.timeline?.time_type === "timestamp"
        ? { timestamp_nanos: time }
        : { duration_nanos: time };
  await viewer.set_time({ ...at, recording: active.recording.store_id });
}

playButton.addEventListener("click", () => {
  togglePlaying().catch(logError);
});
timeSlider.addEventListener("input", () => {
  seek(BigInt(timeSlider.value)).catch(logError);
});
