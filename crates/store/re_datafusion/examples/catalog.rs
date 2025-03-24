use arrow::array::{StringArray, StructArray, UInt64Array};
use datafusion::{common::exec_datafusion_err, prelude::SessionContext};
use itertools::multizip;
use re_datafusion::DataFusionConnector;
use re_log_types::external::re_tuid::Tuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let local_addr = "127.0.0.1:51234";

    let conn = tonic::transport::Endpoint::new(format!("http://{local_addr}"))?
        .connect()
        .await?;

    let df_connector = DataFusionConnector::new(&conn);

    let ctx = SessionContext::default();

    let _ = ctx.register_table("redap_catalog", df_connector.get_datasets())?;

    let df = ctx.table("redap_catalog").await?;

    println!("Datasets listed in the catalog:");
    df.clone().show().await?;

    let datasets = df.select_columns(&["id", "name"])?.collect().await?;

    for dataset in datasets {
        let id_array = dataset
            .column(0)
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or(exec_datafusion_err!("Unable to cast id to struct"))?;
        let time_ns_array = id_array
            .column(0)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or(exec_datafusion_err!("Unable to cast time of id to u64"))?;
        let inc_array = id_array
            .column(1)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or(exec_datafusion_err!("Unable to cast inc of id to u64"))?;

        let name_array = dataset
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(exec_datafusion_err!("Unable to cast name to string"))?;

        for time_inc_tuple in multizip((time_ns_array, inc_array, name_array)) {
            if let (Some(time_ns), Some(inc), Some(name)) = time_inc_tuple {
                let tuid = Tuid::from_nanos_and_inc(time_ns, inc);
                println!("Partitions for dataset: {name}");
                let _ =
                    ctx.register_table("partition_index", df_connector.get_partition_list(tuid))?;

                let df = ctx.table("partition_index").await?;

                df.show().await?;
            }
        }
    }

    // partition index list not implemented:  manifest_registry.rs:347

    // let _ = ctx.register_table(
    //     "partition_index_list",
    //     df_connector.get_partition_index_list(),
    // )?;

    // let df = ctx.table("partition_index_list").await?;

    // df.show().await?;

    Ok(())
}
