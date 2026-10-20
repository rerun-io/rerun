// @ts-ignore
import type { WebHandle, wasm_bindgen } from "./re_viewer";
import { create, fromBinary, toBinary, type MessageInitShape } from "@bufbuild/protobuf";
import {
  ViewerControlRequestSchema,
  ViewerControlResponseSchema,
  type GetViewerStateResponse,
  type ViewerControlRequest,
  type ViewerControlResponse,
} from "./proto/rerun/v1alpha1/viewer_control_pb.js";
import { EntryKind as ProtoEntryKind } from "./proto/rerun/v1alpha1/cloud_pb.js";
import { TimeType as ProtoTimeType } from "./proto/rerun/v1alpha1/common_pb.js";

/** What the viewer shows, as {@link WebViewer.get_state} reports it. */
export interface ViewerState {
  /** Where the viewer is, for {@link WebViewer.set_location}. Empty if that has no location. */
  location: string;

  /** The active recording, as a store id. */
  active_store_id?: string;

  /** Every recording open in the viewer. */
  recordings: ViewerRecording[];

  /** The views of the active recording's blueprint. */
  views: ViewerView[];

  /** What the viewer is still loading, empty once everything has arrived. */
  loading: ViewerLoadingSource[];

  /** The servers in the viewer's left panel. */
  servers: ViewerServer[];

  /** The origin of the catalog server the viewer hosts, if any. */
  catalog_url?: string;

  /** The version of the viewer. */
  viewer_version?: string;
}

/** A recording open in the viewer. */
export interface ViewerRecording {
  /** The recording's store id, `{kind}:{application_id}:{recording_id}`. */
  store_id: string;

  /** The location that shows this recording, for {@link WebViewer.set_location}. */
  location: string;

  /** The recording's timelines. */
  timelines: ViewerTimeline[];

  /** The recording's time cursor, if it has one. */
  current_time?: TimeCursor;
}

/** A timeline of a recording. */
export interface ViewerTimeline {
  /** The timeline's name. */
  name: string;

  /** What kind of time the timeline holds. */
  time_type?: TimeType;

  /** The first and last time on the timeline, if it has any data. */
  time_range?: TimeRange;
}

/** Where a recording's time cursor is. */
export interface TimeCursor {
  /** The timeline the cursor is on. */
  timeline: string;

  /** What kind of time the timeline holds. */
  time_type?: TimeType;

  /** The cursor's time, in the timeline's own unit, if it is set. */
  time?: bigint;
}

/** A range of time, in the timeline's own unit, both ends inclusive. */
export interface TimeRange {
  start: bigint;
  end: bigint;
}

/**
 * What kind of time a timeline holds, and so the unit of its times:
 * a sequence index, a duration in nanoseconds, or a timestamp in nanoseconds since the Unix epoch.
 */
export type TimeType = "sequence" | "duration" | "timestamp";

/** A view of the current blueprint. */
export interface ViewerView {
  view_id: string;

  /** The view's class, e.g. `Spatial3D`. */
  class: string;

  name: string;

  /** The view's origin entity path. */
  origin: string;

  visible: boolean;

  /** The warnings and errors the view reported the last time it was shown. */
  reports: ViewerReport[];
}

/** A warning or an error a view reported. */
export interface ViewerReport {
  /** `warning` or `error`. */
  severity: string;

  summary: string;
  details?: string;
}

/** A server in the viewer's left panel. */
export interface ViewerServer {
  origin: string;

  /** The server's datasets and tables. */
  entries: ViewerServerEntry[];

  /** Why the viewer could not list the server's entries, if it could not. */
  error?: string;
}

/** A dataset or table on a server. */
export interface ViewerServerEntry {
  name: string;
  kind?: EntryKind;

  /** The entry's URL, for {@link WebViewer.open} or {@link WebViewer.set_location}. */
  url: string;
}

/** What kind of entry a server holds. */
export type EntryKind =
  | "dataset"
  | "dataset_view"
  | "table"
  | "table_view"
  | "blueprint_dataset"
  | "asset_dataset";

/** Something the viewer is still loading. */
export interface ViewerLoadingSource {
  /** What is being loaded: a file path, a URL's display name, or a segment id. */
  name: string;

  /** The same thing the viewer's own loading screen says. */
  status: string;
}

/** The public {@link ViewerState} of a `GetViewerState` response. */
function viewer_state_of(state: GetViewerStateResponse): ViewerState {
  return {
    location: state.location,
    active_store_id: state.activeStoreId,
    recordings: state.recordings.map((recording) => ({
      store_id: recording.storeId,
      location: recording.location,
      timelines: recording.timelines.map((timeline) => ({
        name: timeline.timeline?.name ?? "",
        time_type: time_type_of(timeline.timeType),
        time_range: timeline.timeRange && {
          start: timeline.timeRange.start,
          end: timeline.timeRange.end,
        },
      })),
      current_time: recording.currentTime && {
        timeline: recording.currentTime.timeline?.name ?? "",
        time_type: time_type_of(recording.currentTime.timeType),
        time: recording.currentTime.time?.time,
      },
    })),
    views: state.views.map((view) => ({
      view_id: view.viewId,
      class: view.class,
      name: view.name,
      origin: view.origin,
      visible: view.visible,
      reports: view.reports.map(({ severity, summary, details }) => ({ severity, summary, details })),
    })),
    loading: state.loading.map(({ name, status }) => ({ name, status })),
    servers: state.servers.map((server) => ({
      origin: server.origin,
      entries: server.entries.map((entry) => ({
        name: entry.name,
        kind: entry_kind_of(entry.kind),
        url: entry.url,
      })),
      error: server.error,
    })),
    catalog_url: state.catalogUrl,
    viewer_version: state.viewerVersion,
  };
}

function time_type_of(time_type: ProtoTimeType | undefined): TimeType | undefined {
  switch (time_type) {
    case ProtoTimeType.SEQUENCE:
      return "sequence";
    case ProtoTimeType.DURATION_NS:
      return "duration";
    case ProtoTimeType.TIMESTAMP_NS:
      return "timestamp";
    default:
      return undefined;
  }
}

function entry_kind_of(kind: ProtoEntryKind): EntryKind | undefined {
  switch (kind) {
    case ProtoEntryKind.DATASET:
      return "dataset";
    case ProtoEntryKind.DATASET_VIEW:
      return "dataset_view";
    case ProtoEntryKind.TABLE:
      return "table";
    case ProtoEntryKind.TABLE_VIEW:
      return "table_view";
    case ProtoEntryKind.BLUEPRINT_DATASET:
      return "blueprint_dataset";
    case ProtoEntryKind.ASSET_DATASET:
      return "asset_dataset";
    default:
      return undefined;
  }
}

