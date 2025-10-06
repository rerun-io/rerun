use datafusion::prelude::SessionContext;
use re_datafusion::RedapCatalogProvider;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // NOTE: The entire TLS stack expects this global variable to be set. It doesn't matter
    // what we set it to. But we have to set it, or we will crash at runtime, as soon as
    // anything tries to do anything TLS-related.
    // This used to be implicitly done by `object_store`, just by virtue of depending on it,
    // but we removed that unused dependency, so now we must do it ourselves.
    _ = rustls::crypto::ring::default_provider().install_default();

    let local_addr = "rerun+http://127.0.0.1:51234";

    let connection_registry = re_redap_client::ConnectionRegistry::new();

    let client = connection_registry.client(local_addr.parse()?).await?;

    let ctx = SessionContext::default();
    ctx.register_catalog(
        "datafusion",
        Arc::new(RedapCatalogProvider::new(
            Some("datafusion"),
            client,
            tokio::runtime::Handle::current(),
        )),
    );

    let df = ctx.table("__entries").await?;

    println!("Datasets listed in the catalog:");
    df.show().await?;

    Ok(())
}
