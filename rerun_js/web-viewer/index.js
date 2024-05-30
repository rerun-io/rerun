// @ts-check

/** @type {(typeof import("./re_viewer.js").WebHandle) | null} */
let WebHandle = null;

/** @returns {Promise<(typeof import("./re_viewer.js").WebHandle)>} */
async function load() {
  if (WebHandle) {
    return WebHandle;
  }
  WebHandle = (await import("./re_viewer.js")).WebHandle;
  return WebHandle;
}

/**
 * Used to prevent multiple viewers from being fullscreen at the same time.
 *
 * @type {(() => void) | null}
 */
let _minimize_current_fullscreen_viewer = null;

/** @returns {string} */
function randomId() {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes)
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

/**
 * @typedef {"top" | "blueprint" | "selection" | "time"} Panel
 * @typedef {"hidden" | "collapsed" | "expanded"} PanelState
 * @typedef {"webgpu" | "webgl"} Backend
 * @typedef {{
 *   canvas: {
 *     position: string;
 *     width: string; height: string;
 *     top: string; left: string;
 *     bottom: string; right: string;
 *   };
 *   document: { overflow: string };
 * }} CanvasStyle
 * @typedef {{ on: false; saved_style: null }} FullscreenOff
 * @typedef {{ on: true; saved_style: CanvasStyle }} FullscreenOn
 * @typedef {(FullscreenOff | FullscreenOn)} FullscreenState
 */

/**
 * @typedef WebViewerOptions
 * @property {string} [manifest_url] Use a different example manifest.
 * @property {Backend} [render_backend] Force the viewer to use a specific rendering backend.
 * @property {boolean} [hide_welcome_screen] Whether to hide the welcome screen in favor of a simpler one.
 * @property {boolean} [allow_fullscreen] Whether to allow the viewer to enter fullscreen mode.
 */

export class WebViewer {
  #id = randomId();

  /** @type {(import("./re_viewer.js").WebHandle) | null} */
  #handle = null;

  /** @type {HTMLCanvasElement | null} */
  #canvas = null;

  /** @type {'ready' | 'starting' | 'stopped'} */
  #state = "stopped";

