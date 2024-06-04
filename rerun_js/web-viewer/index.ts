import type { WebHandle } from "./re_viewer.js";

interface AppOptions {
  url?: string;
  manifest_url?: string;
  render_backend?: Backend;
  hide_welcome_screen?: boolean;
  panel_state_overrides?: Partial<{
    [K in Panel]: PanelState;
  }>;
  fullscreen?: FullscreenOptions;
}

type WebHandleConstructor = {
  new (app_options?: AppOptions): WebHandle;
};

let WebHandleConstructor: WebHandleConstructor | null = null;

async function load(): Promise<WebHandleConstructor> {
  if (WebHandleConstructor) {
    return WebHandleConstructor;
  }
  WebHandleConstructor = (await import("./re_viewer.js")).WebHandle;
  return WebHandleConstructor;
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
export type CanvasRect = {
  width: string;
  height: string;
  top: string;
  left: string;
  bottom: string;
  right: string;
};

export type CanvasStyle = {
  canvas: CanvasRect & { position: string; transition: string; zIndex: string };
  document: {
    body: { overflow: string; scrollbarGutter: string };
    root: { overflow: string; scrollbarGutter: string };
  };
};

type FullscreenOff = { on: false; saved_style: null; saved_rect: null };

type FullscreenOn = { on: true; saved_style: CanvasStyle; saved_rect: DOMRect };

type FullscreenState = FullscreenOff | FullscreenOn;

interface WebViewerOptions {
  manifest_url?: string;
  render_backend?: Backend;
  hide_welcome_screen?: boolean;
  allow_fullscreen?: boolean;
}

interface FullscreenOptions {
  get_state: () => boolean;
  on_toggle: () => void;
}

interface WebViewerEvents {
  fullscreen: boolean;
  ready: void;
}

type EventsWithValue = {
  [K in keyof WebViewerEvents as WebViewerEvents[K] extends void
    ? never
    : K]: WebViewerEvents[K];
};

type EventsWithoutValue = {
  [K in keyof WebViewerEvents as WebViewerEvents[K] extends void
    ? K
    : never]: WebViewerEvents[K];
};

type WithValue<Events> = {
  [K in keyof Events as Events[K] extends void ? never : K]: Events[K];
};

type WithoutValue<Events> = {
  [K in keyof Events as Events[K] extends void ? K : never]: Events[K];
};

type Cancel = () => void;

export class WebViewer {
  #id = randomId();

  #handle: WebHandle | null = null;

  #canvas: HTMLCanvasElement | null = null;

  #state: "ready" | "starting" | "stopped" = "stopped";

  #fullscreen_state: FullscreenState = {
    on: false,
    saved_style: null,
    saved_rect: null,
  };

  #allow_fullscreen = false;

  /**
   * Start the viewer.
   *
   * @param rrd URLs to `.rrd` files or WebSocket connections to our SDK.
   * @param parent The element to attach the canvas onto.
   * @param options Whether to hide the welcome screen.
   */
  async start(
    rrd: string | string[] | null,
    parent: HTMLElement | null,
    options: WebViewerOptions | null,
  ): Promise<void> {
    parent ??= document.body;
    options ??= {};

    this.#allow_fullscreen = options.allow_fullscreen || false;

    if (this.#state !== "stopped") return;
    this.#state = "starting";

    this.#canvas = document.createElement("canvas");
    this.#canvas.id = this.#id;
    parent.append(this.#canvas);

    let WebHandle_class = await load();
    if (this.#state !== "starting") return;

    const fullscreen = this.#allow_fullscreen
      ? {
          get_state: () => this.#fullscreen_state.on,
          on_toggle: () => this.toggle_fullscreen(),
        }
      : undefined;

    this.#handle = new WebHandle_class({ ...options, fullscreen });
    await this.#handle.start(this.#canvas.id);
    if (this.#state !== "starting") return;

    if (this.#handle.has_panicked()) {
      throw new Error(`Web viewer crashed: ${this.#handle.panic_message()}`);
    }

    this.#state = "ready";
    this.#dispatch_event("ready");

    if (rrd) {
      this.open(rrd);
    }

    return;
  }

  #event_map: Map<
    keyof WebViewerEvents,
    Map<(value: any) => void, { once: boolean }>
  > = new Map();

  #dispatch_event<E extends keyof EventsWithValue>(
    event: E,
    value: EventsWithValue[E],
  ): void;
  #dispatch_event<E extends keyof EventsWithoutValue>(event: E): void;
  #dispatch_event(event: any, value?: any): void {
    const callbacks = this.#event_map.get(event);
    if (callbacks) {
      for (const [callback, { once }] of [...callbacks.entries()]) {
        callback(value);
        if (once) callbacks.delete(callback);
      }
    }
  }

  /**
   * Register an event listener.
   *
   * Returns a function which removes the listener when called.
   */
  on<E extends keyof WithValue<WebViewerEvents>>(
    event: E,
    callback: (value: WithValue<WebViewerEvents>[E]) => void,
  ): Cancel;
  on<E extends keyof WithoutValue<WebViewerEvents>>(
    event: E,
    callback: () => void,
  ): Cancel;
  on(event: any, callback: any): Cancel {
    const callbacks = this.#event_map.get(event) ?? new Map();
    callbacks.set(callback, { once: false });
    this.#event_map.set(event, callbacks);
    return () => callbacks.delete(callback);
  }

  /**
   * Register an event listener which runs only once.
   *
   * Returns a function which removes the listener when called.
   */
  once<E extends keyof WithValue<WebViewerEvents>>(
    event: E,
    callback: (value: WithValue<WebViewerEvents>[E]) => void,
  ): Cancel;
  once<E extends keyof WithoutValue<WebViewerEvents>>(
    event: E,
    callback: () => void,
  ): Cancel;
  once(event: any, callback: any): Cancel {
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
   */
  off<E extends keyof WithValue<WebViewerEvents>>(
    event: E,
    callback: (value: WithValue<WebViewerEvents>[E]) => void,
  ): void;
  off<E extends keyof WithoutValue<WebViewerEvents>>(
    event: E,
    callback: () => void,
  ): void;
  off(event: any, callback: any): void {
    const callbacks = this.#event_map.get(event);
    if (callbacks) {
      callbacks.delete(callback);
    } else {
      console.warn(
        "Attempted to call `WebViewer.off` with an unregistered callback. Are you using ",
      );
    }
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
   * The viewer must have been started via `WebViewer.start`.
   *
   * @param rrd URLs to `.rrd` files or WebSocket connections to our SDK.
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
      this.#handle.add_receiver(url, options.follow_if_http);
      if (this.#handle.has_panicked()) {
        throw new Error(`Web viewer crashed: ${this.#handle.panic_message()}`);
      }
    }
  }

  /**
   * Close a recording.
   *
   * The viewer must have been started via `WebViewer.start`.
   *
   * @param rrd URLs to `.rrd` files or WebSocket connections to our SDK.
   */
  close(rrd: string | string[]) {
    if (!this.#handle) {
      throw new Error(`attempted to close \`${rrd}\` in a stopped viewer`);
    }
    const urls = Array.isArray(rrd) ? rrd : [rrd];
    for (const url of urls) {
      this.#handle.remove_receiver(url);
      if (this.#handle.has_panicked()) {
        throw new Error(`Web viewer crashed: ${this.#handle.panic_message()}`);
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
    if (this.#allow_fullscreen && this.#canvas) {
      const state = this.#fullscreen_state;
      if (state.on) this.#minimize(this.#canvas, state);
    }

    this.#state = "stopped";

    this.#canvas?.remove();
    this.#handle?.destroy();
    this.#handle?.free();

    this.#canvas = null;
    this.#handle = null;
    this.#fullscreen_state.on = false;
    this.#allow_fullscreen = false;
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
    this.#handle.open_channel(id, channel_name);
    const on_send = (/** @type {Uint8Array} */ data: Uint8Array) => {
      if (!this.#handle) {
        throw new Error(
          `attempted to send data through channel \"${channel_name}\" to a stopped web viewer`,
        );
      }
      this.#handle.send_rrd_to_channel(id, data);
    };
    const on_close = () => {
      if (!this.#handle) {
        throw new Error(
          `attempted to send data through channel \"${channel_name}\" to a stopped web viewer`,
        );
      }
      this.#handle.close_channel(id);
    };
    const get_state = () => this.#state;
    return new LogChannel(on_send, on_close, get_state);
  }

  /**
   * Force a panel to a specific state.
   *
   * @param panel
   * @param state
   */
  override_panel_state(panel: Panel, state: PanelState) {
    if (!this.#handle) {
      throw new Error(
        `attempted to set ${panel} panel to ${state} in a stopped web viewer`,
      );
    }
    this.#handle.override_panel_state(panel, state);
  }

  /**
   * Toggle panel overrides set via `override_panel_state`.
   *
   * @param value - set to a specific value. Toggles the previous value if not provided.
   */
  toggle_panel_overrides(value?: boolean | null) {
    if (!this.#handle) {
      throw new Error(
        `attempted to toggle panel overrides in a stopped web viewer`,
      );
    }
    this.#handle.toggle_panel_overrides(value as boolean | undefined);
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
   *
   * Note: When toggling fullscreen, panel overrides are also toggled:
   * - Maximize turns panel overrides _off_,
   * - Minimize turns panel overrides to their state before a maximize.
   */
  toggle_fullscreen() {
    if (!this.#allow_fullscreen) return;

    if (!this.#handle || !this.#canvas) {
      throw new Error(
        `attempted to toggle fullscreen mode in a stopped web viewer`,
      );
    }

    const state = this.#fullscreen_state;
    if (state.on) {
      this.#dispatch_event("fullscreen", true);
      this.#minimize(this.#canvas, state);
    } else {
      this.#dispatch_event("fullscreen", false);
      this.#maximize(this.#canvas);
    }
  }

  #minimize = (
    canvas: HTMLCanvasElement,
    { saved_style, saved_rect }: FullscreenOn,
  ) => {
    this.#fullscreen_state = {
      on: false,
      saved_style: null,
      saved_rect: null,
    };

    if (this.#fullscreen_state.on) return;

    canvas.style.width = saved_rect.width + "px";
    canvas.style.height = saved_rect.height + "px";
    canvas.style.top = saved_rect.top + "px";
    canvas.style.left = saved_rect.left + "px";
    canvas.style.bottom = saved_rect.bottom + "px";
    canvas.style.right = saved_rect.right + "px";

    setTimeout(
      () =>
        requestAnimationFrame(() => {
          if (this.#fullscreen_state.on) return;

          // restore saved style
          for (const prop in saved_style.canvas) {
            // @ts-expect-error
            canvas.style[prop] = saved_style.canvas[prop];
          }
          for (const key in saved_style.document) {
            // @ts-expect-error
            for (const prop in saved_style.document[key]) {
              // @ts-expect-error
              document[key].style[prop] = saved_style.document[key][prop];
            }
          }
        }),
      100,
    );

    _minimize_current_fullscreen_viewer = null;
  };

  #maximize = (canvas: HTMLCanvasElement) => {
    _minimize_current_fullscreen_viewer?.();

    const style = canvas.style;

    const saved_style: CanvasStyle = {
      canvas: {
        position: style.position,
        width: style.width,
        height: style.height,
        top: style.top,
        left: style.left,
        bottom: style.bottom,
        right: style.right,
        transition: style.transition,
        zIndex: style.zIndex,
      },
      document: {
        body: {
          overflow: document.body.style.overflow,
          scrollbarGutter: document.body.style.scrollbarGutter,
        },
        root: {
          overflow: document.documentElement.style.overflow,
          scrollbarGutter: document.documentElement.style.scrollbarGutter,
        },
      },
    };
    const saved_rect = canvas.getBoundingClientRect();

    style.position = "fixed";
    style.width = saved_rect.width + "px";
    style.height = saved_rect.height + "px";
    style.top = saved_rect.top + "px";
    style.left = saved_rect.left + "px";
    style.bottom = saved_rect.bottom + "px";
    style.right = saved_rect.right + "px";
    style.zIndex = "99999";
    style.transition = ["width", "height", "top", "left", "bottom", "right"]
      .map((p) => `${p} 0.1s linear`)
      .join(", ");
    document.body.style.overflow = "hidden";
    document.body.style.scrollbarGutter = "";
    document.documentElement.style.overflow = "hidden";
    document.documentElement.style.scrollbarGutter = "";

    setTimeout(() => {
      requestAnimationFrame(() => {
        if (!this.#fullscreen_state.on) return;

        style.width = `100%`;
        style.height = `100%`;
        style.top = `0px`;
        style.left = `0px`;
        style.bottom = `0px`;
        style.right = `0px`;
      });
    }, 0);

    this.#fullscreen_state = {
      on: true,
      saved_style,
      saved_rect,
    };

    _minimize_current_fullscreen_viewer = () => this.toggle_fullscreen();
  };
}

class LogChannel {
  #on_send;
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
    on_close: () => void,
    get_state: () => "ready" | "starting" | "stopped",
  ) {
    this.#on_send = on_send;
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