/**
 * Where to move the time cursor to, for {@link WebViewer.set_time}, and on which recording and timeline.
 *
 * Set exactly one of `sequence`, `duration_nanos`, and `timestamp_nanos`.
 */
export type SetTimeOptions = SetTimeTarget & TimeValue;

/** The recording and timeline that {@link WebViewer.set_time} acts on. */
export interface SetTimeTarget {
  /** The timeline to seek on. If omitted, the viewer uses its active timeline. */
  timeline?: string;

  /** Start playing from the new position. The viewer pauses by default. */
  play?: boolean;

  /** The recording to seek, as a store id from {@link WebViewer.get_state}. If omitted, the active recording. */
  recording?: string;
}

/**
 * A time on a timeline, as exactly one of a sequence index, a duration, or a timestamp.
 *
 * Integer values only: a fractional `number` is rejected.
 */
export type TimeValue =
  | {
      /** A sequence index. */
      sequence: number | bigint;
      duration_nanos?: never;
      timestamp_nanos?: never;
    }
  | {
      sequence?: never;
      /** A duration, in nanoseconds. */
      duration_nanos: number | bigint;
      timestamp_nanos?: never;
    }
  | {
      sequence?: never;
      duration_nanos?: never;
      /**
       * A timestamp, in nanoseconds since the Unix epoch.
       *
       * A `number` is only exact up to 2^53, which today's timestamps exceed, so prefer a `bigint`.
       */
      timestamp_nanos: number | bigint;
    };

/** Why an operation of the viewer's control API failed, as a gRPC status code. */
export type ViewerControlErrorCode =
  | "INVALID_ARGUMENT"
  | "NOT_FOUND"
  | "FAILED_PRECONDITION"
  | "INTERNAL";

/** A failed operation of the viewer's control API, such as {@link WebViewer.set_time}. */
export class ViewerControlError extends Error {
  override name = "ViewerControlError";

  /** Why the operation failed. */
  readonly code: ViewerControlErrorCode;

  constructor(code: ViewerControlErrorCode, message: string) {
    super(message);
    this.code = code;
  }
}

/** A {@link ViewerControlError} for an error that the wasm viewer rejected with. */
function as_viewer_control_error(err: unknown): unknown {
  if (err instanceof Error && err.name === "ViewerControlError" && "code" in err) {
    return new ViewerControlError(err.code as ViewerControlErrorCode, err.message);
  }
  return err;
}

/** An operation of the viewer's control protocol, e.g. `"getViewerState"`. */
type Operation = NonNullable<ViewerControlRequest["kind"]["case"]>;

/** The request of the operation `K`. */
type OperationRequest<K extends Operation> = Extract<
  NonNullable<MessageInitShape<typeof ViewerControlRequestSchema>["kind"]>,
  { case: K }
>["value"];

/** The response of the operation `K`. */
type OperationResponse<K extends Operation> = Extract<
  ViewerControlResponse["kind"],
  { case: K }
>["value"];

/** The time that `options` seeks to, in the timeline's own unit. */
function time_of(options: TimeValue): bigint {
  // The type allows only one, but plain JavaScript callers are not type-checked.
  const values = [options.sequence, options.duration_nanos, options.timestamp_nanos].filter(
    (value) => value !== undefined,
  );
  if (values.length !== 1) {
    throw new Error(
      "set_time expects exactly one of `sequence`, `duration_nanos`, and `timestamp_nanos`",
    );
  }
  const [value] = values;
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw new Error(`set_time expects an integer time, got ${value}`);
  }
  return BigInt(value);
}

let get_wasm_bindgen: (() => typeof wasm_bindgen) | null = null;
let _wasm_module: WebAssembly.Module | null = null;

/**
 * Feature-detect WebAssembly SIMD (`simd128`).
 *
 * The viewer .wasm is compiled with `-Ctarget-feature=+simd128`, so a browser
 * without SIMD support will fail to instantiate the module with a cryptic
 * `CompileError`. We probe up-front and surface a clear error instead.
 *
 * The probe is a minimal module that uses the `v128.any_true` instruction.
 * Supported in: Chrome 91+, Firefox 89+, Safari 16.4+.
 */
function has_wasm_simd(): boolean {
  try {
    return WebAssembly.validate(new Uint8Array([
      0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0,
      10, 10, 1, 8, 0, 65, 0, 253, 15, 253, 98, 11,
    ]));
  } catch {
    return false;
  }
}

const UNSUPPORTED_BROWSER_MESSAGE =
  "Your browser is too old to run the Rerun Viewer. " +
  "The Viewer requires WebAssembly SIMD support, available in " +
  "Chrome 91+, Firefox 89+, Safari 16.4+, or any modern Chromium-based browser. " +
  "Please update your browser and try again.";

// The public export API uses streams so the private Wasm API can switch to incremental encoding
// without requiring a breaking JavaScript API change.
function export_stream(bytes: Uint8Array): ReadableStream<Uint8Array> {
  return new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(bytes);
      controller.close();
    },
  });
}

async function fetch_viewer_js(base_url?: string): Promise<(() => typeof wasm_bindgen)> {
  // @ts-ignore
  return (await import("./re_viewer")).default;
}

async function fetch_viewer_wasm(
  base_url?: string,
  on_progress?: (received: number, total: number | null) => void,
): Promise<Response> {
  //!<INLINE-MARKER-OPEN>
  const url = base_url
    ? new URL("./re_viewer_bg.wasm", base_url)
    : new URL("./re_viewer_bg.wasm", import.meta.url);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(
      `Failed to fetch viewer Wasm: ${response.status} ${response.statusText}`,
    );
  }
  return wrap_fetch_with_progress(response, on_progress);
  //!<INLINE-MARKER-CLOSE>
}

/**
 * Estimates total uncompressed bytes for progress display.
 * This is a rough estimate — do NOT use for truncation detection.
 */
function estimate_total_bytes(response: Response): number | null {
  // When served with `rerun-final-length`, use that (set by `re_web_viewer_server`).
  const final_length = response.headers.get("rerun-final-length");
  if (final_length != null) return parseInt(final_length, 10);

  // When gzip-compressed, try the GCS uncompressed-size header.
  if (response.headers.get("content-encoding") === "gzip") {
    const uncompressed = response.headers.get("x-goog-meta-uncompressed-size");
    if (uncompressed != null) return parseInt(uncompressed, 10);

    // Fall back to content-length * 3 (good empirical approximation for gzip'd wasm).
    const cl = response.headers.get("content-length");
    if (cl != null) return parseInt(cl, 10) * 3;
  }

  // Uncompressed: content-length is the exact size.
  const cl = response.headers.get("content-length");
  if (cl != null) return parseInt(cl, 10);

  return null;
}

