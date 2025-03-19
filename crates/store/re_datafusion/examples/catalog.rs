use datafusion::prelude::SessionContext;
use re_datafusion::DataFusionConnector;
use re_protos::catalog::v1alpha1::catalog_service_client::CatalogServiceClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let local_addr = "127.0.0.1:51234";

    let conn = tonic::transport::Endpoint::new(format!("http://{local_addr}"))?
        .connect()
        .await?;
    let client = CatalogServiceClient::new(conn);

    let df_connector = DataFusionConnector::new(client);

    let ctx = SessionContext::default();

    let _ = ctx.register_table("redap_catalog", df_connector.get_datasets())?;

    let df = ctx.table("redap_catalog").await?;

    df.show().await?;

    Ok(())
}
