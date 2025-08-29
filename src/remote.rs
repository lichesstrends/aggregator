use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

use regex::Regex;
use tokio::task;

use crate::aggregator::{aggregate_from_reader, write_csv, AggMap};
use crate::config::Config;
use crate::db;

/// A single monthly dump to process.
pub struct PlanItem {
    pub month: String, // "YYYY-MM"
    pub url: String,
}

/// Parse list.txt content (newest-first URLs) into oldest-first plan items.
fn parse_list_to_oldest(list_txt: &str) -> Vec<PlanItem> {
    // Lines look like: https://.../lichess_db_standard_rated_YYYY-MM.pgn.zst
    let re = Regex::new(r"(\d{4}-\d{2})\.pgn\.zst$").unwrap();
    let mut items: Vec<PlanItem> = list_txt
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let month = re
                .captures(line)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())?;
            Some(PlanItem {
                month,
                url: line.to_string(),
            })
        })
        .collect();

    // Site is newest-first; we want oldest-first
    items.sort_by(|a, b| a.month.cmp(&b.month));
    items
}

/// Build the list of months to ingest:
/// - fetch list.txt (blocking work wrapped in spawn_blocking)
/// - order oldest→newest
/// - drop months already ingested with status 'success'
/// - if `until` is Some("YYYY-MM"), keep only <= that month (inclusive)
pub async fn build_plan(
    dbh: &crate::db::Db,
    list_url: &str,
    until: Option<&str>,
) -> anyhow::Result<Vec<PlanItem>> {
    let list_url_owned = list_url.to_string();
    let text = task::spawn_blocking(move || -> anyhow::Result<String> {
        let resp = reqwest::blocking::get(&list_url_owned)?.error_for_status()?;
        Ok(resp.text()?)
    })
    .await??;

    let mut items = parse_list_to_oldest(&text);

    if let Some(until_m) = until {
        items.retain(|it| it.month.as_str() <= until_m);
    }

    let done = db::already_ingested_months(dbh).await?;
    items.retain(|it| !done.contains(&it.month));

    Ok(items)
}

/// Stream one monthly .zst over HTTP, aggregate (parallel inside), optionally write CSV.
/// Returns (aggregate map, total games, elapsed_ms).
pub async fn stream_and_aggregate_async(
    url: &str,
    out_csv: Option<&Path>,
    cfg: &Config,
) -> anyhow::Result<(AggMap, usize, u128)> {
    let url_owned = url.to_string();
    let out_opt: Option<PathBuf> = out_csv.map(|p| p.to_path_buf());
    let cfg_cloned = cfg.clone();

    let (map, games, elapsed_ms) =
        task::spawn_blocking(move || -> anyhow::Result<(AggMap, usize, u128)> {
            let start = Instant::now();

            // HTTP stream -> zstd decoder -> buffered reader
            let resp = reqwest::blocking::get(&url_owned)?.error_for_status()?;
            let decoder = zstd::stream::Decoder::new(resp)?;
            let reader = BufReader::new(decoder);

            let (map, total_games) = aggregate_from_reader(reader, &cfg_cloned)?;
            if let Some(csv_path) = out_opt.as_ref() {
                write_csv(&map, csv_path)?;
            }

            let dur = start.elapsed().as_millis();
            Ok((map, total_games, dur))
        })
        .await??;

    Ok((map, games, elapsed_ms))
}
