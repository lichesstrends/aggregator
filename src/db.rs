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
/// We also compute normalized percentages here and store them.
pub async fn bulk_upsert_aggregates(pool: &SqlitePool, map: &AggMap) -> anyhow::Result<()> {
    if map.is_empty() {
        return Ok(());
    }

    // Collect rows in a deterministic order (optional)
    let mut rows: Vec<_> = map.iter().collect();
    rows.sort_by(|(ka, _), (kb, _)| {
        ka.month
            .cmp(&kb.month)
            .then_with(|| ka.opening.cmp(&kb.opening))
            .then_with(|| ka.w_bucket.cmp(&kb.w_bucket))
            .then_with(|| ka.b_bucket.cmp(&kb.b_bucket))
    });

    // SQLite default max variables is 999.
    // We bind 11 columns per row => 90 rows per statement is safe.
    const COLS_PER_ROW: usize = 11;
    const SQLITE_MAX_VARS: usize = 999;
    let max_rows_per_batch = std::cmp::max(1, (SQLITE_MAX_VARS / COLS_PER_ROW) - 1);

    let mut tx = pool.begin().await?;

    for chunk in rows.chunks(max_rows_per_batch) {
        // Build: INSERT OR REPLACE INTO aggregates (...) VALUES (?,?,?,?,?,?,?,?,?,?,?),(...) ;
        let mut sql = String::from(
            "INSERT OR REPLACE INTO aggregates \
             (month, opening, white_bucket, black_bucket, games, white_wins, black_wins, draws, white_pct, black_pct, draw_pct) \
             VALUES ",
        );

        for i in 0..chunk.len() {
            if i > 0 {
                sql.push(',');
            }
            sql.push_str("(?,?,?,?,?,?,?,?,?,?,?)");
        }

        let mut q = sqlx::query(&sql);

        for (k, c) in chunk {
            let (wp, bp, dp) = c.percentages();
            q = q
                .bind(&k.month)
                .bind(&k.opening)
                .bind(k.w_bucket as i64)
                .bind(k.b_bucket as i64)
                .bind(c.games as i64)
                .bind(c.white_wins as i64)
                .bind(c.black_wins as i64)
                .bind(c.draws as i64)
                .bind(wp)
                .bind(bp)
                .bind(dp);
        }

        q.execute(&mut *tx).await?;
    }

    tx.commit().await?;
    Ok(())
}
