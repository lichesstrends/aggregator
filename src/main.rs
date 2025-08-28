mod aggregator;
mod cli;
mod db;
mod model;
mod pgn;

use std::path::Path;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok(); // load .env if present
    let args = cli::parse();

    // 1) Parse+aggregate from stdin (blocking read is fine in this simple tool)
    let (agg_map, total_games) = aggregator::aggregate_stream_stdin()?;

    // 2) DB connect + migrations
    let pool = db::connect_from_env().await.expect("DB connect failed");
    db::run_migrations(&pool).await.expect("DB migrations failed");

    // 3) Bulk upsert all aggregates in a single transaction
    db::bulk_upsert_aggregates(&pool, &agg_map)
        .await
        .expect("DB bulk upsert failed");

    // 4) Optional CSV output
    if let Some(out) = args.out.as_deref() {
        aggregator::write_csv(&agg_map, Path::new(out)).expect("CSV write failed");
    }

    // Print only number of games (so dev.sh can show "file | time | games=n")
    println!("{}", total_games);
    Ok(())
}
