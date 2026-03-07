use anyhow::Result;
use mae::request_context::RequestContext;

#[derive(Default, Clone)]
pub struct TestContext {}

pub type Ctx = RequestContext<mae::testing::context::TestContext<TestContext>>;

pub async fn get_context() -> Result<Ctx> {
    mae::testing::context::get_context::<TestContext>().await
}

/// Teardown hook used by `#[mae_test(teardown = ...)]`.
///
/// This ensures testcontainers are cleaned even if the test exits early.
pub async fn teardown() {
    mae::testing::container::teardown_all().await;
}

#[cfg(test)]
mod test_context {
    use anyhow::Result;
    use mae::testing::must::must_eq;
    use mae_macros::mae_test;
    use sqlx::Row;

    use super::get_context;

    #[cfg_attr(miri, ignore)]
    #[mae_test(docker, teardown = crate::common::context::teardown)]
    async fn parallelism() -> Result<()> {
        let ctx = get_context().await?;
        let mut conn = ctx.custom.scoped_connection().await?;

        let n: i32 = sqlx::query("SELECT 1").fetch_one(&mut conn).await?.get(0);
        must_eq(n, 1);

        Ok(())
    }

    #[cfg_attr(miri, ignore)]
    #[mae_test(docker, teardown = crate::common::context::teardown)]
    async fn uses_test_context_schema_isolation() -> Result<()> {
        let ctx = get_context().await?;
        let mut conn = ctx.custom.scoped_connection().await?;

        sqlx::query("SELECT 1").execute(&mut conn).await?;

        Ok(())
    }
}
