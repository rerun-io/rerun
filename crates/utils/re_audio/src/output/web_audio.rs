//! Web playback through the Web Audio API.
//!
//! Browsers keep a new `AudioContext` suspended until the page has seen a user gesture,
//! and some only resume it from inside the gesture's event handler. The device listens for
//! gestures on the window and resumes the context from there whenever it is not running.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::*;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, AudioScheduledSourceNode,
};

use super::{FailureCallback, FillCallback, OutputDeviceParameters, OutputError};

/// Buffers scheduled on the context. One plays while the other is filled and waits.
const NUM_BUFFERS: usize = 2;

/// Events that count as a user gesture for audio playback.
///
/// Each browser accepts a different subset of these. Listening to all of them is fine,
/// since `resume` from an event the browser doesn't accept leaves the context suspended.
const GESTURE_EVENTS: [&str; 7] = [
    "click",
    "keydown",
    "mousedown",
    "mouseup",
    "pointerdown",
    "pointerup",
    "touchend",
];

pub struct Device {
    feed: Rc<RefCell<Feed>>,

    /// Resumes the context, registered on the window for each of [`GESTURE_EVENTS`].
    on_gesture: Closure<dyn FnMut()>,

    /// Starts the buffers once the context runs. Set as the context's `onstatechange`.
    _on_state_change: Closure<dyn FnMut()>,
}

// SAFETY: wasm32 has a single thread, so the JS objects are never touched from two threads.
#[expect(unsafe_code, reason = "JS objects are not `Send`")]
#[expect(clippy::undocumented_unsafe_blocks)] // false positive
unsafe impl Send for Device {}

/// The context and the buffers it plays, refilled one at a time as each finishes.
struct Feed {
    context: AudioContext,
    params: OutputDeviceParameters,
    fill: FillCallback,

    /// Taken on the first failure, so it is reported only once.
    on_failure: Option<FailureCallback>,

    /// Whether the buffers are scheduled. This happens once, the first time the context runs,
    /// so `fill` is not called while the context is suspended.
    started: bool,

    /// Context time at which the next buffer starts.
    next_start_secs: f64,

    /// Interleaved samples from `fill`.
    interleaved: Vec<f32>,

    /// One channel of `interleaved`, the layout `AudioBuffer::copy_to_channel` takes.
    channel: Vec<f32>,

    slots: Vec<Slot>,
}

struct Slot {
    buffer: AudioBuffer,

    /// Refills and schedules this slot again. Set as the `onended` of its source.
    on_ended: Closure<dyn FnMut()>,

    /// The scheduled source playing this slot's buffer.
    source: Option<AudioBufferSourceNode>,
}

impl Drop for Device {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            for event in GESTURE_EVENTS {
                window
                    .remove_event_listener_with_callback_and_bool(
                        event,
                        self.on_gesture.as_ref().unchecked_ref(),
                        true,
                    )
                    .ok();
            }
        }

        let mut feed = self.feed.borrow_mut();
        feed.context.set_onstatechange(None);
        feed.stop_sources();
        feed.context.close().ok();
    }
}

impl Device {
    /// Creates an `AudioContext` and starts calling `fill` for samples once it runs.
    pub fn new(
        params: OutputDeviceParameters,
        fill: FillCallback,
        on_failure: FailureCallback,
    ) -> Result<Self, OutputError> {
        let window = web_sys::window().ok_or_else(|| OutputError::Device("No window".to_owned()))?;

        let options = web_sys::AudioContextOptions::new();
        options.set_sample_rate(params.sample_rate as f32);
        let context = AudioContext::new_with_context_options(&options)
            .map_err(|err| js_error("Creating an AudioContext", &err))?;

        let num_samples = params.frames_per_callback * params.num_channels;
        let feed = Rc::new(RefCell::new(Feed {
            context: context.clone(),
            params,
            fill,
            on_failure: Some(on_failure),
            started: false,
            next_start_secs: 0.0,
            interleaved: vec![0.0; num_samples],
            channel: vec![0.0; params.frames_per_callback],
            slots: Vec::with_capacity(NUM_BUFFERS),
        }));

        for slot_index in 0..NUM_BUFFERS {
            let buffer = context
                .create_buffer(
                    params.num_channels as u32,
                    params.frames_per_callback as u32,
                    params.sample_rate as f32,
                )
                .map_err(|err| js_error("Creating an AudioBuffer", &err))?;
            let on_ended = Closure::<dyn FnMut()>::new({
                let feed = Rc::downgrade(&feed);
                move || on_slot_ended(&feed, slot_index)
            });
            feed.borrow_mut().slots.push(Slot {
                buffer,
                on_ended,
                source: None,
            });
        }

        let on_state_change = Closure::<dyn FnMut()>::new({
            let feed = Rc::downgrade(&feed);
            move || {
                if let Some(feed) = feed.upgrade() {
                    feed.borrow_mut().start_if_running();
                }
            }
        });
        context.set_onstatechange(Some(on_state_change.as_ref().unchecked_ref()));
        feed.borrow_mut().start_if_running();

        let on_resume_failed = Closure::<dyn FnMut(JsValue)>::new(|err: JsValue| {
            re_log::error_once!(
                "Resuming the AudioContext failed: {}",
                re_web::Error::from(&err)
            );
        });
        let on_gesture = Closure::<dyn FnMut()>::new(move || {
            if context.state() != AudioContextState::Running {
                match context.resume() {
                    Ok(promise) => drop(promise.catch(&on_resume_failed)),
                    Err(err) => re_log::error_once!(
                        "Resuming the AudioContext failed: {}",
                        re_web::Error::from(&err)
                    ),
                }
                if let Err(err) = play_silence(&context) {
                    re_log::error_once!(
                        "Playing silence to resume audio failed: {}",
                        re_web::Error::from(&err)
                    );
                }
            }
        });
        for event in GESTURE_EVENTS {
            // Capture phase, so the listener runs before the canvas stops propagation.
            window
                .add_event_listener_with_callback_and_bool(
                    event,
                    on_gesture.as_ref().unchecked_ref(),
                    true,
                )
                .map_err(|err| js_error("Listening for user gestures", &err))?;
        }

        Ok(Self {
            feed,
            on_gesture,
            _on_state_change: on_state_change,
        })
    }
}

