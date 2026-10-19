import { WebViewer } from "@rerun-io/web-viewer";

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

/** Read the playback state of the active recording back from the viewer, and show it in the controls. */
function refresh() {
  const recordingId = viewer.get_active_recording_id();
  if (recordingId === null) {
    return;
  }

  playButton.disabled = false;
  playButton.textContent = viewer.get_playing(recordingId) ? "Pause" : "Play";

  const timeline = viewer.get_active_timeline(recordingId);
  timelineLabel.textContent = timeline ?? "No timeline";

  const range = timeline === null ? null : viewer.get_time_range(recordingId, timeline);
  timeSlider.disabled = range === null;
  if (timeline !== null && range !== null) {
    const time = viewer.get_current_time(recordingId, timeline);
    timeSlider.min = String(range.min);
    timeSlider.max = String(range.max);
    timeSlider.value = String(time);
    timeLabel.textContent = String(time);
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
playButton.addEventListener("click", () => {
  const recordingId = viewer.get_active_recording_id();
  if (recordingId !== null) {
    viewer.set_playing(recordingId, !viewer.get_playing(recordingId));
  }
});
timeSlider.addEventListener("input", () => {
  const recordingId = viewer.get_active_recording_id();
  const timeline = recordingId === null ? null : viewer.get_active_timeline(recordingId);
  if (recordingId !== null && timeline !== null) {
    viewer.set_current_time(recordingId, timeline, Number(timeSlider.value));
  }
});
