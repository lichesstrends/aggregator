use anyhow::Context;
use sqlx::{PgPool, SqlitePool};
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;

use crate::aggregator::AggMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Backend { Sqlite, Postgres }

pub enum Db {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

fn env_var(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

fn detect_backend_from_url(url: &str) -> anyhow::Result<Backend> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
        Ok(Backend::Postgres)
    } else if lower.starts_with("sqlite:") {
        Ok(Backend::Sqlite)
    } else {
        anyhow::bail!("Unsupported DATABASE_URL scheme: {}", url);
    }
}

pub async fn connect_from_env() -> anyhow::Result<Db> {
    let url = env_var("DATABASE_URL", "");
    if url.is_empty() {
        anyhow::bail!("DATABASE_URL not set");
    }
    let backend = detect_backend_from_url(&url)?;
    let max = env_var("DB_MAX_CONNECTIONS", "10").parse::<u32>().unwrap_or(10);

    Ok(match backend {
        Backend::Sqlite => {
            let pool = SqlitePoolOptions::new()
                .max_connections(max)
                .connect(&url)
                .await
                .with_context(|| "connecting to SQLite")?;
            Db::Sqlite(pool)
        }
        Backend::Postgres => {
            let pool = PgPoolOptions::new()
                .max_connections(max)
                .connect(&url)
                .await
                .with_context(|| "connecting to Postgres")?;
            Db::Postgres(pool)
        }
    })
}

pub async fn run_migrations(db: &Db) -> anyhow::Result<()> {
    match db {
        Db::Sqlite(pool) => sqlx::migrate!("./migrations").run(pool).await?,
        Db::Postgres(pool) => sqlx::migrate!("./migrations").run(pool).await?,
    }
    Ok(())
}

pub async fn already_ingested_months(db: &Db) -> anyhow::Result<std::collections::HashSet<String>> {
    let months: Vec<String> = match db {
        Db::Sqlite(pool) => {
            sqlx::query_scalar::<_, String>(
                "SELECT month FROM ingestions WHERE status = 'success'"
            )
            .fetch_all(pool)
            .await?
        }
        Db::Postgres(pool) => {
            sqlx::query_scalar::<_, String>(
                "SELECT month FROM ingestions WHERE status = 'success'"
            )
            .fetch_all(pool)
            .await?
        }
    };
    Ok(months.into_iter().collect())
}


pub async fn mark_ingestion_start(
    db: &Db, month: &str, url: &str, started_iso: &str
) -> anyhow::Result<()> {
    match db {
        Db::Sqlite(pool) => {
            sqlx::query(
                "INSERT INTO ingestions (month, url, started_at, status)
                 VALUES (?, ?, ?, 'started')
                 ON CONFLICT(month) DO UPDATE SET
                   url=excluded.url,
                   started_at=excluded.started_at,
                   status='started'"
            )
            .bind(month).bind(url).bind(started_iso)
            .execute(pool).await?;
        }
        Db::Postgres(pool) => {
            sqlx::query(
                "INSERT INTO ingestions (month, url, started_at, status)
                 VALUES ($1, $2, $3, 'started')
                 ON CONFLICT (month) DO UPDATE SET
                   url = EXCLUDED.url,
                   started_at = EXCLUDED.started_at,
                   status = 'started'"
            )
            .bind(month).bind(url).bind(started_iso)
            .execute(pool).await?;
        }
    }
    Ok(())
}

pub async fn mark_ingestion_finish(
    db: &Db, month: &str, games: i64, duration_ms: i64, status: &str, finished_iso: &str
) -> anyhow::Result<()> {
    match db {
        Db::Sqlite(pool) => {
            sqlx::query(
                "UPDATE ingestions
                   SET games = ?, duration_ms = ?, status = ?, finished_at = ?
                 WHERE month = ?"
            )
            .bind(games).bind(duration_ms).bind(status).bind(finished_iso).bind(month)
            .execute(pool).await?;
        }
        Db::Postgres(pool) => {
            sqlx::query(
                "UPDATE ingestions
                   SET games = $2, duration_ms = $3, status = $4, finished_at = $5
                 WHERE month = $1"
            )
            .bind(month).bind(games).bind(duration_ms).bind(status).bind(finished_iso)
            .execute(pool).await?;
        }
    }
    Ok(())
}

pub async fn bulk_upsert_aggregates(db: &Db, map: &AggMap) -> anyhow::Result<()> {
    if map.is_empty() { return Ok(()); }

    // deterministic order (optional)
    let mut rows: Vec<_> = map.iter().collect();
    rows.sort_by(|(ka, _), (kb, _)| {
        ka.month
            .cmp(&kb.month)
            .then_with(|| ka.eco_group.cmp(&kb.eco_group))
            .then_with(|| ka.w_bucket.cmp(&kb.w_bucket))
            .then_with(|| ka.b_bucket.cmp(&kb.b_bucket))
    });

    match db {
        // --- SQLite: single-row INSERT OR REPLACE inside one tx ---
        Db::Sqlite(pool) => {
            let mut tx = pool.begin().await?;
            let sql = "INSERT OR REPLACE INTO aggregates \
                       (month, eco_group, white_bucket, black_bucket, games, white_wins, black_wins, draws) \
                       VALUES (?, ?, ?, ?, ?, ?, ?, ?)";
            for (k, c) in &rows {
                sqlx::query(sql)
                    .bind(&k.month)
                    .bind(&k.eco_group)
                    .bind(k.w_bucket as i64)
                    .bind(k.b_bucket as i64)
                    .bind(c.games as i64)
                    .bind(c.white_wins as i64)
                    .bind(c.black_wins as i64)
                    .bind(c.draws as i64)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
        }

        // --- Postgres: single-row INSERT ... ON CONFLICT ... DO UPDATE inside one tx ---
        Db::Postgres(pool) => {
            let mut tx = pool.begin().await?;
            let sql = "INSERT INTO aggregates \
                       (month, eco_group, white_bucket, black_bucket, games, white_wins, black_wins, draws) \
                       VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
                       ON CONFLICT (month, eco_group, white_bucket, black_bucket) DO UPDATE SET
                         games = EXCLUDED.games,
                         white_wins = EXCLUDED.white_wins,
                         black_wins = EXCLUDED.black_wins,
                         draws = EXCLUDED.draws";
            for (k, c) in &rows {
                sqlx::query(sql)
                    .bind(&k.month)
                    .bind(&k.eco_group)
                    .bind(k.w_bucket as i32)
                    .bind(k.b_bucket as i32)
                    .bind(c.games as i64)
                    .bind(c.white_wins as i64)
                    .bind(c.black_wins as i64)
                    .bind(c.draws as i64)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
        }
    }

    Ok(())
}
