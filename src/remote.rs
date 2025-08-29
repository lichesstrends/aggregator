use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

use regex::Regex;
use tokio::task;

use crate::aggregator::{aggregate_from_reader, write_csv, AggMap};
use crate::config::Config;
use crate::db;

// ---- Types ----

pub struct PlanItem {
    pub month: String, // "YYYY-MM"
    pub url: String,
}

// ---- Helpers ----

fn parse_list_to_oldest(list_txt: &str) -> Vec<PlanItem> {
    // Lines like: https://.../lichess_db_standard_rated_YYYY-MM.pgn.zst
    let re = Regex::new(r"(\d{4}-\d{2})\.pgn\.zst$").unwrap();
    let mut items: Vec<PlanItem> = list_txt
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() { return None; }
            let month = re.captures(line)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())?;
            Some(PlanItem { month, url: line.to_string() })
        })
        .collect();

    // Newest-first on the site; we want oldest-first
    items.sort_by(|a, b| a.month.cmp(&b.month));
    items
}

fn norm_month(s: &str) -> Option<String> {
    // Accept "YYYY-MM", "YYYY-M", "YYYY/MM", "YYYY.MM"
    let s = s.trim();
    let parts: Vec<&str> = s.split(|c| c == '-' || c == '/' || c == '.').collect();
    if parts.len() < 2 { return None; }
    let y = parts[0];
    let m = parts[1];
    if y.len() != 4 || !y.chars().all(|c| c.is_ascii_digit()) { return None; }
    if !m.chars().all(|c| c.is_ascii_digit()) { return None; }
    let mi: u32 = m.parse().ok()?;
    if !(1..=12).contains(&mi) { return None; }
    Some(format!("{}-{:02}", y, mi))
}

async fn fetch_list(list_url: &str) -> anyhow::Result<String> {
    vprintln!("remote: GET {}", list_url);
    let t0 = Instant::now();
    let list_url_owned = list_url.to_string();
    let text = task::spawn_blocking(move || -> anyhow::Result<String> {
        let resp = reqwest::blocking::get(&list_url_owned)?.error_for_status()?;
        Ok(resp.text()?)
    }).await??;
    vprintln!(
        "remote: list.txt fetched in {:.3}s ({} bytes)",
        t0.elapsed().as_secs_f64(),
        text.len()
    );
    Ok(text)
}

// ---- Plans ----

/// Build ingest plan using DB (skips already-success months).
pub async fn build_plan(
    dbh: &crate::db::Db,
    list_url: &str,
    since: Option<&str>,
    until: Option<&str>,
) -> anyhow::Result<Vec<PlanItem>> {
    let text = fetch_list(list_url).await?;
    let mut items = parse_list_to_oldest(&text);
    vprintln!("remote: months available = {}", items.len());

    let since_n = since.and_then(norm_month);
    let until_n = until.and_then(norm_month);

    if let Some(ref since_m) = since_n {
        let before = items.len();
        items.retain(|it| it.month.as_str() >= since_m.as_str());
        vprintln!(
            "remote: filtered by since={} -> {} items (was {})",
            since_m, items.len(), before
        );
    }

    if let Some(ref until_m) = until_n {
        let before = items.len();
        items.retain(|it| it.month.as_str() <= until_m.as_str());
        vprintln!(
            "remote: filtered by until={} -> {} items (was {})",
            until_m, items.len(), before
        );
    }

    let t1 = Instant::now();
    let done = db::already_ingested_months(dbh).await?;
    let before = items.len();
    items.retain(|it| !done.contains(&it.month));
    vprintln!(
        "remote: filtered already-ingested -> {} items (was {}), query took {:.3}s",
        items.len(), before, t1.elapsed().as_secs_f64()
    );

    Ok(items)
}

/// Build plan without touching DB (dry-run path).
pub async fn plan_no_db(
    list_url: &str,
    since: Option<&str>,
    until: Option<&str>,
) -> anyhow::Result<Vec<PlanItem>> {
    let text = fetch_list(list_url).await?;
    let mut items = parse_list_to_oldest(&text);
    vprintln!("remote: months available = {}", items.len());

    let since_n = since.and_then(norm_month);
    let until_n = until.and_then(norm_month);

    if let Some(ref since_m) = since_n {
        let before = items.len();
        items.retain(|it| it.month.as_str() >= since_m.as_str());
        vprintln!(
            "remote: filtered by since={} -> {} items (was {})",
            since_m, items.len(), before
        );
    }

    if let Some(ref until_m) = until_n {
        let before = items.len();
        items.retain(|it| it.month.as_str() <= until_m.as_str());
        vprintln!(
            "remote: filtered by until={} -> {} items (was {})",
            until_m, items.len(), before
        );
    }
    Ok(items)
}

// ---- Streaming + aggregation ----

/// Stream one monthly .zst over HTTP, aggregate, optionally write CSV.
/// Returns (aggregate map, total games, elapsed_ms).
pub async fn stream_and_aggregate_async(
    url: &str,
    out_csv: Option<&Path>,
    cfg: &Config,
) -> anyhow::Result<(AggMap, usize, u128)> {
    let url_owned = url.to_string();
    let out_opt: Option<PathBuf> = out_csv.map(|p| p.to_path_buf());
    let cfg_cloned = cfg.clone();

    let (map, games, elapsed_ms) = task::spawn_blocking(move || -> anyhow::Result<(AggMap, usize, u128)> {
        let start = Instant::now();

        vprintln!("remote: HTTP GET {}", url_owned);
        let t_net = Instant::now();
        let resp = reqwest::blocking::get(&url_owned)?.error_for_status()?;
        vprintln!("remote: HTTP connected in {:.3}s", t_net.elapsed().as_secs_f64());

        let t_dec = Instant::now();
        let decoder = zstd::stream::Decoder::new(resp)?;
        vprintln!("remote: zstd decoder ready in {:.3}s", t_dec.elapsed().as_secs_f64());

        let reader = BufReader::new(decoder);
        vprintln!("remote: aggregation start");
        let (map, total_games) = aggregate_from_reader(reader, &cfg_cloned)?;
        vprintln!("remote: aggregation done; games={}", total_games);

        if let Some(csv_path) = out_opt.as_ref() {
            let t_csv = Instant::now();
            vprintln!("remote: writing CSV to {}", csv_path.display());
            write_csv(&map, csv_path)?;
            vprintln!("remote: CSV written in {:.3}s", t_csv.elapsed().as_secs_f64());
        }

        let dur = start.elapsed().as_millis();
        Ok((map, total_games, dur))
    })
    .await??;

    Ok((map, games, elapsed_ms))
}