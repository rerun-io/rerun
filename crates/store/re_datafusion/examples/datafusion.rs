use std::sync::Arc;

use re_chunk::{
    external::re_log_types::example_components::MyPoint, Chunk, EntityPath, RowId, TimePoint,
    Timeline,
};
use re_chunk_store::ChunkStore;
use re_datafusion::create_datafusion_context;
use re_log_types::example_components::MyLabel;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let entity_path: EntityPath = "some_entity".into();

    let timeline_frame = Timeline::new_sequence("frame");
    let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);

    let point1 = MyPoint::new(1.0, 2.0);
    let point2 = MyPoint::new(3.0, 4.0);
    let point3 = MyPoint::new(5.0, 6.0);
    let label1 = MyLabel("point1".to_owned());
    let label2 = MyLabel("point2".to_owned());
    let label3 = MyLabel("point3".to_owned());

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );

    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batches(
            RowId::new(),
            timepoint.clone(),
            [&[point1, point2] as _, &[label1, label2] as _],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batches(
            RowId::new(),
            timepoint.clone(),
            [&[point3] as _, &[label3] as _],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    let ctx = create_datafusion_context(store)?;

    let df = ctx.sql("SELECT * FROM custom_table").await?;

    df.show().await?;
    Ok(())
}
