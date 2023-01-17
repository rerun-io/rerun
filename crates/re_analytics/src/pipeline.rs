use std::{
    fs::File,
    time::{Duration, Instant},
};

use crossbeam::channel;
use re_log::error;

use crate::{Config, Event};

// TODO: let's say this is specifically our _native posthog_ pipeline

// TODO: web impl (how does one POST when in web?)
// TODO: endpoint configuration would ideally not live in code...
// TODO: what do we do on web? do we even send stats when running on web?

// ---

#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

// TODO: do we want a singleton? do we just pass it around in ctx? let's just do ctx for now

#[derive(Debug)]
pub struct EventPipeline {
    // TODO: not cloning this everytime
    pub analytics_id: String,
    pub session_id: String,

    event_tx: channel::Sender<Event>,
}

// TODO:
// - dump to storage (file on native, localstore on web)

impl EventPipeline {
    pub fn new(config: &Config, tick: Duration) -> Result<Self, PipelineError> {
        let (event_tx, event_rx) = channel::unbounded(); // TODO: bounded?

        // let (file_tx, file_rx) = channel::unbounded(); // TODO: bounded?

        // TODO: try to send on shutdown as best as possible

        let data_path = config.data_path().to_owned();
        // TODO: during boot, push existing files into the pipe
        // TODO: file names are session IDs, which are tuids, which are sorted

        let file_path = data_path.join(format!("{}.json", config.session_id));
        let mut file = File::create(file_path)?;

        let is_first_run = config.is_first_run();

        // TODO: maybe we do it all in one thread tho?
        // TODO: when do we join this one?
        let file_writer = std::thread::spawn(move || {
            let mut tick_id = 1u64;
            let mut last_tick = Instant::now();

            for event in event_rx {
                if let Err(err) = serde_json::to_writer(&mut file, &event) {
                    // TODO: we're not gonna have a good time if the write fails halfway... then again
                    // there's really no reason it should, so...
                    error!(%err, "couldn't write to analytics data file");

                    // TODO: i guess we truncate it and move on then?
                }

                if !is_first_run && Instant::now().duration_since(last_tick) >= tick {
                    match file.set_len(0) {
                        Ok(_) => {
                            tick_id += 1;
                            last_tick = Instant::now();
                        }
                        Err(err) => {
                            error!(%err, "couldn't truncate analytics data file");
                        }
                    }
                }
            }
        });

        // let file_uploader = std::thread::spawn(move || {
        //     let mut last_tick = Instant::now();
        //     for event in event_rx {
        //         if Instant::now().duration_since(last_tick) >= tick {
        //             // TODO: close file, send it to other thread
        //         }
        //     }
        // });

        Ok(Self {
            analytics_id: config.analytics_id.clone(),
            session_id: config.session_id.to_string(),
            event_tx,
        })
    }

    // TODO: there's gonna be some dedup mess in here

    pub fn record(&self, event: Event) {
        self.event_tx
            .send(event)
            // TODO: can only fail if we close the channel, which we don't.
            .unwrap();
    }
}

// ---

struct FileSink {}

struct PostHogUploader {}
