use std::sync::Arc;

use re_chunk::Chunk;
use re_log_types::TimePoint;
use re_sdk::external::re_importer::UrdfTree;

use super::error::ChunkPipelineError;
use super::{ChunkStream, ChunkStreamFactory};

/// Factory for creating chunk streams from an already parsed URDF tree.
pub(crate) struct UrdfTreeStreamFactory {
    tree: Arc<UrdfTree>,
    include_joint_transforms: bool,
}

impl UrdfTreeStreamFactory {
    pub(crate) fn new(tree: Arc<UrdfTree>, include_joint_transforms: bool) -> Self {
        Self {
            tree,
            include_joint_transforms,
        }
    }
}

impl ChunkStreamFactory for UrdfTreeStreamFactory {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let (tx, rx) = crossbeam::channel::bounded::<Result<Arc<Chunk>, ChunkPipelineError>>(
            super::CHUNK_CHANNEL_CAPACITY,
        );

        let tree = Arc::clone(&self.tree);
        let include_joint_transforms = self.include_joint_transforms;

        std::thread::Builder::new()
            .name("urdf-chunk-source".into())
            .spawn(move || {
                let result = tree.emit(
                    &mut |chunk| {
                        re_quota_channel::send_crossbeam(&tx, Ok(Arc::new(chunk))).ok();
                    },
                    &TimePoint::default(),
                    include_joint_transforms,
                );

                if let Err(err) = result {
                    re_quota_channel::send_crossbeam(
                        &tx,
                        Err(ChunkPipelineError::Urdf {
                            reason: format!("Failed to stream URDF: {err}"),
                        }),
                    )
                    .ok();
                }
            })
            .expect("Failed to spawn URDF emit thread");

        Ok(Box::new(UrdfTreeStream { rx }))
    }
}

struct UrdfTreeStream {
    rx: crossbeam::channel::Receiver<Result<Arc<Chunk>, ChunkPipelineError>>,
}

impl ChunkStream for UrdfTreeStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match self.rx.recv() {
            Ok(Ok(chunk)) => Ok(Some(chunk)),
            Ok(Err(err)) => Err(err),
            Err(crossbeam::channel::RecvError) => Ok(None),
        }
    }
}
