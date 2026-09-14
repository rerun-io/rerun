use crate::execute::execute;
use std::collections::VecDeque;

use re_chunk::Chunk;

use crate::{LeRobotError, plan::EpisodePlan};

pub(crate) struct EpisodeChunkIterator {
    plan: EpisodePlan,
    next_feature: usize,
    pending: VecDeque<Chunk>,
}

impl EpisodeChunkIterator {
    pub(crate) fn new(plan: EpisodePlan) -> Self {
        Self {
            plan,
            next_feature: 0,
            pending: VecDeque::new(),
        }
    }
}

impl Iterator for EpisodeChunkIterator {
    type Item = Result<Chunk, LeRobotError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(chunk) = self.pending.pop_front() {
                return Some(Ok(chunk));
            }
            let feature = self.plan.features.get(self.next_feature)?;
            self.next_feature += 1;
            match execute(feature, &self.plan) {
                Ok(chunks) => self.pending.extend(chunks),
                Err(err) => return Some(Err(err)),
            }
        }
    }
}
