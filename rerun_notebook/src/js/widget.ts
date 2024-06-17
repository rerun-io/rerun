import { LogChannel, Panel, PanelState, WebViewer } from "@rerun-io/web-viewer";
import type { AnyModel, Render } from "@anywidget/types";
import "./widget.css";

type PanelStates = Partial<Record<Panel, PanelState>>;

const PANELS = ["top", "blueprint", "selection", "time"] as const;

/* Specifies attributes defined with traitlets in ../rerun_notebook/__init__.py */
interface WidgetModel {
  width?: number;
  height?: number;

  _url?: string;
  _panel_states?: PanelStates;
  _data?: DataView;
}

type Opt<T> = T | null | undefined;

class ViewerWidget {
  viewer: WebViewer = new WebViewer();
  url: Opt<string> = null;
  panel_states: Opt<PanelStates> = null;
  options = { hide_welcome_screen: true };

  channel: LogChannel | null = null;

  constructor(model: AnyModel<WidgetModel>) {
    // TODO: use `width`/`height` to set canvas size if present

    this.url = model.get("_url");
    model.on("change:_url", this.on_change_url);

    this.panel_states = model.get("_panel_states");
    model.on("change:_panel_states", this.on_change_panel_states);

    // Buffer data until the viewer is ready
    const queue: Uint8Array[] = [];
    const push = (data?: Opt<DataView>) =>
      data && queue.push(new Uint8Array(data.buffer));

    push(model.get("_data"));
    model.on("change:_data", (_, data) => push(data));

    this.viewer.on("ready", () => {
      this.channel = this.viewer.open_channel("temp");

      // Send buffered data
      for (const data of queue) {
        this.channel.send_rrd(data);
      }
      // Any subsequent data will be sent immediately
      model.on("change:_data", this.on_change_data);

      model.send("ready");
    });
  }

  async start(el: HTMLElement) {
    await this.viewer.start(this.url ?? null, el, this.options);

    this.on_change_panel_states(this.panel_states);
  }

  stop() {
    this.viewer.stop();
  }

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
      const state: any = new_panel_states?.[panel];
      this.viewer.override_panel_state(panel, state);
    }
    this.panel_states = new_panel_states;
  };

  on_change_data = (_: unknown, data?: Opt<DataView>) => {
    if (data && this.channel) {
      this.channel.send_rrd(new Uint8Array(data.buffer));
    }
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
