use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use gilrs::{Axis, Button, Event, EventType, GamepadId, Gilrs};
use glam::Vec2;
use parking_lot::Mutex;

use crate::{GamepadInput, GamepadNavigation, StandardGamepadMapping};

type WakeCallback = dyn Fn() + Send + Sync;

static EVENT_BACKEND: OnceLock<Arc<EventBackend>> = OnceLock::new();

// Gamepad input is not delivered through egui/winit events, so an idle viewer would never wake up
// to poll it. This backend blocks on gamepad events on a worker thread, stores the latest snapshot,
// and wakes up the UI through a callback.
#[derive(Clone, Copy, Debug, Default)]
struct GamepadSnapshot {
    input: Option<GamepadInput>,
}

struct EventBackend {
    enabled: AtomicBool,
    snapshot: Mutex<GamepadSnapshot>,
    worker_thread: Mutex<Option<std::thread::Thread>>,
    wake_callback: Mutex<Option<Arc<WakeCallback>>>,
}

impl EventBackend {
    fn new() -> Arc<Self> {
        let this = Arc::new(Self {
            enabled: AtomicBool::new(false),
            snapshot: Mutex::new(GamepadSnapshot::default()),
            worker_thread: Mutex::new(None),
            wake_callback: Mutex::new(None),
        });

        match std::thread::Builder::new()
            .name("re_gamepad_event_backend".to_owned())
            .spawn({
                let this = Arc::clone(&this);
                move || this.run()
            }) {
            Ok(handle) => {
                *this.worker_thread.lock() = Some(handle.thread().clone());
            }
            Err(err) => {
                re_log::debug_once!("Failed to start native gamepad event backend: {err}");
            }
        }

        this
    }

    fn set_wake_callback(&self, wake_callback: Arc<WakeCallback>) {
        *self.wake_callback.lock() = Some(wake_callback);
        self.enabled.store(true, Ordering::Relaxed);
        self.unpark();
    }

    fn clear_wake_callback(&self) {
        *self.wake_callback.lock() = None;
        self.enabled.store(false, Ordering::Relaxed);
    }

    fn navigation(&self, dt: f32) -> Option<GamepadNavigation> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }

        let input = self.snapshot.lock().input?;
        Some(StandardGamepadMapping::default().map_input(input, dt))
    }

    fn run(&self) -> ! {
        let mut gilrs = match Gilrs::new() {
            Ok(gilrs) => gilrs,
            Err(err) => {
                re_log::debug_once!("Failed to initialize native gamepad backend: {err}");
                loop {
                    std::thread::park();
                }
            }
        };

        let mut active_gamepad = first_connected_gamepad(&gilrs);
        self.update_snapshot(&gilrs, active_gamepad);

        loop {
            if !self.enabled.load(Ordering::Relaxed) {
                std::thread::park();
                continue;
            }

            let Some(event) = gilrs.next_event_blocking(Some(Duration::from_millis(250))) else {
                continue;
            };

            active_gamepad = update_active_gamepad(active_gamepad, &gilrs, event);
            while let Some(event) = gilrs.next_event() {
                active_gamepad = update_active_gamepad(active_gamepad, &gilrs, event);
            }

            if active_gamepad.is_some_and(|id| !gilrs.gamepad(id).is_connected()) {
                active_gamepad = first_connected_gamepad(&gilrs);
            }

            self.update_snapshot(&gilrs, active_gamepad);
            self.wake();
        }
    }

    fn update_snapshot(&self, gilrs: &Gilrs, active_gamepad: Option<GamepadId>) {
        let input = active_gamepad
            .filter(|&id| gilrs.gamepad(id).is_connected())
            .map(|id| input_snapshot(gilrs, id));

        *self.snapshot.lock() = GamepadSnapshot { input };
    }

    fn wake(&self) {
        let wake_callback = self.wake_callback.lock().clone();

        if let Some(wake_callback) = wake_callback {
            wake_callback();
        }
    }

    fn unpark(&self) {
        if let Some(worker_thread) = &*self.worker_thread.lock() {
            worker_thread.unpark();
        }
    }
}

/// Registers a callback that is called whenever the gamepad backend observes input.
pub fn set_event_waker(wake_callback: impl Fn() + Send + Sync + 'static) {
    EVENT_BACKEND
        .get_or_init(EventBackend::new)
        .set_wake_callback(Arc::new(wake_callback));
}

/// Clears the gamepad event callback and parks the backend until it is enabled again.
pub fn clear_event_waker() {
    if let Some(event_backend) = EVENT_BACKEND.get() {
        event_backend.clear_wake_callback();
    }
}