/**
 * Wraps a fetch response to track download progress.
 */
function wrap_fetch_with_progress(
  response: Response,
  on_progress?: (received: number, total: number | null) => void,
): Response {
  const total_bytes = estimate_total_bytes(response);

  if (!response.body) return response;

  let received = 0;
  const body = response.body;
  const tracked = new ReadableStream({
    async start(controller) {
      const reader = body.getReader();
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        received += value.byteLength;
        on_progress?.(received, total_bytes);
        controller.enqueue(value);
      }
      controller.close();
    },
  });

  return new Response(tracked, {
    status: response.status,
    statusText: response.statusText,
    headers: response.headers,
  });
}

function format_mib(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1) + " MiB";
}

async function load(
  base_url?: string,
  on_progress?: (received: number, total: number | null) => void,
): Promise<typeof wasm_bindgen.WebHandle> {
  if (!has_wasm_simd()) {
    throw new Error(UNSUPPORTED_BROWSER_MESSAGE);
  }

  // instantiate wbg globals+module for every invocation of `load`,
  // but don't load the JS/Wasm source every time
  if (!get_wasm_bindgen || !_wasm_module) {
    [get_wasm_bindgen, _wasm_module] = await Promise.all([
      fetch_viewer_js(base_url),
      WebAssembly.compileStreaming(fetch_viewer_wasm(base_url, on_progress)),
    ]);
  }
  let bindgen = get_wasm_bindgen();
  await bindgen({ module_or_path: _wasm_module });
  return class extends bindgen.WebHandle {
    free() {
      super.free();
      // @ts-ignore
      bindgen.deinit();
    }
  };
}

let _minimize_current_fullscreen_viewer: (() => void) | null = null;

function randomId(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes)
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

export type Panel = "top" | "blueprint" | "selection" | "time";
export type PanelState = "hidden" | "collapsed" | "expanded";
export type Backend = "webgpu" | "webgl";
export type VideoDecoder = "auto" | "prefer_software" | "prefer_hardware";

export interface LoginOptions {
  /** URL to redirect to after successful OAuth login (e.g. "/signed-in" or "https://example.com/signed-in"). */
  signed_in_url: string;
  /** URL to redirect to after logout (e.g. "/signed-out" or "https://example.com/signed-out"). */
  signed_out_url: string;
}

// NOTE: When changing these options, consider how it affects the `web-viewer-react` package:
//       - Should this option be exposed?
//       - Should changing this option result in the viewer being restarted?
export interface WebViewerOptions {
  /** Url to the example manifest. Unused if `hide_welcome_screen` is set to `true`. */
  manifest_url?: string;

  /** The render backend used by the viewer. Either "webgl" or "webgpu". Prefers "webgpu". */
  render_backend?: Backend;

  /** Video decoder config used by the viewer. Either "auto", "prefer_software" or "prefer_hardware". */
  video_decoder?: VideoDecoder;

  /**
   * An upper limit on how much memory the viewer uses, e.g. "500MB" or "50%".
   *
   * When this limit is reached, the viewer drops the oldest data.
   * A percentage is relative to the 4 GiB a Wasm process can use.
   * The limit is capped at 2.5 GB, which is also the default.
   * If not set, uses the limit from the viewer settings.
   */
  memory_limit?: string;

  /** If set to `true`, hides the welcome screen, which contains our examples. Defaults to `false`. */
  hide_welcome_screen?: boolean;

  /**
   * Override whether the viewer checks for newer Rerun releases on startup.
   * Set to `false` to disable these checks.
   * If not set, uses the saved user preference (enabled by default).
   */
  check_for_updates_on_startup?: boolean;

  /**
   * Allow the viewer to handle fullscreen mode.
   * This option sets canvas style so is not recommended if you are doing anything custom,
   * or are embedding the viewer in an iframe.
   *
   * Defaults to `false`.
   */
  allow_fullscreen?: boolean;

  /**
   * Enable the history feature of the viewer.
   *
   * This is only relevant when `hide_welcome_screen` is `false`,
   * as it's currently only used to allow going between the welcome screen and examples.
   *
   * Defaults to `false`.
   */
  enable_history?: boolean;

  /** The CSS width of the canvas. */
  width?: string;

  /** The CSS height of the canvas. */
  height?: string;

  /** The fallback token to use, if any.
   *
   * The fallback token behaves similarly to the `REDAP_TOKEN` env variable. If set in the
   * enclosing notebook environment, it should be used to set the fallback token.
   */
  fallback_token?: string;

  /**
   * The color theme to use.
   *
   * If not set, the viewer uses the previously persisted theme preference or defaults to "system".
   */
  theme?: "dark" | "light" | "system";

  /**
   * Enable OAuth login in the viewer.
   *
   * When set, the viewer shows login UI and uses the provided URLs for OAuth redirects.
   *
   * To use this:
   * 1. Host the `signed-in.html` and `signed-out.html` pages alongside your viewer.
   *    Templates can be found at:
   *    - https://github.com/rerun-io/rerun/blob/main/crates/top/re_web_viewer_server/web_viewer/signed-in.html
   *    - https://github.com/rerun-io/rerun/blob/main/crates/top/re_web_viewer_server/web_viewer/signed-out.html
   * 2. Set the URLs to those pages here.
   * 3. Contact your Rerun representative to have the redirect URLs
   *    and origin whitelisted in the OAuth configuration.
   *
   * When not set (default), login UI is hidden. Token-based auth still works.
   */
  login?: LoginOptions;
}

// `AppOptions` and `WebViewerOptions` must be compatible
// otherwise we need to restructure how we pass options to the viewer

/**
 * The public interface is @see {WebViewerOptions}. This adds a few additional, internal options.
 *
 * @private
 */
export interface AppOptions extends WebViewerOptions {
  /** The url that's used when sharing web viewer urls
   *
   * If not set, the viewer will use the url of the page it is embedded in.
   */
  viewer_base_url?: string;

  /** Whether the viewer is running in a notebook. */
  notebook?: boolean;

  url?: string;
  panel_state_overrides?: Partial<{
    [K in Panel]: PanelState;
  }>;
  on_viewer_event?: (event_json: string) => void;
  fullscreen?: FullscreenOptions;
}

// Types are based on `crates/top/re_viewer/src/event.rs`.
// Important: The event names defined here are `snake_case` versions
// of their `PascalCase` counterparts on the Rust side.
/** An event produced in the Viewer. */
export type ViewerEvent =
  | PlayEvent
  | PauseEvent
  | TimeUpdateEvent
  | TimelineChangeEvent
  | SelectionChangeEvent
  | RecordingOpenEvent;

