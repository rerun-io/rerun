//! Probe the Wayland compositor for the actual decoration mode it will use.
//!
//! See [`crate::custom_window_decorations_default`] for how the result is used.
//!
//! Just checking for the presence of `zxdg_decoration_manager_v1` is not enough:
//! some compositors (notably GNOME/mutter) advertise the protocol but still
//! default to client-side decorations. We bind a throwaway `xdg_toplevel`,
//! ask for `server_side`, and read back the mode the compositor commits to.
//! The surface never gets a buffer attached, so no window appears.

// NOTE: This does a lot of back and forth, but ultimately we persist this after
// the initial launch together with the rest of the app options.

use wayland_client::{
    ConnectError, Connection, Dispatch, DispatchError, QueueHandle, WEnum, delegate_noop,
    protocol::{
        wl_compositor::WlCompositor,
        wl_registry::{self, WlRegistry},
        wl_surface::WlSurface,
    },
};
use wayland_protocols::xdg::{
    decoration::zv1::client::{
        zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
        zxdg_toplevel_decoration_v1::{self, Mode, ZxdgToplevelDecorationV1},
    },
    shell::client::{
        xdg_surface::{self, XdgSurface},
        xdg_toplevel::XdgToplevel,
        xdg_wm_base::{self, XdgWmBase},
    },
};

#[derive(Debug, thiserror::Error)]
enum ProbeError {
    #[error(transparent)]
    Connect(#[from] ConnectError),

    #[error("registry roundtrip failed: {0}")]
    RegistryRoundtrip(DispatchError),

    #[error("decoration roundtrip failed: {0}")]
    DecorationRoundtrip(DispatchError),

    #[error("compositor does not expose `{0}`")]
    MissingGlobal(&'static str),

    /// The compositor simply doesn't speak the protocol — expected on many
    /// setups (older sway, weston, some tiling WMs). Not a real failure.
    #[error("compositor does not advertise `zxdg_decoration_manager_v1`")]
    NoDecorationManager,

    #[error("compositor did not send a `zxdg_toplevel_decoration_v1` configure event")]
    NoConfigureEvent,
}

/// Whether we should draw our own decorations on this system.
///
/// Returns `true` when we are not on Wayland, when the probe fails, or when
/// the compositor commits to client-side decorations.
pub(crate) fn should_draw_own_decorations() -> bool {
    match probe() {
        Ok(Mode::ServerSide) => false,
        Ok(_) => true,
        Err(err @ (ProbeError::NoDecorationManager | ProbeError::NoConfigureEvent)) => {
            re_log::debug!("Drawing custom decorations: {err}");
            true
        }
        Err(err) => {
            re_log::warn_once!("Drawing custom decorations because Wayland probe failed: {err}");
            true
        }
    }
}

fn probe() -> Result<Mode, ProbeError> {
    let conn = Connection::connect_to_env()?;

    let mut event_queue = conn.new_event_queue::<State>();
    let qh = event_queue.handle();
    let _registry = conn.display().get_registry(&qh, ());

    let mut state = State::default();

    // First roundtrip: discover the globals we need.
    event_queue
        .roundtrip(&mut state)
        .map_err(ProbeError::RegistryRoundtrip)?;

    let compositor = state
        .compositor
        .take()
        .ok_or(ProbeError::MissingGlobal("wl_compositor"))?;
    let wm_base = state
        .wm_base
        .take()
        .ok_or(ProbeError::MissingGlobal("xdg_wm_base"))?;
    let decoration_manager = state
        .decoration_manager
        .take()
        .ok_or(ProbeError::NoDecorationManager)?;

    // Build a throwaway toplevel and request server-side decorations. Without a
    // buffer attach the surface never maps, so nothing becomes visible.
    let surface = compositor.create_surface(&qh, ());
    let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
    let toplevel = xdg_surface.get_toplevel(&qh, ());
    let decoration = decoration_manager.get_toplevel_decoration(&toplevel, &qh, ());
    decoration.set_mode(Mode::ServerSide);
    surface.commit();

    // Second roundtrip: read the mode the compositor actually chose.
    let result = event_queue
        .roundtrip(&mut state)
        .map_err(ProbeError::DecorationRoundtrip)
        .and_then(|_| state.decoration_mode.ok_or(ProbeError::NoConfigureEvent));

    // Tear down in reverse dependency order.
    decoration.destroy();
    toplevel.destroy();
    xdg_surface.destroy();
    surface.destroy();

    result
}

#[derive(Default)]
struct State {
    compositor: Option<WlCompositor>,
    wm_base: Option<XdgWmBase>,
    decoration_manager: Option<ZxdgDecorationManagerV1>,
    decoration_mode: Option<Mode>,
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name, interface, ..
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_compositor" => {
                state.compositor = Some(registry.bind::<WlCompositor, _, _>(name, 1, qh, ()));
            }
            "xdg_wm_base" => {
                state.wm_base = Some(registry.bind::<XdgWmBase, _, _>(name, 1, qh, ()));
            }
            "zxdg_decoration_manager_v1" => {
                state.decoration_manager =
                    Some(registry.bind::<ZxdgDecorationManagerV1, _, _>(name, 1, qh, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<XdgWmBase, ()> for State {
    fn event(
        _: &mut Self,
        wm_base: &XdgWmBase,
        event: xdg_wm_base::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Protocol requires us to reply or the compositor will consider us hung.
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<XdgSurface, ()> for State {
    fn event(
        _: &mut Self,
        xdg_surface: &XdgSurface,
        event: xdg_surface::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surface.ack_configure(serial);
        }
    }
}

impl Dispatch<ZxdgToplevelDecorationV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZxdgToplevelDecorationV1,
        event: zxdg_toplevel_decoration_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_toplevel_decoration_v1::Event::Configure {
            mode: WEnum::Value(mode),
        } = event
        {
            state.decoration_mode = Some(mode);
        }
    }
}

delegate_noop!(State: WlCompositor);
delegate_noop!(State: ZxdgDecorationManagerV1);
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: ignore XdgToplevel);
