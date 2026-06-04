use anyhow::Result;
use mae::context::RequestContext;

#[derive(Default, Clone)]
pub struct TestContext {}

pub type Ctx = RequestContext<mae::testing::context::TestContext<TestContext>>;

pub async fn get_context<'c>() -> Result<Ctx> {
    mae::testing::context::get_context::<TestContext>().await
}

#[cfg(test)]
mod test_context {
    use anyhow::Result;
    use mae::testing::must::must_eq;
    use mae_macros::mae_test;
    use sqlx::Row;

    use super::get_context;

    #[cfg_attr(miri, ignore)]
    #[mae_test(docker, teardown = mae::testing::container::teardown_all)]
    async fn parallelism() -> Result<()> {
        let ctx = get_context().await?;

        let n = ctx
            .pg_context
            .with_tx(|tx| {
                Box::pin(async move {
                    let n = sqlx::query("SELECT 1").fetch_one(&mut **tx).await?.get(0);
                    Ok(n)
                })
            })
            .await?;
        must_eq(n, 1);

        Ok(())
    }

    #[cfg_attr(miri, ignore)]
    #[mae_test(docker, teardown = mae::testing::container::teardown_all)]
    async fn uses_test_context_schema_isolation() -> Result<()> {
        let ctx = get_context().await?;

        ctx.pg_context
            .with_tx(|tx| {
                Box::pin(async move {
                    sqlx::query("SELECT 1").execute(&mut **tx).await?;
                    Ok(())
                })
            })
            .await?;

        Ok(())
    }
}