/**
 * Properties available on all {@link ViewerEvent} types.
 */
export type ViewerEventBase = {
  application_id: string;
  recording_id: string;
  partition_id?: string;
}

/**
 * Fired when the timeline starts playing.
 */
export type PlayEvent = ViewerEventBase & {
  type: "play";
};

/**
 * Fired when the timeline stops playing.
 */
export type PauseEvent = ViewerEventBase & {
  type: "pause";
}

/**
 * Fired when the timepoint changes.
 */
export type TimeUpdateEvent = ViewerEventBase & {
  type: "time_update";
  time: number;
}

/**
 * Fired when a different timeline is selected.
 */
export type TimelineChangeEvent = ViewerEventBase & {
  type: "timeline_change";
  timeline: string;
  time: number;
}

/**
 * Fired when the selection changes.
 *
 * This event is fired each time any part of the event payload changes,
 * this includes for example clicking on different parts of the same
 * entity in a 2D or 3D view.
 */
export type SelectionChangeEvent = ViewerEventBase & {
  type: "selection_change";
  items: SelectionChangeItem[];
}

/**
 * Fired when a new recording is opened in the Viewer.
 *
 * For `rrd` file or stream, a recording is considered "open" after
 * enough information about the recording, such as its ID and source,
 * is received.
 *
 * Contains some basic information about the origin of the recording.
 */
export type RecordingOpenEvent = ViewerEventBase & {
  type: "recording_open";

  /**
   * Where the recording came from.
   *
   * The value should be considered unstable, which is why we don't
   * list the possible values here.
   */
  source: string;

  /**
   * Version of the SDK used to create this recording.
   *
   * Uses semver format.
   */
  version?: string;
}

// A bit of TypeScript metaprogramming to automatically produce a
// mapping of event names to event payloads given the above type
// definitions.

// Yield the event with type `K`.
type _GetViewerEvent<K> =
  Extract<ViewerEvent, { type: K }>;

// `ViewerEvent` is a union of all events, so its `type` field
// is a union of all `type` fields.
type _ViewerEventNames = ViewerEvent["type"];

// For every event, get its payload type.
type ViewerEventMap = {
  [K in _ViewerEventNames]: _GetViewerEvent<K>
}

/**
 * Selected an entity, or an instance of an entity.
 *
 * If the entity was selected within a view, then this also
 * includes the view's name.
 *
 * If the entity was selected within a 2D or 3D space view,
 * then this also includes the position.
 */
export type EntityItem = {
  type: "entity";

  entity_path: string;
  instance_id?: number;
  view_name?: string;
  position?: [number, number, number];
};

/** Selected a view. */
export type ViewItem = { type: "view"; view_id: string; view_name: string };

/** Selected a container. */
export type ContainerItem = {
  type: "container";
  container_id: string;
  container_name: string;
};

/** A single item in a selection. */
export type SelectionChangeItem = EntityItem | ViewItem | ContainerItem;

interface FullscreenOptions {
  get_state: () => boolean;
  on_toggle: () => void;
}

export interface WebViewerEvents extends ViewerEventMap {
  fullscreen: boolean;
  ready: void;
}

// This abomination is a mapped type with key filtering, and is used to split the events
// into those which take no value in their callback, and those which do.
// https://www.typescriptlang.org/docs/handbook/2/mapped-types.html#key-remapping-via-as
export type EventsWithValue = {
  [K in keyof WebViewerEvents as WebViewerEvents[K] extends void
  ? never
  : K]: WebViewerEvents[K] extends any[]
  ? WebViewerEvents[K]
  : [WebViewerEvents[K]];
};

export type EventsWithoutValue = {
  [K in keyof WebViewerEvents as WebViewerEvents[K] extends void
  ? K
  : never]: WebViewerEvents[K];
};

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Resolve a potentially relative URL against the current origin. */
function resolveAbsoluteUrl(url: string): string {
  return new URL(url, window.location.href).toString();
}

/**
 * Rerun Web Viewer
 *
 * ```ts
 * const viewer = new WebViewer();
 * await viewer.start();
 * ```
 *
 * Data may be provided to the Viewer as:
 * - An HTTP file URL, e.g. `viewer.start("https://app.rerun.io/version/0.38.1/examples/dna.rrd")`
 * - A Rerun gRPC URL, e.g. `viewer.start("rerun+http://127.0.0.1:9876/proxy")`
 * - A stream of log messages, via {@link WebViewer.open_channel}.
 *
 * Callbacks may be attached for various events using {@link WebViewer.on}:
 *
 * ```ts
 * viewer.on("time_update", (time) => console.log(`current time: {time}`));
 * ```
 *
 * For the full list of available events, see {@link ViewerEvent}.
 */
export class WebViewer {
  #id = randomId();
  // NOTE: Using the handle requires wrapping all calls to its methods in try/catch.
  //       On failure, call `this.stop` to prevent a memory leak, then re-throw the error.
  #handle: WebHandle | null = null;
  #canvas: HTMLCanvasElement | null = null;
  #loader: HTMLDivElement | null = null;
  #state: "ready" | "starting" | "stopped" = "stopped";
  #fullscreen = false;
  #allow_fullscreen = false;

  constructor() {
    injectStyle();
    setupGlobalEventListeners();
  }

