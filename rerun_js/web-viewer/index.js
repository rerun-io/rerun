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

/** @returns {string} */
function randomId() {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes)
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

export class WebViewer {
  /** @type {(import("./re_viewer.js").WebHandle) | null} */
  #handle = null;

  /** @type {HTMLCanvasElement | null} */
  #canvas = null;

  /** @type {'ready' | 'starting' | 'stopped'} */
  #state = "stopped";

  /**
   * Start the viewer.
   *
   * @param {string | string[]} [rrd] URLs to `.rrd` files or WebSocket connections to our SDK.
   * @param {HTMLElement} [parent] The element to attach the canvas onto.
   * @returns {Promise<void>}
   */
  async start(rrd, parent = document.body) {
    if (this.#state !== "stopped") return;
    this.#state = "starting";

    this.#canvas = document.createElement("canvas");
    this.#canvas.id = randomId();
    parent.append(this.#canvas);

    let WebHandle_class = await load();
    if (this.#state !== "starting") return;

    this.#handle = new WebHandle_class();
    await this.#handle.start(this.#canvas.id, undefined);
    if (this.#state !== "starting") return;

    if (this.#handle.has_panicked()) {
      throw new Error(`Web viewer crashed: ${this.#handle.panic_message()}`);
    }

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
   */
  open(rrd) {
    if (!this.#handle) {
      throw new Error(`attempted to open \`${rrd}\` in a stopped viewer`);
    }
    const urls = Array.isArray(rrd) ? rrd : [rrd];
    for (const url of urls) {
      this.#handle.add_receiver(url);
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
    this.#state = "stopped";

    this.#canvas?.remove();
    this.#handle?.destroy();
    this.#handle?.free();

    this.#canvas = null;
    this.#handle = null;
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
    if (!this.#handle) throw new Error("...");
    const id = crypto.randomUUID();
    this.#handle.open_channel(id, channel_name);
    const on_send = (/** @type {Uint8Array} */ data) => {
      if (!this.#handle) throw new Error("...");
      this.#handle.send_rrd_to_channel(id, data);
    };
    const on_close = () => {
      if (!this.#handle) throw new Error("...");
      this.#handle.close_channel(id);
    };
    const get_state = () => this.#state;
    return new LogChannel(on_send, on_close, get_state);
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
