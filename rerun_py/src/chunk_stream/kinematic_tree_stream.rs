use std::sync::Arc;

use re_chunk::Chunk;
use re_log_types::TimePoint;
use re_sdk::external::re_importer::KinematicTree;

use super::error::ChunkPipelineError;
use super::{ChunkStream, ChunkStreamFactory};

/// Factory for creating chunk streams from an already parsed robot model.
pub(crate) struct KinematicTreeStreamFactory {
    tree: Arc<dyn KinematicTree>,
    include_joint_transforms: bool,
}

impl KinematicTreeStreamFactory {
    pub(crate) fn new(tree: Arc<dyn KinematicTree>, include_joint_transforms: bool) -> Self {
        Self {
            tree,
            include_joint_transforms,
        }
    }
}

impl ChunkStreamFactory for KinematicTreeStreamFactory {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let (tx, rx) = crossbeam::channel::bounded::<Result<Arc<Chunk>, ChunkPipelineError>>(
            super::CHUNK_CHANNEL_CAPACITY,
        );

        let tree = Arc::clone(&self.tree);
        let include_joint_transforms = self.include_joint_transforms;

        let format = tree.model_format();
        std::thread::Builder::new()
            .name(format!("{format}-chunk-source").to_lowercase())
            .spawn(move || {
                let result = tree.emit(
                    &mut |chunk| {
                        re_quota_channel::send_crossbeam(&tx, Ok(Arc::new(chunk))).ok();
                    },
                    &TimePoint::STATIC,
                    include_joint_transforms,
                );

                if let Err(err) = result {
                    re_quota_channel::send_crossbeam(
                        &tx,
                        Err(ChunkPipelineError::RobotModel {
                            format,
                            reason: format!("{err:#}"),
                        }),
                    )
                    .ok();
                }
            })
            .expect("Failed to spawn robot model emit thread");

        Ok(Box::new(KinematicTreeStream { rx }))
    }
}

struct KinematicTreeStream {
    rx: crossbeam::channel::Receiver<Result<Arc<Chunk>, ChunkPipelineError>>,
}

impl ChunkStream for KinematicTreeStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match self.rx.recv() {
            Ok(Ok(chunk)) => Ok(Some(chunk)),
            Ok(Err(err)) => Err(err),
            Err(crossbeam::channel::RecvError) => Ok(None),
        }
    }
}
