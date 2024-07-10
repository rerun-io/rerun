use datafusion::error::Result;

use re_datafusion::create_datafusion_context;

#[tokio::main]
async fn main() -> Result<()> {
    let ctx = create_datafusion_context()?;

    let df = ctx.sql("SELECT * FROM custom_table").await?;

    df.show().await?;
    Ok(())
}