  /**
   * Start the viewer.
   *
   * @param rrd URLs to `.rrd` files or gRPC connections to our SDK.
   * @param parent The element to attach the canvas onto.
   * @param options Web Viewer configuration.
   */
  async start(
    rrd: string | string[] | null,
    parent: HTMLElement | null,
    options: WebViewerOptions | null,
  ): Promise<void> {
    parent ??= document.body;
    options ??= {};
    options = options ? { ...options } : options;

    this.#allow_fullscreen = options.allow_fullscreen || false;

    if (this.#state !== "stopped") return;
    this.#state = "starting";
    this.#clearLoader();

    this.#canvas = document.createElement("canvas");
    this.#canvas.style.width = options.width ?? "640px";
    this.#canvas.style.height = options.height ?? "360px";
    parent.append(this.#canvas);

    // Show loading progress bar
    this.#loader = document.createElement("div");
    this.#loader.innerHTML = `
      <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; background-color: #1c1c1c; font-family: sans-serif; color: white;">
        <div style="margin-bottom: 16px;">Loading Rerun\u2026</div>
        <div style="width: 200px;">
          <div style="background: #333; border-radius: 4px; height: 6px; overflow: hidden;">
            <div class="rerun-progress-bar" style="background: white; height: 100%; width: 0%; transition: width 0.2s;"></div>
          </div>
          <div class="rerun-progress-text" style="margin-top: 6px; font-size: 12px; color: #999;"></div>
        </div>
      </div>
    `;
    this.#loader.style.position = "absolute";
    this.#loader.style.inset = "0";
    parent.style.position = "relative";
    parent.append(this.#loader);

    const progress_bar = this.#loader.querySelector(".rerun-progress-bar") as HTMLElement;
    const progress_text = this.#loader.querySelector(".rerun-progress-text") as HTMLElement;

    const on_progress = (received: number, total: number | null) => {
      if (total != null && total > 0) {
        const pct = Math.min((received / total) * 100, 100);
        progress_bar.style.width = pct.toFixed(1) + "%";
        progress_text.textContent = `${Math.round(pct)}%`;
      } else {
        progress_text.textContent = format_mib(received);
      }
    };

    // This yield appears to be necessary to ensure that the canvas is attached to the DOM
    // and visible. Without it we get occasionally get a panic about a failure to find a canvas
    // element with the given ID.
    await delay(0);

    let base_url: string | undefined = (options as any)?.base_url;
    if (base_url) {
      delete (options as any).base_url;
    }

    let WebHandle_class: typeof wasm_bindgen.WebHandle;
    try {
      WebHandle_class = await load(base_url, on_progress);
    } catch (e) {
      this.#clearLoader();
      this.#fail("Failed to load rerun", String(e));
      throw e;
    }
    if (this.#state !== "starting") {
      this.#clearLoader();
      return;
    }

    const fullscreen = this.#allow_fullscreen
      ? {
        get_state: () => this.#fullscreen,
        on_toggle: () => this.toggle_fullscreen(),
      }
      : undefined;

    const on_viewer_event = (event_json: string) => {
      // for notebooks/gradio, we can avoid a whole layer
      // of serde by sending over the raw json directly,
      // which will be deserialized in Python instead
      this.#dispatch_raw_event(event_json);

      // for JS users, we dispatch the parsed event
      let event: ViewerEvent = JSON.parse(event_json);
      this.#dispatch_event(
        event.type as any,
        event,
      );
    }

    const login = options.login
      ? {
          signed_in_url: resolveAbsoluteUrl(options.login.signed_in_url),
          signed_out_url: resolveAbsoluteUrl(options.login.signed_out_url),
        }
      : undefined;

    this.#handle = new WebHandle_class({
      ...options,
      login,
      fullscreen,
      on_viewer_event,
    });
    try {
      await this.#handle.start(this.#canvas);
    } catch (e) {
      this.#clearLoader();
      this.#fail("Failed to start", String(e));
      throw e;
    }
    if (this.#state !== "starting") {
      this.#clearLoader();
      return;
    }

    this.#clearLoader();
    this.#state = "ready";
    this.#dispatch_event("ready");

    if (rrd) {
      this.open(rrd);
    }

    let self = this;

    function check_for_panic() {
      if (self.#handle?.has_panicked()) {
        self.#fail("Rerun has crashed.", self.#handle?.panic_message());
      } else {
        let delay_ms = 1000;
        setTimeout(check_for_panic, delay_ms);
      }
    }

    check_for_panic();

    return;
  }

  #raw_events: Set<(event_json: string) => void> = new Set();
  #dispatch_raw_event(event_json: string) {
    for (const callback of this.#raw_events) {
      callback(event_json);
    }
  }

  /** Internal interface */
  // NOTE: Callbacks passed to this function must NOT invoke any viewer methods!
  //       The `setTimeout` is omitted to avoid the 1-tick delay, as it is unnecessary,
  //       because this is only meant to be used for sending events to Jupyter/Gradio.
  //
  // Do not change this without searching for grepping for usage!
  private _on_raw_event(callback: (event: string) => void): () => void {
    this.#raw_events.add(callback);
    return () => this.#raw_events.delete(callback);
  }

  #event_map: Map<
    keyof WebViewerEvents,
    Map<(...args: any[]) => void, { once: boolean }>
  > = new Map();

  #dispatch_event<E extends keyof EventsWithValue>(
    event: E,
    ...args: EventsWithValue[E]
  ): void;
  #dispatch_event<E extends keyof EventsWithoutValue>(event: E): void;
  #dispatch_event(event: any, ...args: any[]): void {
    // Dispatch events on next tick.
    // This is necessary because we may have been called somewhere deep within the viewer's call stack,
    // which means that `app` may be locked. The event will not actually be dispatched until the
    // full call stack has returned or the current task has yielded to the event loop. It does not
    // guarantee that we will be able to acquire the lock here, but it makes it a lot more likely.
    setTimeout(() => {
      const callbacks = this.#event_map.get(event);
      if (callbacks) {
        for (const [callback, { once }] of [...callbacks.entries()]) {
          callback(...args);
          if (once) callbacks.delete(callback);
        }
      }
    }, 0);
  }

  /**
   * Register an event listener.
   *
   * Returns a function which removes the listener when called.
   *
   * See {@link ViewerEvent} for a full list of available events.
   */
  on<E extends keyof EventsWithValue>(
    event: E,
    callback: (...args: EventsWithValue[E]) => void,
  ): () => void;
  on<E extends keyof EventsWithoutValue>(
    event: E,
    callback: () => void,
  ): () => void;
  on(event: any, callback: any): () => void {
    const callbacks = this.#event_map.get(event) ?? new Map();
    callbacks.set(callback, { once: false });
    this.#event_map.set(event, callbacks);
    return () => callbacks.delete(callback);
  }

  /**
   * Register an event listener which runs only once.
   *
   * Returns a function which removes the listener when called.
   *
   * See {@link ViewerEvent} for a full list of available events.
   */
  once<E extends keyof EventsWithValue>(
    event: E,
    callback: (value: EventsWithValue[E]) => void,
  ): () => void;
  once<E extends keyof EventsWithoutValue>(
    event: E,
    callback: () => void,
  ): () => void;
  once(event: any, callback: any): () => void {
    const callbacks = this.#event_map.get(event) ?? new Map();
    callbacks.set(callback, { once: true });
    this.#event_map.set(event, callbacks);
    return () => callbacks.delete(callback);
  }

  /**
   * Unregister an event listener.
   *
   * The event emitter relies on referential equality to store callbacks.
   * The `callback` passed in must be the exact same _instance_ of the function passed in to `on` or `once`.
   *
   * See {@link ViewerEvent} for a full list of available events.
   */
  off<E extends keyof EventsWithValue>(
    event: E,
    callback: (value: EventsWithValue[E]) => void,
  ): void;
  off<E extends keyof EventsWithoutValue>(event: E, callback: () => void): void;
  off(event: any, callback: any): void {
    const callbacks = this.#event_map.get(event);
    if (callbacks) {
      callbacks.delete(callback);
    } else {
      console.warn(
        "Attempted to call `WebViewer.off` with an unregistered callback. Are you passing in the same function instance?",
      );
    }
  }

  /**
   * The underlying canvas element.
   */
  get canvas() {
    return this.#canvas;
  }

  /**
   * Returns `true` if the viewer is ready to connect to data sources.
   */
  get ready() {
    return this.#state === "ready";
  }

  /**
   * Open a recording.
   *
   * The viewer must have been started via {@link WebViewer.start}.
   *
   * @param rrd URLs to `.rrd` files or gRPC connections to our SDK.
   */
  open(rrd: string | string[]) {
    if (!this.#handle) {
      throw new Error(`attempted to open \`${rrd}\` in a stopped viewer`);
    }

    const urls = Array.isArray(rrd) ? rrd : [rrd];
    for (const url of urls) {
      try {
        this.#handle.add_receiver(url);
      } catch (e) {
        this.#fail("Failed to open recording", String(e));
        throw e;
      }
    }
  }