/// Plays a single silent frame.
///
/// In Safari, starting a source during a user gesture also resumes a suspended context.
fn play_silence(context: &AudioContext) -> Result<(), JsValue> {
    let buffer = context.create_buffer(1, 1, context.sample_rate())?;
    let source = context.create_buffer_source()?;
    source.set_buffer(Some(&buffer));
    source.connect_with_audio_node(&context.destination())?;
    AudioScheduledSourceNode::start(&source)
}

fn on_slot_ended(feed: &Weak<RefCell<Feed>>, slot_index: usize) {
    let Some(feed) = feed.upgrade() else {
        return;
    };
    let mut feed = feed.borrow_mut();
    if let Err(err) = feed.play_slot(slot_index, false) {
        feed.fail(&err);
    }
}

impl Feed {
    /// Schedules every buffer, the first time this is called while the context runs.
    ///
    /// The first buffer is silence, so `fill` runs at most one buffer ahead of what is heard.
    fn start_if_running(&mut self) {
        if self.started || self.context.state() != AudioContextState::Running {
            return;
        }
        self.started = true;
        for slot_index in 0..NUM_BUFFERS {
            if let Err(err) = self.play_slot(slot_index, slot_index == 0) {
                self.fail(&err);
                return;
            }
        }
    }

    /// Fills the slot's buffer from `fill`, or with silence if `silence` is set,
    /// and schedules it after the previously scheduled buffer.
    fn play_slot(&mut self, slot_index: usize, silence: bool) -> Result<(), JsValue> {
        let Self {
            context,
            params,
            fill,
            next_start_secs,
            interleaved,
            channel,
            slots,
            ..
        } = self;
        let Some(slot) = slots.get_mut(slot_index) else {
            re_log::debug_panic!("Audio slot {slot_index} out of range");
            return Ok(());
        };

        if silence {
            interleaved.fill(0.0);
        } else {
            fill(params, interleaved);
        }
        for channel_index in 0..params.num_channels {
            for (out, frame) in std::iter::zip(
                channel.iter_mut(),
                interleaved.chunks_exact(params.num_channels),
            ) {
                *out = frame[channel_index];
            }
            #[expect(clippy::cast_possible_wrap, reason = "a handful of channels")]
            let channel_number = channel_index as i32;
            slot.buffer.copy_to_channel(channel, channel_number)?;
        }

        let source = context.create_buffer_source()?;
        source.set_buffer(Some(&slot.buffer));
        source.connect_with_audio_node(&context.destination())?;
        AudioScheduledSourceNode::set_onended(
            &source,
            Some(slot.on_ended.as_ref().unchecked_ref()),
        );

        // After a late callback the buffer starts at the current time, which leaves a gap.
        let start_secs = next_start_secs.max(context.current_time());
        AudioScheduledSourceNode::start_with_when(&source, start_secs)?;
        *next_start_secs = start_secs + params.frames_per_callback as f64 / params.sample_rate as f64;
        slot.source = Some(source);

        Ok(())
    }

    /// Stops playback and reports the failure, the first time only.
    fn fail(&mut self, err: &JsValue) {
        self.stop_sources();
        if let Some(on_failure) = self.on_failure.take() {
            on_failure(js_error("Scheduling audio", err));
        }
    }

    fn stop_sources(&mut self) {
        for slot in &mut self.slots {
            if let Some(source) = slot.source.take() {
                AudioScheduledSourceNode::set_onended(&source, None);
                AudioScheduledSourceNode::stop(&source).ok();
            }
        }
    }
}

fn js_error(what: &str, err: &JsValue) -> OutputError {
    OutputError::Device(format!("{what} failed: {}", re_web::Error::from(err)))
}
