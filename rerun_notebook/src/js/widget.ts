import {
  type LogChannel,
  type Panel,
  type PanelState,
  parseStoreId,
  WebViewer,
  type AppOptions,
} from "@rerun-io/web-viewer";

import type { AnyModel, Render } from "@anywidget/types";
import "./widget.css";

type PanelStates = Partial<Record<Panel, PanelState>>;

const PANELS = ["top", "blueprint", "selection", "time"] as const;

/* Specifies attributes defined with traitlets in ../rerun_notebook/__init__.py */
interface WidgetModel {
  "_width": number | string;
  "_height": number | string;

  "_url"?: string;
  "_panel_states"?: PanelStates;

  "_fallback_token"?: string;
  "_theme"?: AppOptions["theme"] | null;
}

type Opt<T> = T | null | undefined;

function resize(el: HTMLElement, width: number | string, height: number | string) {
  const style = el.style;

  if (typeof width === "string" && width === "auto") {
    style.width = "100%";
  } else if (typeof width === "number") {
    style.width = `${Math.max(200, width)}px`;
  } else {
    style.width = "640px";
  }

  if (typeof height === "string" && height === "auto") {
    style.height = "auto";
    style.aspectRatio = "16 / 9";
  } else if (typeof height === "number") {
    style.height = `${Math.max(200, height)}px`;
    style.aspectRatio = "";
  } else {
    style.height = "640px";
    style.aspectRatio = "";
  }
}

class ViewerWidget {
  viewer: WebViewer = new WebViewer();

  /**
   * The viewer operations from Python, run one at a time in the order they arrived.
   *
   * A recording switch reads the viewer state first, so a seek sent right after it must wait for it.
   */
  operations: Promise<void> = Promise.resolve();
  url: Opt<string> = null;
  panelStates: Opt<PanelStates> = null;
  options: AppOptions = {
    notebook: true,
    hideWelcomeScreen: true,
    width: "100%",
    height: "100%",
  };

  channel: LogChannel | null = null;

  constructor(model: AnyModel<WidgetModel>, el: HTMLElement) {
    this.url = model.get("_url");

    this.panelStates = model.get("_panel_states");
    model.on("change:_panel_states", this.onChangePanelStates);

    const onWidthChange = (...[, width]: [unknown, number | string]) => {
      this.onResize(el, width, model.get("_height"));
    };
    const onHeightChange = (...[, height]: [unknown, number | string]) => {
      this.onResize(el, model.get("_width"), height);
    };
    model.on("change:_width", onWidthChange);
    model.on("change:_height", onHeightChange);

    model.on("msg:custom", this.onCustomMessage);

    this.options.fallbackToken = model.get("_fallback_token");
    const theme = model.get("_theme");
    if (theme) {
      this.options.theme = theme;
    }

    (this.viewer as any)._on_raw_event((event: string) => model.send(event));

    this.viewer.on("ready", () => {
      this.channel = this.viewer.openChannel("temp");
      this.onChangePanelStates(null, this.panelStates);

      model.send("ready");
    });

    void this.viewer.start(this.url ?? null, el, this.options);
    this.onResize(el, model.get("_width"), model.get("_height"));
  }

  stop() {
    this.viewer.stop();
  }

  onResize(parent: HTMLElement, width: number | string, height: number | string) {
    resize(parent, width, height);
  }

  onChangePanelStates = (...[, newPanelStates]: [unknown, Opt<PanelStates>?]) => {
    for (const panel of PANELS) {
      // TODO(jprochazk): update `overridePanelState` to accept `PanelState | undefined | null` as value
      this.viewer.overridePanelState(panel, newPanelStates?.[panel]);
    }
    this.panelStates = newPanelStates;
  };

  onCustomMessage = (msg: any, buffers: DataView[]) => {
    switch (msg?.type) {
      case "rrd": {
        if (!this.channel) throw new Error("on_custom_message called before channel init");
        this.channel.sendRrd(new Uint8Array(buffers[0].buffer));
        break;
      }
      case "table": {
        if (!this.channel) throw new Error("on_custom_message called before channel init");
        this.channel.sendTable(new Uint8Array(buffers[0].buffer));
        break;
      }
      case "time_ctrl": {
        this.setTimeCtrl(msg);
        break;
      }
      case "recording_id": {
        this.setRecordingId(msg.recording_id ?? null);
        break;
      }
      case "open_url": {
        this.queueOperation("Failed to open URL:", async () => {
          // `open` sends its request right away, but only answers once the URL has loaded.
          // Waiting for that would hold back a `close_url` meant to cancel the load.
          this.viewer.open(msg.url).catch((err) => console.warn("Failed to open URL:", err));
        });
        break;
      }
      case "close_url": {
        this.queueOperation("Failed to close URL:", async () => {
          await this.viewer.close(msg.url);
        });
        break;
      }
      case "set_credentials": {
        this.viewer.setCredentials(msg.access_token, msg.email);
        break;
      }
      default: {
        console.error("received unknown message type", msg, buffers);
        throw new Error(`unknown message type ${msg}, check console for more details`);
      }
    }
  };

  /**
   * Seek, switch timeline, and play or pause, on the active recording.
   *
   * Python sends integer times as strings, since a JSON number loses nanosecond precision.
   */
  setTimeCtrl(msg: {
    timeline?: string | null;
    sequence?: string | null;
    "duration_nanos"?: string | null;
    "timestamp_nanos"?: string | null;
    play?: boolean;
  }) {
    const time =
      msg.sequence != null
        ? { sequence: BigInt(msg.sequence) }
        : msg.duration_nanos != null
          ? { durationNanos: BigInt(msg.duration_nanos) }
          : msg.timestamp_nanos != null
            ? { timestampNanos: BigInt(msg.timestamp_nanos) }
            : {};
    this.queueOperation("Failed to set the time:", () =>
      this.viewer.setTime({
        timeline: msg.timeline ?? undefined,
        ...time,
        playState: msg.play ? "playing" : "paused",
      }),
    );
  }

  /** Show the open recording with this recording id. */
  setRecordingId(recordingId: string | null) {
    if (recordingId === null) {
      return;
    }

    this.queueOperation("Failed to switch recording:", async () => {
      const state = await this.viewer.getState();
      const recording = state.recordings.find(
        (recording) => parseStoreId(recording.storeId).recordingId === recordingId,
      );
      if (recording) {
        await this.viewer.setLocation(recording.location);
      }
    });
  }

  /** Run `operation` once the earlier operations are done, and log if it fails. */
  queueOperation(failure: string, operation: () => Promise<void>) {
    this.operations = this.operations.then(operation).catch((err) => console.warn(failure, err));
  }
}

const render: Render<WidgetModel> = ({ model, el }) => {
  el.classList.add("rerun_notebook");

  const container = document.createElement("div");
  el.append(container);

  let widget = new ViewerWidget(model, container);
  return () => widget.stop();
};

function errorBoundary<Fn extends (...args: any[]) => any>(f: Fn): Fn {
  const wrapper = (...args: any[]) => {
    try {
      return f(...args);
    } catch (e) {
      const el = document.querySelector(".rerun_notebook");
      if (el) {
        el.innerHTML = `<div class="error">${String(e)}</div>`;
      }
    }
  };

  return wrapper as any;
}

export default { render: errorBoundary(render) };