  /**
   * Close a recording.
   *
   * The viewer must have been started via {@link WebViewer.start}.
   *
   * @param rrd URLs to `.rrd` files or gRPC connections to our SDK.
   */
  close(rrd: string | string[]) {
    if (!this.#handle) {
      throw new Error(`attempted to close \`${rrd}\` in a stopped viewer`);
    }

    const urls = Array.isArray(rrd) ? rrd : [rrd];
    for (const url of urls) {
      try {
        this.#handle.remove_receiver(url);
      } catch (e) {
        this.#fail("Failed to close recording", String(e));
        throw e;
      }
    }
  }

  /**
   * Save the active recording as an `.rrd` byte stream.
   *
   * The snapshot is selected when this method is called.
   * Concatenate all stream byte sequences to obtain one valid file.
   * An individual sequence is not independently accepted by {@link WebViewer.open_channel}.
   *
   * @throws If the viewer is stopped or has no active recording.
   */
  save_recording(): ReadableStream<Uint8Array> {
    if (!this.#handle) {
      throw new Error(`attempted to save recording in a stopped web viewer`);
    }

    const bytes = this.#handle.save_recording();
    return export_stream(bytes);
  }

  /**
   * Save the active blueprint as an `.rbl` byte stream.
   *
   * The snapshot is selected when this method is called.
   * Concatenate all stream byte sequences to obtain one valid file.
   * An individual sequence is not independently accepted by {@link WebViewer.open_channel}.
   *
   * @throws If the viewer is stopped or has no active blueprint.
   */
  save_blueprint(): ReadableStream<Uint8Array> {
    if (!this.#handle) {
      throw new Error(`attempted to save blueprint in a stopped web viewer`);
    }

    const bytes = this.#handle.save_blueprint();
    return export_stream(bytes);
  }

  /**
   * Stop the viewer, freeing all associated memory.
   *
   * The same viewer instance may be started multiple times.
   */
  stop() {
    if (this.#state === "stopped") return;
    if (this.#allow_fullscreen && this.#canvas && this.#fullscreen) {
      this.#minimize();
    }

    this.#state = "stopped";

    this.#canvas?.remove();
    this.#clearLoader();

    try {
      this.#handle?.destroy();
      this.#handle?.free();
    } catch (e) {
      this.#handle = null;
      throw e;
    }

    this.#canvas = null;
    this.#handle = null;
    this.#loader = null;
    this.#fullscreen = false;
    this.#allow_fullscreen = false;
  }

  #fail(message: string, error_message?: string) {
    console.error("WebViewer failure:", message, error_message);
    if (this.canvas?.parentElement) {
      const parent = this.canvas.parentElement;
      parent.innerHTML = `
        <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; color: white; font-family: sans-serif; background-color: #1c1c1c;">
          <h1 class="rerun-fail-message"></h1>
          <pre class="rerun-fail-error" style="text-align: left; white-space: pre-wrap; word-break: break-word; max-width: 90vw;"></pre>
          <button class="rerun-fail-clear-cache">Clear caches and reload</button>
        </div>
      `;

      parent.querySelector(".rerun-fail-message")!.textContent = message;

      const errorEl = parent.querySelector(".rerun-fail-error")!;
      if (error_message) {
        errorEl.textContent = error_message;
      } else {
        errorEl.remove();
      }

      parent.querySelector(".rerun-fail-clear-cache")!.addEventListener("click", async () => {
        if ("caches" in window) {
          const keys = await caches.keys();
          await Promise.all(keys.map((key) => caches.delete(key)));
        }
        window.location.reload();
      });
    }

    this.stop();
  }

