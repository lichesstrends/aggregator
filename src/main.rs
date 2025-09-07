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
mod local;

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

    // Remote base URL: config + optional CLI override
    let remote_base = if args.remote_url.is_empty() {
        cfg.remote_base_url.clone()
    } else {
        args.remote_url.clone()
    };

    // --- REMOTE MODE ---
    if args.ingest_remote {
        eprintln!("➡️ Remote ingest starting...");
        if args.save {
            let dbh = db::connect_from_env().await.expect("DB connect failed");
            db::run_migrations(&dbh).await.expect("DB migrations failed");

            vprintln!("remote: building plan from {}", remote_base);
            let plan = remote::build_plan(&dbh, &remote_base, args.since.as_deref(), args.until.as_deref())
                .await
                .expect("build plan failed");
            vprintln!("remote: plan size after filters = {}", plan.len());

            if plan.is_empty() {
                eprintln!("ℹ️ No remote files were processed.");
                return Ok(());
            }

            let mut processed = 0usize;
            for item in plan {
                // We expect a hash for remote items via sha256sums.txt
                let hash = item.hash.as_deref().unwrap_or_else(|| {
                    // Fallback (rare): stable pseudo-hash derived from URL
                    // (won’t dedupe with local files, but avoids panic)
                    // This string lives only in this scope; copy below.
                    "url_fallback_nohash"
                }).to_string();

                let start_iso = Utc::now().to_rfc3339();
                db::mark_ingestion_start(&dbh, &hash, &item.url, &start_iso)
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
                    &dbh, &hash, games as i64, dur_ms as i64, "success", &finish_iso,
                )
                .await
                .expect("mark finish failed");

                eprintln!("{} | hash={} | {:.3}s | games={}", item.month, &hash[0..8], (dur_ms as f64)/1000.0, games);
                processed += 1;
            }

            eprintln!("✅ Remote ingest completed ({} month{}).", processed, if processed==1 {""} else {"s"});
            return Ok(());
        } else {
            vprintln!("remote (dry-run): building plan (no DB) from {}", remote_base);
            let plan = remote::plan_no_db(&remote_base, args.since.as_deref(), args.until.as_deref())
                .await
                .expect("build plan (no DB) failed");

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

    // --- LOCAL MODE (compressed files) ---
    if !args.files.is_empty() {
        eprintln!("➡️ Local ingest starting...");
        if args.save {
            let dbh = db::connect_from_env().await.expect("DB connect failed");
            db::run_migrations(&dbh).await.expect("DB migrations failed");

            let mut processed = 0usize;
            for file in &args.files {
                if !file.exists() {
                    eprintln!("❌ file not found: {}", file.display());
                    continue;
                }
                let out_csv = make_out_path_for_file(args.out.as_deref(), file);

                let start_iso = Utc::now().to_rfc3339();
                // Process file and compute hash
                let (hash, map, games, dur_ms) =
                    match local::process_local_file(file, out_csv.as_deref(), &cfg) {
                        Ok((h, m, g, d)) => (h, m, g, d),
                        Err(e) => {
                            eprintln!("❌ failed to process {}: {}", file.display(), e);
                            continue;
                        }
                    };

                db::mark_ingestion_start(&dbh, &hash, &file.display().to_string(), &start_iso)
                    .await
                    .expect("mark start failed");

                db::bulk_upsert_aggregates(&dbh, &map, cfg.db_batch_rows)
                    .await
                    .expect("DB bulk upsert failed");

                let finish_iso = Utc::now().to_rfc3339();
                db::mark_ingestion_finish(
                    &dbh, &hash, games as i64, dur_ms as i64, "success", &finish_iso,
                )
                .await
                .expect("mark finish failed");

                eprintln!("{} | hash={} | {:.3}s | games={}", file.display(), &hash[0..8], (dur_ms as f64)/1000.0, games);
                processed += 1;
            }

            eprintln!("✅ Local ingest completed ({} file{}).", processed, if processed==1 {""} else {"s"});
            return Ok(());
        } else {
            // Dry-run: no DB
            let mut processed = 0usize;
            for file in &args.files {
                if !file.exists() {
                    eprintln!("❌ file not found: {}", file.display());
                    continue;
                }
                let out_csv = make_out_path_for_file(args.out.as_deref(), file);
                let (_hash, _map, games, dur_ms) =
                    match local::process_local_file(file, out_csv.as_deref(), &cfg) {
                        Ok((h, m, g, d)) => (h, m, g, d),
                        Err(e) => {
                            eprintln!("❌ failed to process {}: {}", file.display(), e);
                            continue;
                        }
                    };
                eprintln!("{} | {:.3}s | games={}", file.display(), (dur_ms as f64)/1000.0, games);
                processed += 1;
            }
            eprintln!("✅ Local ingest completed ({} file{}).", processed, if processed==1 {""} else {"s"});
            return Ok(());
        }
    }

    // --- Nothing to do? show help ---
    cli::print_help();
    Ok(())
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

fn make_out_path_for_file(base: Option<&Path>, file: &Path) -> Option<PathBuf> {
    base.map(|p| {
        let mut name = p.to_path_buf();
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
        if name.is_dir() {
            name.push(format!("{}.csv", stem));
            name
        } else if let Some(out_stem) = name.file_stem().and_then(|s| s.to_str()) {
            let ext = name.extension().and_then(|e| e.to_str()).unwrap_or("csv");
            let parent = name.parent().unwrap_or_else(|| Path::new("."));
            let mut newp = parent.to_path_buf();
            newp.push(format!("{}-{}.{}", out_stem, stem, ext));
            newp
        } else {
            name
        }
    })
}