  /**
   * @type {FullscreenState}
   */
  #fullscreen_state = {
    on: false,
    saved_style: null,
  };

  #allow_fullscreen = false;

  /**
   * Start the viewer.
   *
   * @param {string | string[] | null} [rrd] URLs to `.rrd` files or WebSocket connections to our SDK.
   * @param {HTMLElement | null} [parent] The element to attach the canvas onto.
   * @param {WebViewerOptions | null} [options] Whether to hide the welcome screen.
   * @returns {Promise<void>}
   */
  async start(rrd, parent, options) {
    parent ??= document.body;
    options ??= {};

    if (this.#state !== "stopped") return;
    this.#state = "starting";

    this.#canvas = document.createElement("canvas");
    this.#canvas.id = this.#id;
    parent.append(this.#canvas);

    /**
     * @typedef AppOptions
     * @property {string} [url]
     * @property {string} [manifest_url]
     * @property {Backend} [render_backend]
     * @property {Partial<{[K in Panel]: PanelState}>} [panel_state_overrides]
     * @property {boolean} [hide_welcome_screen]
     * @property {boolean} [allow_fullscreen]
     * @property {() => void} [on_toggle_fullscreen]
     */
    /** @typedef {(import("./re_viewer.js").WebHandle)} _WebHandle */
    /** @typedef {{ new(app_options?: AppOptions): _WebHandle }} WebHandleConstructor */

    let WebHandle_class = /** @type {WebHandleConstructor} */ (await load());
    if (this.#state !== "starting") return;

    let on_toggle_fullscreen = () => this.toggle_fullscreen();
    this.#handle = new WebHandle_class({ ...options, on_toggle_fullscreen });
    await this.#handle.start(this.#canvas.id);
    if (this.#state !== "starting") return;

    if (this.#handle.has_panicked()) {
      throw new Error(`Web viewer crashed: ${this.#handle.panic_message()}`);
    }

    this.#allow_fullscreen = options.allow_fullscreen || false;

    this.#state = "ready";
    if (rrd) {
      this.open(rrd);
    }

    return;
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
   * @see {WebViewer.start}
   *
   * @param {string | string[]} rrd URLs to `.rrd` files or WebSocket connections to our SDK.
   * @param {{ follow_if_http?: boolean }} options
   *        - follow_if_http: Whether Rerun should open the resource in "Following" mode when streaming
   *        from an HTTP url. Defaults to `false`. Ignored for non-HTTP URLs.
   */
  open(rrd, options = {}) {
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
   * @see {WebViewer.start}
   *
   * @param {string | string[]} rrd URLs to `.rrd` files or WebSocket connections to our SDK.
   */
  close(rrd) {
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
   * @param {string} channel_name used to identify the channel.
   *
   * @returns {LogChannel}
   */
  open_channel(channel_name = "rerun-io/web-viewer") {
    if (!this.#handle) {
      throw new Error(
        `attempted to open channel \"${channel_name}\" in a stopped web viewer`,
      );
    }
    const id = crypto.randomUUID();
    this.#handle.open_channel(id, channel_name);
    const on_send = (/** @type {Uint8Array} */ data) => {
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
   * @param {Panel} panel
   * @param {PanelState} state
   */
  override_panel_state(panel, state) {
    if (!this.#handle) {
      throw new Error(
        `attempted to set ${panel} panel to ${state} in a stopped web viewer`,
      );
    }
    this.#handle.override_panel_state(panel, state);
  }

  /**
   * Toggle panel overrides set via `override_panel_state`.
   */
  toggle_panel_overrides() {
    if (!this.#handle) {
      throw new Error(
        `attempted to toggle panel overrides in a stopped web viewer`,
      );
    }
    this.#handle.toggle_panel_overrides();
  }

  #minimize = (
    /** @type {HTMLCanvasElement} */ canvas,
    /** @type {FullscreenOn} */ { saved_style },
  ) => {
    this.#fullscreen_state = {
      on: false,
      saved_style: null,
    };

    if (this.#fullscreen_state.on) return;

    for (const key in saved_style.canvas) {
      // @ts-expect-error
      canvas.style[key] = saved_style.canvas[key];
    }
    for (const key in saved_style.document) {
      // @ts-expect-error
      document.body.style[key] = saved_style.document[key];
    }

    _minimize_current_fullscreen_viewer = null;
  };

  #maximize = (/** @type {HTMLCanvasElement} */ canvas) => {
    _minimize_current_fullscreen_viewer?.();

    const style = canvas.style;

    /** @type {CanvasStyle} */
    const saved_style = {
      canvas: {
        position: style.position,
        width: style.width,
        height: style.height,
        top: style.top,
        left: style.left,
        bottom: style.bottom,
        right: style.right,
      },
      document: { overflow: document.body.style.overflow },
    };

    style.width = `100%`;
    style.height = `100%`;
    style.top = `0px`;
    style.left = `0px`;
    style.bottom = `0px`;
    style.right = `0px`;
    document.body.style.overflow = "hidden";

    this.#fullscreen_state = {
      on: true,
      saved_style,
    };

    _minimize_current_fullscreen_viewer = () => this.toggle_fullscreen();
  };

  toggle_fullscreen() {
    if (!this.#handle || !this.#canvas) {
      throw new Error(
        `attempted to toggle fullscreen mode in a stopped web viewer`,
      );
    }

    if (!this.#allow_fullscreen) return;

    const state = this.#fullscreen_state;
    if (state.on) {
      this.#minimize(this.#canvas, state);
    } else {
      this.#maximize(this.#canvas);
    }
  }
}

export class LogChannel {
  #on_send;
  #on_close;
  #get_state;
  #closed = false;

  /** @internal
   *
   * @param {(data: Uint8Array) => void} on_send
   * @param {() => void} on_close
   * @param {() => 'ready' | 'starting' | 'stopped'} get_state
   */
  constructor(on_send, on_close, get_state) {
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
   * @param {Uint8Array} rrd_bytes Is an rrd file stored in a byte array, received via some other side channel.
   */
  send_rrd(rrd_bytes) {
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
