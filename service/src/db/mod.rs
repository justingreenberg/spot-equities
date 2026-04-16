pub mod models;
pub mod queries;

use sqlx::SqlitePool;

pub async fn init_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePool::connect(database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
