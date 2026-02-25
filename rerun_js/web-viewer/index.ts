// @ts-ignore
import type { WebHandle, wasm_bindgen } from "./re_viewer";

let get_wasm_bindgen: (() => typeof wasm_bindgen) | null = null;
let _wasm_module: WebAssembly.Module | null = null;

async function fetch_viewer_js(base_url?: string): Promise<(() => typeof wasm_bindgen)> {
  // @ts-ignore
  return (await import("./re_viewer")).default;
}

async function fetch_viewer_wasm(base_url?: string): Promise<Response> {
  //!<INLINE-MARKER-OPEN>
  if (base_url) {
    return fetch(new URL("./re_viewer_bg.wasm", base_url))
  } else {
    return fetch(new URL("./re_viewer_bg.wasm", import.meta.url));
  }
  //!<INLINE-MARKER-CLOSE>
}

async function load(base_url?: string): Promise<typeof wasm_bindgen.WebHandle> {
  // instantiate wbg globals+module for every invocation of `load`,
  // but don't load the JS/Wasm source every time
  if (!get_wasm_bindgen || !_wasm_module) {
    [get_wasm_bindgen, _wasm_module] = await Promise.all([
      fetch_viewer_js(base_url),
      WebAssembly.compileStreaming(fetch_viewer_wasm(base_url)),
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

  /** If set to `true`, hides the welcome screen, which contains our examples. Defaults to `false`. */
  hide_welcome_screen?: boolean;

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

// Types are based on `crates/viewer/re_viewer/src/event.rs`.
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

/**
 * Rerun Web Viewer
 *
 * ```ts
 * const viewer = new WebViewer();
 * await viewer.start();
 * ```
 *
 * Data may be provided to the Viewer as:
 * - An HTTP file URL, e.g. `viewer.start("https://app.rerun.io/version/0.30.0-rc.2/examples/dna.rrd")`
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

    this.#canvas = document.createElement("canvas");
    this.#canvas.style.width = options.width ?? "640px";
    this.#canvas.style.height = options.height ?? "360px";
    parent.append(this.#canvas);

    // Show loading spinner
    const loader = document.createElement("div");
    loader.id = "rerun-loader";
    loader.innerHTML = `
      <style>
        @keyframes rerun-spin { to { transform: rotate(360deg); } }
      </style>
      <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; background-color: #1c1c1c; font-family: sans-serif; color: white;">
        <div style="width: 40px; height: 40px; border: 3px solid #444; border-top-color: white; border-radius: 50%; animation: rerun-spin 1s linear infinite;"></div>
        <div style="margin-top: 16px;">Loading Rerun…</div>
      </div>
    `;
    loader.style.position = "absolute";
    loader.style.inset = "0";
    parent.style.position = "relative";
    parent.append(loader);

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
      WebHandle_class = await load(base_url);
    } catch (e) {
      loader.remove();
      this.#fail("Failed to load rerun", String(e));
      throw e;
    }
    if (this.#state !== "starting") return;

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

    this.#handle = new WebHandle_class({
      ...options,
      fullscreen,
      on_viewer_event,
    });
    try {
      await this.#handle.start(this.#canvas);
    } catch (e) {
      loader.remove();
      this.#fail("Failed to start", String(e));
      throw e;
    }
    if (this.#state !== "starting") return;

    loader.remove();
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
   * @param options
   *        - follow_if_http: Whether Rerun should open the resource in "Following" mode when streaming
   *        from an HTTP url. Defaults to `false`. Ignored for non-HTTP URLs.
   */
  open(rrd: string | string[], options: { follow_if_http?: boolean } = {}) {
    if (!this.#handle) {
      throw new Error(`attempted to open \`${rrd}\` in a stopped viewer`);
    }

    const urls = Array.isArray(rrd) ? rrd : [rrd];
    for (const url of urls) {
      try {
        this.#handle.add_receiver(url, options.follow_if_http);
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

    try {
      this.#handle?.destroy();
      this.#handle?.free();
    } catch (e) {
      this.#handle = null;
      throw e;
    }

    this.#canvas = null;
    this.#handle = null;
    this.#fullscreen = false;
    this.#allow_fullscreen = false;
  }

  #fail(message: string, error_message?: string) {
    console.error("WebViewer failure:", message, error_message);
    if (this.canvas?.parentElement) {
      const parent = this.canvas.parentElement;
      parent.innerHTML = `
        <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; color: white; font-family: sans-serif; background-color: #1c1c1c;">
          <h1 id="fail-message"></h1>
          <pre id="fail-error" style="text-align: left;"></pre>
          <button id="fail-clear-cache">Clear caches and reload</button>
        </div>
      `;

      document.getElementById("fail-message")!.textContent = message;

      const errorEl = document.getElementById("fail-error")!;
      if (error_message) {
        errorEl.textContent = error_message;
      } else {
        errorEl.remove();
      }

      document.getElementById("fail-clear-cache")!.addEventListener("click", async () => {
        if ("caches" in window) {
          const keys = await caches.keys();
          await Promise.all(keys.map((key) => caches.delete(key)));
        }
        window.location.reload();
      });
    }

    this.stop();
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
