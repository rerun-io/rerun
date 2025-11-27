import {
  type LogChannel,
  type Panel,
  type PanelState,
  WebViewer,
  type AppOptions,
} from "@rerun-io/web-viewer";

import type { AnyModel, Render } from "@anywidget/types";
import "./widget.css";

type PanelStates = Partial<Record<Panel, PanelState>>;

const PANELS = ["top", "blueprint", "selection", "time"] as const;

/* Specifies attributes defined with traitlets in ../rerun_notebook/__init__.py */
interface WidgetModel {
  _width: number | string;
  _height: number | string;

  _url?: string;
  _panel_states?: PanelStates;

  _fallback_token?: string;
}

type Opt<T> = T | null | undefined;

function _resize(el: HTMLElement, width: number | string, height: number | string) {
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
  url: Opt<string> = null;
  panel_states: Opt<PanelStates> = null;
  options: AppOptions = {
    notebook: true,
    hide_welcome_screen: true,
    width: "100%",
    height: "100%",
  };

  channel: LogChannel | null = null;

  constructor(model: AnyModel<WidgetModel>, el: HTMLElement) {
    this.url = model.get("_url");

    this.panel_states = model.get("_panel_states");
    model.on("change:_panel_states", this.on_change_panel_states);

    model.on("change:_width", (_, width) => this.on_resize(el, width, model.get("_height")));
    model.on("change:_height", (_, height) => this.on_resize(el, model.get("_width"), height));

    model.on("msg:custom", this.on_custom_message);

    this.options.fallback_token = model.get("_fallback_token");

    (this.viewer as any)._on_raw_event((event: string) => model.send(event));

    this.viewer.on("ready", () => {
      this.channel = this.viewer.open_channel("temp");
      this.on_change_panel_states(null, this.panel_states);

      model.send("ready");
    });

    this.viewer.start(this.url ?? null, el, this.options);
    this.on_resize(el, model.get("_width"), model.get("_height"));
  }

  stop() {
    this.viewer.stop();
  }

  on_resize(parent: HTMLElement, width: number | string, height: number | string) {
    _resize(parent, width, height)
  };

  on_change_panel_states = (
    _: unknown,
    new_panel_states?: Opt<PanelStates>,
  ) => {
    for (const panel of PANELS) {
      // TODO(jprochazk): update `override_panel_state` to accept `PanelState | undefined | null` as value
      this.viewer.override_panel_state(panel, new_panel_states?.[panel]);
    }
    this.panel_states = new_panel_states;
  };

  on_custom_message = (msg: any, buffers: DataView[]) => {
    switch (msg?.type) {
      case "rrd": {
        if (!this.channel)
          throw new Error("on_custom_message called before channel init");
        this.channel.send_rrd(new Uint8Array(buffers[0].buffer));
        break;
      }
      case "table": {
        if (!this.channel)
          throw new Error("on_custom_message called before channel init")
        this.channel.send_table(new Uint8Array(buffers[0].buffer));
        break;
      }
      case "time_ctrl": {
        this.set_time_ctrl(msg.timeline ?? null, msg.time ?? null, msg.play ?? false);
        break;
      }
      case "recording_id": {
        this.set_recording_id(msg.recording_id ?? null)
        break;
      }
      case "open_url": {
        this.viewer.open(msg.url)
        break;
      }
      case "close_url": {
        this.viewer.close(msg.url)
        break;
      }
      case "set_credentials": {
        this.viewer.set_credentials(msg.access_token, msg.email)
        break;
      }
      default: {
        console.error("received unknown message type", msg, buffers);
        throw new Error(`unknown message type ${msg}, check console for more details`);
      }
    }
  };

  set_time_ctrl(
    timeline: string | null,
    time: number | null,
    play: boolean,
  ) {
    let recording_id = this.viewer.get_active_recording_id();
    if (recording_id === null) {
      return;
    }

    let active_timeline = this.viewer.get_active_timeline(recording_id);

    if (timeline === null) {
      timeline = active_timeline;
    }

    if (timeline === null) {
      return;
    }

    if (timeline !== active_timeline) {
      this.viewer.set_active_timeline(recording_id, timeline);
    }

    this.viewer.set_playing(recording_id, play);

    if (time !== null) {
      this.viewer.set_current_time(recording_id, timeline, time);
    }
  };

  set_recording_id(recording_id: string | null) {
    if (recording_id === null) {
      return;
    }

    this.viewer.set_active_recording_id(recording_id);
  };
}



const render: Render<WidgetModel> = ({ model, el }) => {
  el.classList.add("rerun_notebook");

  const container = document.createElement("div");
  el.append(container);

  let widget = new ViewerWidget(model, container);
  return () => widget.stop();
};

function error_boundary<Fn extends (...args: any[]) => any>(f: Fn): Fn {
  const wrapper = (...args: any[]) => {
    try {
      return f(...args);
    } catch (e) {
      const el = document.querySelector(".rerun_notebook");
      if (el) {
        el.innerHTML = `<div class="error">${e}</div>`;
      }
    }
  };

  return wrapper as any;
}


export default { render: error_boundary(render) };