/// Polls the active gamepad and maps it to a navigation command.
///
/// Returns `None` when no gamepad is connected or when the native backend could not be initialized.
pub fn navigation_from_active_gamepad(dt: f32) -> Option<GamepadNavigation> {
    EVENT_BACKEND.get_or_init(EventBackend::new).navigation(dt)
}

/// Native gamepad manager backed by `gilrs`.
pub struct GamepadManager {
    gilrs: Gilrs,
    active_gamepad: Option<GamepadId>,
    mapping: StandardGamepadMapping,
}

impl GamepadManager {
    /// Creates a new gamepad manager.
    pub fn new() -> Result<Self, Box<gilrs::Error>> {
        let mut this = Self {
            gilrs: Gilrs::new().map_err(Box::new)?,
            active_gamepad: None,
            mapping: StandardGamepadMapping::default(),
        };
        this.select_first_connected_gamepad();
        Ok(this)
    }

    /// Polls the active gamepad and maps it to a navigation command.
    pub fn navigation(&mut self, dt: f32) -> Option<GamepadNavigation> {
        self.poll_events();
        let active_gamepad = self.active_gamepad()?;
        let input = self.input_snapshot(active_gamepad);
        Some(self.mapping.map_input(input, dt))
    }

    /// Drains pending backend events and marks the most recently active gamepad.
    fn poll_events(&mut self) {
        while let Some(event) = self.gilrs.next_event() {
            self.active_gamepad = Some(event.id);
        }
    }

    /// Returns the current active gamepad, falling back to any connected device.
    fn active_gamepad(&mut self) -> Option<GamepadId> {
        if self
            .active_gamepad
            .is_some_and(|id| self.gilrs.gamepad(id).is_connected())
        {
            return self.active_gamepad;
        }

        self.select_first_connected_gamepad();
        self.active_gamepad
    }

    /// Selects the first connected gamepad reported by the backend.
    fn select_first_connected_gamepad(&mut self) {
        self.active_gamepad = self
            .gilrs
            .gamepads()
            .find_map(|(id, gamepad)| gamepad.is_connected().then_some(id));
    }

    /// Captures normalized input for the given gamepad.
    fn input_snapshot(&self, gamepad_id: GamepadId) -> GamepadInput {
        input_snapshot(&self.gilrs, gamepad_id)
    }
}

fn update_active_gamepad(
    active_gamepad: Option<GamepadId>,
    gilrs: &Gilrs,
    event: Event,
) -> Option<GamepadId> {
    match event.event {
        EventType::Disconnected => {
            if Some(event.id) == active_gamepad {
                first_connected_gamepad(gilrs)
            } else {
                active_gamepad
            }
        }
        EventType::Connected
        | EventType::ButtonPressed(_, _)
        | EventType::ButtonRepeated(_, _)
        | EventType::ButtonReleased(_, _)
        | EventType::ButtonChanged(_, _, _)
        | EventType::AxisChanged(_, _, _) => Some(event.id),
        _ => active_gamepad,
    }
}

fn first_connected_gamepad(gilrs: &Gilrs) -> Option<GamepadId> {
    gilrs
        .gamepads()
        .find_map(|(id, gamepad)| gamepad.is_connected().then_some(id))
}

fn input_snapshot(gilrs: &Gilrs, gamepad_id: GamepadId) -> GamepadInput {
    let gamepad = gilrs.gamepad(gamepad_id);

    let axis = |axis: Axis| gamepad.value(axis).clamp(-1.0, 1.0);
    let button = |button: Button| gamepad.is_pressed(button);
    let trigger = |trigger_axis: Axis, trigger_button: Button| {
        trigger_value(axis(trigger_axis), button(trigger_button))
    };

    GamepadInput {
        left_stick: Vec2::new(axis(Axis::LeftStickX), axis(Axis::LeftStickY)),
        right_stick: Vec2::new(axis(Axis::RightStickX), axis(Axis::RightStickY)),
        left_trigger: trigger(Axis::LeftZ, Button::LeftTrigger2),
        right_trigger: trigger(Axis::RightZ, Button::RightTrigger2),
        slow_down: button(Button::LeftTrigger),
        speed_up: button(Button::RightTrigger),
    }
}

fn trigger_value(raw_axis: f32, button_pressed: bool) -> f32 {
    if button_pressed {
        return 1.0;
    }

    if raw_axis < 0.0 {
        ((raw_axis + 1.0) * 0.5).clamp(0.0, 1.0)
    } else {
        raw_axis.clamp(0.0, 1.0)
    }
}
