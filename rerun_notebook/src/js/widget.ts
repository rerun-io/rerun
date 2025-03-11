import {
  type LogChannel,
  type Panel,
  type PanelState,
  WebViewer,
} from "@rerun-io/web-viewer/inlined.js";

import type { AnyModel, Render } from "@anywidget/types";
import "./widget.css";

type PanelStates = Partial<Record<Panel, PanelState>>;

const PANELS = ["top", "blueprint", "selection", "time"] as const;

/* Specifies attributes defined with traitlets in ../rerun_notebook/__init__.py */
interface WidgetModel {
  _width?: number;
  _height?: number;

  _url?: string;
  _panel_states?: PanelStates;
  _time_ctrl: [timeline: string | null, time: number | null, play: boolean];
  _recording_id?: string;
}

type Opt<T> = T | null | undefined;

class ViewerWidget {
  viewer: WebViewer = new WebViewer();
  url: Opt<string> = null;
  panel_states: Opt<PanelStates> = null;
  options = { hide_welcome_screen: true };

  channel: LogChannel | null = null;

  constructor(model: AnyModel<WidgetModel>) {
    this.url = model.get("_url");
    model.on("change:_url", this.on_change_url);

    this.panel_states = model.get("_panel_states");
    model.on("change:_panel_states", this.on_change_panel_states);

    model.on("change:_width", (_, width) => this.on_resize(null, { width }));
    model.on("change:_height", (_, height) => this.on_resize(null, { height }));

    model.on("msg:custom", this.on_custom_message);

    model.on("change:_time_ctrl", (_, [timeline, time, play]) =>
      this.on_time_ctrl(null, timeline, time, play),
    );
    model.on("change:_recording_id", this.on_set_recording_id);

    // CALLBACK DEFINITION
    // The entire object passed to `model.send` must be JSON-serializable.
    // The shape is:
    //   {
    //     event: string;
    //     payload: any;
    //   }

    this.viewer.on("selectionchange", (items) => {
      model.send({
        event: "selectionchange",
        payload: items,
      });
    });
    this.viewer.on("timelinechange", (timeline, time) => {
      model.send({
        event: "timelinechange",
        payload: { timeline, time },
      });
    });
    this.viewer.on("timeupdate", (time) => {
      model.send({
        event: "timeupdate",
        payload: time,
      });
    });

    this.viewer.on("ready", () => {
      this.channel = this.viewer.open_channel("temp");

      this.on_resize(null, {
        width: model.get("_width"),
        height: model.get("_height"),
      });

      model.send("ready");
    });
  }

  async start(el: HTMLElement) {
    await this.viewer.start(this.url ?? null, el, this.options);

    this.on_change_panel_states(null, this.panel_states);
  }

  stop() {
    this.viewer.stop();
  }

  on_resize = (_: unknown, new_size: { width?: number; height?: number }) => {
    const canvas = this.viewer.canvas;
    if (!canvas) throw new Error("on_resize called before viewer ready");

    const MIN_WIDTH = 200;
    const MIN_HEIGHT = 200;

    if (new_size.width) {
      const newWidth = Math.max(new_size.width, MIN_WIDTH);
      canvas.style.width = `${newWidth}px`;
      canvas.style.minWidth = "none";
      canvas.style.maxWidth = "none";
    } else {
      canvas.style.width = "";
      canvas.style.minWidth = "";
      canvas.style.maxWidth = "";
    }

    if (new_size.height) {
      const newHeight = Math.max(new_size.height, MIN_HEIGHT);
      canvas.style.height = `${newHeight}px`;
      canvas.style.minHeight = "none";
      canvas.style.maxHeight = "none";
    } else {
      canvas.style.height = "";
      canvas.style.minHeight = "";
      canvas.style.maxHeight = "";
    }
  };

  on_change_url = (_: unknown, new_url?: Opt<string>) => {
    if (this.url) this.viewer.close(this.url);
    if (new_url) this.viewer.open(new_url);
    this.url = new_url;
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
    if (msg?.type === "rrd") {
      if (!this.channel)
        throw new Error("on_custom_message called before channel init");
      this.channel.send_rrd(new Uint8Array(buffers[0].buffer));
    } else {
      console.log("unknown message type", msg, buffers);
    }
  };

  on_time_ctrl = (
    _: unknown,
    timeline: string | null,
    time: number | null,
    play: boolean,
  ) => {
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

  on_set_recording_id = (_: unknown, recording_id: string | null) => {
    if (recording_id === null) {
      return;
    }

    this.viewer.set_active_recording_id(recording_id);
  };
}

const render: Render<WidgetModel> = ({ model, el }) => {
  el.classList.add("rerun_notebook");

  let widget = new ViewerWidget(model);
  widget.start(el);
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
