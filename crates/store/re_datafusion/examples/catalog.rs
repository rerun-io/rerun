use arrow::array::{Int32Array, StringArray};
use datafusion::{common::exec_datafusion_err, prelude::SessionContext};
use itertools::multizip;
use re_datafusion::DataFusionConnector;
use re_log_types::external::re_types_core::Loggable as _;
use re_protos::catalog::v1alpha1::EntryKind;
use re_tuid::Tuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let local_addr = "127.0.0.1:51234";

    let conn = tonic::transport::Endpoint::new(format!("http://{local_addr}"))?
        .connect()
        .await?;

    let mut df_connector = DataFusionConnector::new(&conn);

    let ctx = SessionContext::default();

    let _ = ctx.register_table("entries", df_connector.get_entry_list().await)?;

    let df = ctx.table("entries").await?;

    println!("Datasets listed in the catalog:");
    df.clone().show().await?;

    let datasets = df
        .select_columns(&["id", "name", "entry_kind"])?
        .collect()
        .await?;

    for dataset in datasets {
        let id_array: Vec<Option<Tuid>> = Tuid::from_arrow_opt(dataset.column(0))?;

        let name_array = dataset
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(exec_datafusion_err!("Unable to cast name to string"))?;

        let kind_array = dataset
            .column(2)
            .as_any()
            .downcast_ref::<Int32Array>()
            .ok_or(exec_datafusion_err!(
                "Unable to cast kind_array type to i32"
            ))?;

        for time_inc_tuple in multizip((id_array, name_array, kind_array)) {
            if let (Some(tuid), Some(name), Some(kind)) = time_inc_tuple {
                if kind != EntryKind::Dataset as i32 {
                    continue;
                }
                let dataset_entry = df_connector.get_dataset_entry(tuid).await?;

                if let Some(entry) = dataset_entry {
                    let registration_name = format!("{name}_partion_list");

                    let url = entry.dataset_handle.unwrap().dataset_url().to_owned();
                    println!("Partitions for dataset: {name}");
                    let _ = ctx.register_table(
                        &registration_name,
                        df_connector.get_partition_list(tuid, &url).await,
                    )?;

                    let df = ctx.table(registration_name).await?;

                    // TODO(jleibs): This is a hack to work around the fact that the schema is not
                    // Something is wrong with the schema of the empty table
                    if df.clone().count().await? == 0 {
                        println!("No partitions found for dataset: {name}");
                    } else {
                        df.show().await?;
                    }

                    // Not yet implemented in manifest_registry.rs
                    // println!("Partitions index:");
                    // let _ = ctx.register_table(
                    //     "partition_index",
                    //     df_connector.get_partition_index_list(tuid, &url),
                    // )?;

                    // ctx.table("partition_index").await?.show().await?;
                }
            }
        }
    }

    Ok(())
}
