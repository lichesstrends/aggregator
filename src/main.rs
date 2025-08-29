mod aggregator;
mod cli;
mod config;
mod db;
mod model;
mod pgn;
mod remote;

use std::path::{Path, PathBuf};
use chrono::Utc;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let args = cli::parse();
    let cfg = config::Config::load();

    if let Some(n) = cfg.rayon_threads {
        let _ = rayon::ThreadPoolBuilder::new().num_threads(n).build_global();
    }

    let dbh = db::connect_from_env().await.expect("DB connect failed");
    db::run_migrations(&dbh).await.expect("DB migrations failed");

    // pick list_url: CLI override > config
    let list_url = if args.list_url.is_empty() { cfg.list_url.clone() } else { args.list_url.clone() };

    if args.ingest_remote {
        let plan = remote::build_plan(&dbh, &list_url, args.until.as_deref())
            .await
            .expect("build plan failed");

        if plan.is_empty() {
            eprintln!("Nothing to ingest (already up to date).");
            println!("0");
            return Ok(());
        }

        for item in plan {
            let start_iso = Utc::now().to_rfc3339();
            db::mark_ingestion_start(&dbh, &item.month, &item.url, &start_iso)
                .await
                .expect("mark start failed");

            let out_csv: Option<PathBuf> = args.out.as_deref().map(|p| {
                let mut name = p.to_path_buf();
                if name.is_dir() {
                    name.push(format!("{}.csv", item.month));
                    name
                } else {
                    if let Some(stem) = name.file_stem().and_then(|s| s.to_str()) {
                        let ext = name.extension().and_then(|e| e.to_str()).unwrap_or("csv");
                        let parent = name.parent().unwrap_or_else(|| Path::new("."));
                        let mut newp = parent.to_path_buf();
                        newp.push(format!("{}-{}.{}", stem, item.month, ext));
                        newp
                    } else {
                        name
                    }
                }
            });

            let (map, games, dur_ms) =
                remote::stream_and_aggregate_async(&item.url, out_csv.as_deref(), &cfg)
                    .await
                    .expect("stream+aggregate failed");

            db::bulk_upsert_aggregates(&dbh, &map)
                .await
                .expect("DB bulk upsert failed");

            let finish_iso = Utc::now().to_rfc3339();
            db::mark_ingestion_finish(
                &dbh, &item.month, games as i64, dur_ms as i64, "success", &finish_iso,
            )
            .await
            .expect("mark finish failed");

            eprintln!("{} | {:.3}s | games={}", item.month, (dur_ms as f64)/1000.0, games);
        }

        println!("0");
        return Ok(());
    }

    // stdin mode
    let (map, total_games) =
        aggregator::aggregate_from_reader(std::io::BufReader::new(std::io::stdin().lock()), &cfg)?;
    db::bulk_upsert_aggregates(&dbh, &map).await.expect("DB bulk upsert failed");
    if let Some(out) = args.out.as_deref() {
        aggregator::write_csv(&map, Path::new(out)).expect("CSV write failed");
    }
    println!("{}", total_games);
    Ok(())
}
