// make vprintln! visible everywhere
#[macro_use]
mod verbose;

mod aggregator;
mod cli;
mod config;
mod db;
mod model;
mod pgn;
mod eco;
mod remote;

use std::path::{Path, PathBuf};
use chrono::Utc;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let args = cli::parse();
    if args.help {
        cli::print_help();
        return Ok(());
    }

    let cfg = config::Config::load();
    verbose::set(args.verbose);
    if let Some(n) = cfg.rayon_threads {
        let _ = rayon::ThreadPoolBuilder::new().num_threads(n).build_global();
    }

    // list_url lives in config; CLI --list-url can override
    let list_url = if args.list_url.is_empty() { cfg.list_url.clone() } else { args.list_url.clone() };

    // --- REMOTE MODE ---
    if args.ingest_remote {
        eprintln!("➡️ Remote ingest starting...");
        if args.save {
            // save: DB on, migrations, skip already ingested, upsert
            let dbh = db::connect_from_env().await.expect("DB connect failed");
            db::run_migrations(&dbh).await.expect("DB migrations failed");

            vprintln!("remote: building plan from {}", list_url);
            let plan = remote::build_plan(&dbh, &list_url, args.since.as_deref(), args.until.as_deref())
                .await
                .expect("build plan failed");
            vprintln!("remote: plan size after filters = {}", plan.len());

            if plan.is_empty() {
                eprintln!("ℹ️ No remote files were processed.");
                return Ok(());
            }

            let mut processed = 0usize;
            for item in plan {
                let start_iso = Utc::now().to_rfc3339();
                db::mark_ingestion_start(&dbh, &item.month, &item.url, &start_iso)
                    .await
                    .expect("mark start failed");

                let out_csv = make_monthly_out_path(args.out.as_deref(), &item.month);

                let (map, games, dur_ms) =
                    remote::stream_and_aggregate_async(&item.url, out_csv.as_deref(), &cfg)
                        .await
                        .expect("stream+aggregate failed");

                db::bulk_upsert_aggregates(&dbh, &map, cfg.db_batch_rows)
                    .await
                    .expect("DB bulk upsert failed");

                let finish_iso = Utc::now().to_rfc3339();
                db::mark_ingestion_finish(
                    &dbh, &item.month, games as i64, dur_ms as i64, "success", &finish_iso,
                )
                .await
                .expect("mark finish failed");

                eprintln!("{} | {:.3}s | games={}", item.month, (dur_ms as f64)/1000.0, games);
                processed += 1;
            }

            eprintln!("✅ Remote ingest completed ({} month{}).", processed, if processed==1 {""} else {"s"});
            return Ok(());
        } else {
            // DRY-RUN remote: no DB touches at all
            vprintln!("remote (dry-run): building plan (no DB) from {}", list_url);
            let plan = remote::plan_no_db(&list_url, args.since.as_deref(), args.until.as_deref())
                .await
                .expect("build plan (no DB) failed");
            vprintln!("remote (dry-run): items = {}", plan.len());

            if plan.is_empty() {
                eprintln!("ℹ️ No remote files were processed.");
                return Ok(());
            }

            let mut processed = 0usize;
            for item in plan {
                let out_csv = make_monthly_out_path(args.out.as_deref(), &item.month);
                let (_map, games, dur_ms) =
                    remote::stream_and_aggregate_async(&item.url, out_csv.as_deref(), &cfg)
                        .await
                        .expect("stream+aggregate failed (dry-run)");

                eprintln!("{} | {:.3}s | games={}", item.month, (dur_ms as f64)/1000.0, games);
                processed += 1;
            }

            eprintln!("✅ Remote ingest completed ({} month{}).", processed, if processed==1 {""} else {"s"});
            return Ok(());
        }
    }

    // --- LOCAL (stdin) MODE ---
    eprintln!("➡️ Local ingest starting...");
    if args.save {
        // connect + upsert
        let dbh = db::connect_from_env().await.expect("DB connect failed");
        db::run_migrations(&dbh).await.expect("DB migrations failed");

        let (map, total_games) =
            aggregator::aggregate_from_reader(std::io::BufReader::new(std::io::stdin().lock()), &cfg)?;
        db::bulk_upsert_aggregates(&dbh, &map, cfg.db_batch_rows).await.expect("DB bulk upsert failed");
        if let Some(out) = args.out.as_deref() {
            aggregator::write_csv(&map, Path::new(out)).expect("CSV write failed");
        }
        println!("{}", total_games);
        eprintln!("✅ Local ingest completed.");
        return Ok(());
    } else {
        // dry-run: just count + optional CSV
        let (map, total_games) =
            aggregator::aggregate_from_reader(std::io::BufReader::new(std::io::stdin().lock()), &cfg)?;
        if let Some(out) = args.out.as_deref() {
            aggregator::write_csv(&map, Path::new(out)).expect("CSV write failed");
        }
        println!("{}", total_games);
        eprintln!("✅ Local ingest completed.");
        return Ok(());
    }
}

fn make_monthly_out_path(base: Option<&Path>, month: &str) -> Option<PathBuf> {
    base.map(|p| {
        let mut name = p.to_path_buf();
        if name.is_dir() {
            name.push(format!("{}.csv", month));
            name
        } else if let Some(stem) = name.file_stem().and_then(|s| s.to_str()) {
            let ext = name.extension().and_then(|e| e.to_str()).unwrap_or("csv");
            let parent = name.parent().unwrap_or_else(|| Path::new("."));
            let mut newp = parent.to_path_buf();
            newp.push(format!("{}-{}.{}", stem, month, ext));
            newp
        } else {
            name
        }
    })
}