  #clearLoader() {
    this.#loader?.remove();
    this.#loader = null;
  }

  /**
   * Opens a new channel for sending log messages.
   *
   * The channel can be used to incrementally push `rrd` chunks into the viewer.
   *
   * @param channel_name used to identify the channel.
   */
  open_channel(channel_name: string = "rerun-io/web-viewer"): LogChannel {
    if (!this.#handle) {
      throw new Error(
        `attempted to open channel \"${channel_name}\" in a stopped web viewer`,
      );
    }

    const id = crypto.randomUUID();

    try {
      this.#handle.open_channel(id, channel_name);
    } catch (e) {
      this.#fail("Failed to open channel", String(e));
      throw e;
    }

    const on_send = (/** @type {Uint8Array} */ data: Uint8Array) => {
      if (!this.#handle) {
        throw new Error(
          `attempted to send data through channel \"${channel_name}\" to a stopped web viewer`,
        );
      }

      try {
        this.#handle.send_rrd_to_channel(id, data);
      } catch (e) {
        this.#fail("Failed to send data", String(e));
        throw e;
      }
    };

    const on_send_table = (/** @type {Uint8Array} */ data: Uint8Array) => {
      if (!this.#handle) {
        throw new Error(
          `attempted to send data through channel \"${channel_name}\" to a stopped web viewer`,
        );
      }

      try {
        this.#handle.send_table_to_channel(id, data);
      } catch (e) {
        this.#fail("Failed to send table", String(e));
        throw e;
      }
    }

    const on_close = () => {
      if (!this.#handle) {
        throw new Error(
          `attempted to send data through channel \"${channel_name}\" to a stopped web viewer`,
        );
      }

      try {
        this.#handle.close_channel(id);
      } catch (e) {
        this.#fail("Failed to close channel", String(e));
        throw e;
      }
    };

    const get_state = () => this.#state;

    return new LogChannel(on_send, on_send_table, on_close, get_state);
  }

  /**
   * Force a panel to a specific state.
   *
   * @param panel which panel to configure
   * @param state which state to force the panel into
   */
  override_panel_state(panel: Panel, state: PanelState | undefined | null) {
    if (!this.#handle) {
      throw new Error(
        `attempted to set ${panel} panel to ${state} in a stopped web viewer`,
      );
    }

    try {
      this.#handle.override_panel_state(panel, state);
    } catch (e) {
      this.#fail("Failed to override panel state", String(e));
      throw e;
    }
  }

  /**
   * Toggle panel overrides set via `override_panel_state`.
   *
   * @param value set to a specific value. Toggles the previous value if not provided.
   */
  toggle_panel_overrides(value?: boolean | null) {
    if (!this.#handle) {
      throw new Error(
        `attempted to toggle panel overrides in a stopped web viewer`,
      );
    }

    try {
      this.#handle.toggle_panel_overrides(value as boolean | undefined);
    } catch (e) {
      this.#fail("Failed to toggle panel overrides", String(e));
      throw e;
    }
  }

  /**
   * Get the active recording id.
   */
  get_active_recording_id(): string | null {
    if (!this.#handle) {
      throw new Error(
        `attempted to get active recording id in a stopped web viewer`,
      );
    }

    return this.#handle.get_active_recording_id() ?? null;
  }

  /**
   * Set the active recording id.
   *
   * This is the same as clicking on the recording in the Viewer's left panel.
   */
  set_active_recording_id(value: string) {
    if (!this.#handle) {
      throw new Error(
        `attempted to set active recording id to ${value} in a stopped web viewer`,
      );
    }

    this.#handle.set_active_recording_id(value);
  }

  /**
   * What the viewer currently shows.
   *
   * Covers the active recording, every open recording with its timelines, their time ranges and
   * its time cursor, and every view of the current blueprint with the warnings and errors it reports.
   *
   * This API is experimental, and may change between releases.
   */
  async get_state(): Promise<ViewerState> {
    return viewer_state_of(await this.#viewer_control("getViewerState", {}));
  }

  /**
   * Move the time cursor.
   *
   * ```ts
   * await viewer.set_time({ timeline: "frame", sequence: 42, play: true });
   * ```
   *
   * Rejects with a {@link ViewerControlError} with the code `INVALID_ARGUMENT` if the recording or timeline doesn't exist.
   *
   * This API is experimental, and may change between releases.
   */
  async set_time(options: SetTimeOptions): Promise<void> {
    await this.#viewer_control("setTimeCursor", {
      storeId: options.recording,
      timeline: options.timeline === undefined ? undefined : { name: options.timeline },
      time: { time: time_of(options) },
      play: options.play ?? false,
    });
  }

  /**
   * Where the viewer is: a recording, a catalog entry, the settings, …
   *
   * Pass it to {@link WebViewer.set_location} to come back to it later.
   * This is a Viewer URL, not the address of the web page.
   * A recording is reported as `about:recording?app_id=…&recording_id=…`.
   *
   * This API is experimental, and may change between releases.
   */
  async get_location(): Promise<string> {
    return (await this.#viewer_control("getLocation", {})).location;
  }

  /**
   * Show a location the viewer already has open.
   *
   * Takes a location from {@link WebViewer.get_location}, or the `location` of a recording from {@link WebViewer.get_state}.
   * This only navigates, and never loads anything: use {@link WebViewer.open} for that.
   *
   * Rejects with a {@link ViewerControlError} with the code `NOT_FOUND` if the viewer does not have it open,
   * and `INVALID_ARGUMENT` if it is not a location.
   *
   * This API is experimental, and may change between releases.
   */
  async set_location(location: string): Promise<void> {
    await this.#viewer_control("setLocation", { location });
  }

  /**
   * Get the play state.
   *
   * This always returns `false` if the recording can't be found.
   */
  get_playing(recording_id: string): boolean {
    if (!this.#handle) {
      throw new Error(`attempted to get play state in a stopped web viewer`);
    }

    return this.#handle.get_playing(recording_id) || false;
  }

  /**
   * Set the play state.
   *
   * This does nothing if the recording can't be found.
   */
  set_playing(recording_id: string, value: boolean) {
    if (!this.#handle) {
      throw new Error(
        `attempted to set play state to ${value ? "playing" : "paused"
        } in a stopped web viewer`,
      );
    }

    this.#handle.set_playing(recording_id, value);
  }

  /**
   * Get the current time.
   *
   * The interpretation of time depends on what kind of timeline it is:
   *
   * - For time timelines, this is the time in nanoseconds.
   * - For sequence timelines, this is the sequence number.
   *
   * This always returns `0` if the recording or timeline can't be found.
   */
  get_current_time(recording_id: string, timeline: string): number {
    if (!this.#handle) {
      throw new Error(`attempted to get current time in a stopped web viewer`);
    }

    return this.#handle.get_time_for_timeline(recording_id, timeline) || 0;
  }

  /**
   * Set the current time.
   *
   * Equivalent to clicking on the timeline in the time panel at the specified `time`.
   * The interpretation of `time` depends on what kind of timeline it is:
   *
   * - For time timelines, this is the time in nanoseconds.
   * - For sequence timelines, this is the sequence number.
   *
   * This does nothing if the recording or timeline can't be found.
   */
  set_current_time(recording_id: string, timeline: string, time: number) {
    if (!this.#handle) {
      throw new Error(
        `attempted to set current time to ${time} in a stopped web viewer`,
      );
    }

    this.#handle.set_time_for_timeline(recording_id, timeline, time);
  }

  /**
   * Get the active timeline.
   *
   * This always returns `null` if the recording can't be found.
   */
  get_active_timeline(recording_id: string): string | null {
    if (!this.#handle) {
      throw new Error(
        `attempted to get active timeline in a stopped web viewer`,
      );
    }

    return this.#handle.get_active_timeline(recording_id) ?? null;
  }

  /**
   * Set the active timeline.
   *
   * This does nothing if the recording or timeline can't be found.
   */
  set_active_timeline(recording_id: string, timeline: string) {
    if (!this.#handle) {
      throw new Error(
        `attempted to set active timeline to ${timeline} in a stopped web viewer`,
      );
    }

    this.#handle.set_active_timeline(recording_id, timeline);
  }

  /**
   * Get the time range for a timeline.
   *
   * This always returns `null` if the recording or timeline can't be found.
   */
  get_time_range(
    recording_id: string,
    timeline: string,
  ): { min: number; max: number } | null {
    if (!this.#handle) {
      throw new Error(`attempted to get time range in a stopped web viewer`);
    }

    return this.#handle.get_timeline_time_range(recording_id, timeline);
  }

  /**
   * Toggle fullscreen mode.
   *
   * This does nothing if `allow_fullscreen` was not set to `true` when starting the viewer.
   *
   * Fullscreen mode works by updating the underlying `<canvas>` element's `style`:
   * - `position` to `fixed`
   * - width/height/top/left to cover the entire viewport
   *
   * When fullscreen mode is toggled off, the style is restored to its previous values.
   *
   * When fullscreen mode is toggled on, any other instance of the viewer on the page
   * which is already in fullscreen mode is toggled off. This means that it doesn't
   * have to be tracked manually.
   *
   * This functionality can also be directly accessed in the viewer:
   * - The maximize/minimize top panel button
   * - The `Toggle fullscreen` UI command (accessible via the command palette, CTRL+P)
   */
  toggle_fullscreen() {
    if (!this.#allow_fullscreen) return;

    if (!this.#handle || !this.#canvas) {
      throw new Error(
        `attempted to toggle fullscreen mode in a stopped web viewer`,
      );
    }

    if (this.#fullscreen) {
      this.#minimize();
    } else {
      this.#maximize();
    }
  }

  set_credentials(access_token: string, email: string) {
    if (!this.#handle) {
      throw new Error(
        `attempted to set credentials in a stopped web viewer`,
      );
    }
    this.#handle.set_credentials(access_token, email);
  }



  #minimize = () => { };

  #maximize = () => {
    _minimize_current_fullscreen_viewer?.();

    const canvas = this.#canvas!;
    const rect = canvas.getBoundingClientRect();

    const sync_style_to_rect = () => {
      canvas.style.left = rect.left + "px";
      canvas.style.top = rect.top + "px";
      canvas.style.width = rect.width + "px";
      canvas.style.height = rect.height + "px";
    };
    const undo_style = () => canvas.removeAttribute("style");
    const transition = (callback: () => void) =>
      setTimeout(() => requestAnimationFrame(callback), transition_delay_ms);

    canvas.classList.add(classes.fullscreen_base, classes.fullscreen_rect);
    sync_style_to_rect();
    requestAnimationFrame(() => {
      if (!this.#fullscreen) return;
      canvas.classList.add(classes.transition);
      transition(() => {
        if (!this.#fullscreen) return;
        undo_style();

        document.body.classList.add(classes.hide_scrollbars);
        document.documentElement.classList.add(classes.hide_scrollbars);
        this.#dispatch_event("fullscreen", true);
      });
    });

    this.#minimize = () => {
      document.body.classList.remove(classes.hide_scrollbars);
      document.documentElement.classList.remove(classes.hide_scrollbars);

      sync_style_to_rect();
      canvas.classList.remove(classes.fullscreen_rect);
      transition(() => {
        if (this.#fullscreen) return;

        undo_style();
        canvas.classList.remove(classes.fullscreen_base, classes.transition);
      });

      _minimize_current_fullscreen_viewer = null;
      this.#fullscreen = false;
      this.#dispatch_event("fullscreen", false);
    };

    _minimize_current_fullscreen_viewer = () => this.#minimize();
    this.#fullscreen = true;
  };

  /**
   * Perform one operation of the viewer's control protocol, defined by `viewer_control.proto`.
   *
   * A failed operation rejects with a {@link ViewerControlError}.
   */
  async #viewer_control<K extends Operation>(
    operation: K,
    request: OperationRequest<K>,
  ): Promise<OperationResponse<K>> {
    if (!this.#handle) {
      throw new ViewerControlError("FAILED_PRECONDITION", "attempted to control a stopped web viewer");
    }

    const kind = { case: operation, value: request } as MessageInitShape<
      typeof ViewerControlRequestSchema
    >["kind"];
    const request_bytes = toBinary(
      ViewerControlRequestSchema,
      create(ViewerControlRequestSchema, { kind }),
    );
    let response_bytes;
    try {
      response_bytes = await this.#handle.viewer_control(request_bytes);
    } catch (err) {
      throw as_viewer_control_error(err);
    }
    const response = fromBinary(ViewerControlResponseSchema, response_bytes);

    if (response.kind.case !== operation) {
      throw new Error(`Expected a ${operation} response, got ${response.kind.case}`);
    }
    return response.kind.value as OperationResponse<K>;
  }
}

export class LogChannel {
  #on_send;
  #on_send_table;
  #on_close;
  #get_state;
  #closed = false;

  /**
   * @param on_send
   * @param on_close
   * @param get_state
   */
  constructor(
    on_send: (data: Uint8Array) => void,
    on_send_table: (data: Uint8Array) => void,
    on_close: () => void,
    get_state: () => "ready" | "starting" | "stopped",
  ) {
    this.#on_send = on_send;
    this.#on_send_table = on_send_table;
    this.#on_close = on_close;
    this.#get_state = get_state;
  }

  get ready() {
    return !this.#closed && this.#get_state() === "ready";
  }

  /**
   * Send an `rrd` containing log messages to the viewer.
   *
   * Does nothing if `!this.ready`.
   *
   * @param rrd_bytes Is an rrd file stored in a byte array, received via some other side channel.
   */
  send_rrd(rrd_bytes: Uint8Array) {
    if (!this.ready) return;
    this.#on_send(rrd_bytes);
  }

  send_table(table_bytes: Uint8Array) {
    if (!this.ready) return;
    this.#on_send_table(table_bytes)
  }

  /**
   * Close the channel.
   *
   * Does nothing if `!this.ready`.
   */
  close() {
    if (!this.ready) return;
    this.#on_close();
    this.#closed = true;
  }
}

const classes = {
  hide_scrollbars: "rerun-viewer-hide-scrollbars",
  fullscreen_base: "rerun-viewer-fullscreen-base",
  fullscreen_rect: "rerun-viewer-fullscreen-rect",
  transition: "rerun-viewer-transition",
};

const transition_delay_ms = 100;

const css = `
  html.${classes.hide_scrollbars},
  body.${classes.hide_scrollbars} {
    scrollbar-gutter: auto !important;
    overflow: hidden !important;
  }

  .${classes.fullscreen_base} {
    position: fixed;
    z-index: 99999;
  }

  .${classes.transition} {
    transition: all ${transition_delay_ms / 1000}s linear;
  }

  .${classes.fullscreen_rect} {
    left: 0;
    top: 0;
    width: 100%;
    height: 100%;
  }
`;

function injectStyle() {
  const ID = "__rerun_viewer_style";

  if (document.getElementById(ID)) {
    // already injected
    return;
  }

  const style = document.createElement("style");
  style.id = ID;
  style.appendChild(document.createTextNode(css));
  document.head.appendChild(style);
}

function setupGlobalEventListeners() {
  window.addEventListener("keyup", (e) => {
    if (e.code === "Escape") {
      _minimize_current_fullscreen_viewer?.();
    }
  });
}
