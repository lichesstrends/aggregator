use std::collections::HashSet;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};

use crate::aggregator::AggMap;

fn env_var(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

pub async fn connect_from_env() -> anyhow::Result<SqlitePool> {
    let url = env_var("DATABASE_URL", "sqlite:///data/lichess.db");
    let max = env_var("DB_MAX_CONNECTIONS", "10")
        .parse::<u32>()
        .unwrap_or(10);

    // Optional tuning
    let jm = match env_var("SQLITE_JOURNAL_MODE", "WAL").to_uppercase().as_str() {
        "MEMORY" => SqliteJournalMode::Memory,
        "OFF"    => SqliteJournalMode::Off,
        "TRUNCATE" => SqliteJournalMode::Truncate,
        "PERSIST"  => SqliteJournalMode::Persist,
        "DELETE"   => SqliteJournalMode::Delete,
        _ => SqliteJournalMode::Wal,
    };
    let sync = match env_var("SQLITE_SYNCHRONOUS", "NORMAL").to_uppercase().as_str() {
        "OFF"   => SqliteSynchronous::Off,
        "FULL"  => SqliteSynchronous::Full,
        "EXTRA" => SqliteSynchronous::Extra,
        _ => SqliteSynchronous::Normal,
    };

    if !Sqlite::database_exists(&url).await.unwrap_or(false) {
        Sqlite::create_database(&url).await?;
    }

    let opts = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .journal_mode(jm)
        .synchronous(sync);
    // You could also set .pragma to tweak temp_store, cache_size, etc.

    let pool = SqlitePoolOptions::new()
        .max_connections(max)
        .connect_with(opts)
        .await?;

    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    // Embeds ./migrations at compile time
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

/// Bulk upsert all aggregate rows in a single transaction.
pub async fn bulk_upsert_aggregates(pool: &SqlitePool, map: &AggMap) -> anyhow::Result<()> {
    if map.is_empty() {
        return Ok(());
    }

    // Deterministic order (optional)
    let mut rows: Vec<_> = map.iter().collect();
    rows.sort_by(|(ka, _), (kb, _)| {
        ka.month
            .cmp(&kb.month)
            .then_with(|| ka.eco_group.cmp(&kb.eco_group))
            .then_with(|| ka.w_bucket.cmp(&kb.w_bucket))
            .then_with(|| ka.b_bucket.cmp(&kb.b_bucket))
    });

    // SQLite default max variables is 999.
    // We bind 8 columns per row => safe batch size ~120 rows.
    const COLS_PER_ROW: usize = 8;
    const SQLITE_MAX_VARS: usize = 999;
    let max_rows_per_batch = std::cmp::max(1, (SQLITE_MAX_VARS / COLS_PER_ROW) - 1);

    let mut tx = pool.begin().await?;

    for chunk in rows.chunks(max_rows_per_batch) {
        // INSERT OR REPLACE INTO aggregates (...) VALUES (?,?,?,?,?,?,?,?),(...) ;
        let mut sql = String::from(
            "INSERT OR REPLACE INTO aggregates \
             (month, eco_group, white_bucket, black_bucket, games, white_wins, black_wins, draws) \
             VALUES "
        );

        for i in 0..chunk.len() {
            if i > 0 { sql.push(','); }
            sql.push_str("(?,?,?,?,?,?,?,?)");
        }

        let mut q = sqlx::query(&sql);

        for (k, c) in chunk {
            q = q
                .bind(&k.month)
                .bind(&k.eco_group)
                .bind(k.w_bucket as i64)
                .bind(k.b_bucket as i64)
                .bind(c.games as i64)
                .bind(c.white_wins as i64)
                .bind(c.black_wins as i64)
                .bind(c.draws as i64);
        }

        q.execute(&mut *tx).await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn already_ingested_months(pool: &SqlitePool) -> anyhow::Result<HashSet<String>> {
    let rows = sqlx::query_scalar::<_, String>("SELECT month FROM ingestions WHERE status = 'success'")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().collect())
}

pub async fn mark_ingestion_start(pool: &SqlitePool, month: &str, url: &str, started_iso: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO ingestions (month, url, started_at, status)
         VALUES (?1, ?2, ?3, 'started')
         ON CONFLICT(month) DO UPDATE SET url=excluded.url, started_at=excluded.started_at, status='started'"
    )
    .bind(month).bind(url).bind(started_iso)
    .execute(pool).await?;
    Ok(())
}

pub async fn mark_ingestion_finish(pool: &SqlitePool, month: &str, games: i64, duration_ms: i64, status: &str, finished_iso: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE ingestions
           SET games = ?2,
               duration_ms = ?3,
               status = ?4,
               finished_at = ?5
         WHERE month = ?1"
    )
    .bind(month)
    .bind(games)
    .bind(duration_ms)
    .bind(status)
    .bind(finished_iso)
    .execute(pool).await?;
    Ok(())
}
