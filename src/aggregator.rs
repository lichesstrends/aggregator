use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::model::{Counter, Key};
use crate::pgn::{
    elo_bucket, is_game_start, month_from_headers, opening_from_headers, parse_elo, parse_headers,
    result_from_headers,
};

pub struct Aggregator {
    pub map: HashMap<Key, Counter>,
}

impl Aggregator {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Process one game's lines: compute key, update counters.
    pub fn process_game(&mut self, game_lines: &[String]) {
        if game_lines.is_empty() {
            return;
        }
        let h = parse_headers(game_lines);

        let month = month_from_headers(&h);
        let opening = opening_from_headers(&h);
        let result = result_from_headers(&h);

        let w_elo = parse_elo(h.get("WhiteElo"));
        let b_elo = parse_elo(h.get("BlackElo"));

        let key = Key {
            month,
            opening,
            w_bucket: elo_bucket(w_elo),
            b_bucket: elo_bucket(b_elo),
        };

        let counter = self.map.entry(key).or_default();
        counter.add_result(&result);
    }

    /// Write CSV with percentages and raw games.
    pub fn write_csv(&self, out_path: &Path) -> io::Result<()> {
        let mut entries: Vec<_> = self.map.iter().collect();
        entries.sort_by_key(|(_, c)| std::cmp::Reverse(c.games));

        let mut f = File::create(out_path)?;
        writeln!(
            f,
            "month,opening,white_bucket,black_bucket,games,white_pct,black_pct,draw_pct"
        )?;

        for (k, c) in entries {
            let (w, b, d) = c.percentages();
            writeln!(
                f,
                "{},{},{},{},{},{:.3},{:.3},{:.3}",
                k.month,
                escape_csv(&k.opening),
                k.w_bucket,
                k.b_bucket,
                c.games,
                w,
                b,
                d
            )?;
        }
        Ok(())
    }

    pub fn total_games(&self) -> u64 {
        self.map.values().map(|c| c.games).sum()
    }
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Stream stdin PGN into games, aggregate, optionally write CSV, return games count.
pub fn run_stream_stdin(out_path: Option<&Path>) -> io::Result<usize> {
    let stdin = std::io::stdin();
    let reader = std::io::BufReader::new(stdin.lock());

    let mut agg = Aggregator::new();
    let mut current_game: Vec<String> = Vec::with_capacity(512);

    for line_res in reader.lines() {
        let line = line_res?;
        if is_game_start(&line) && !current_game.is_empty() {
            agg.process_game(&current_game);
            current_game.clear();
        }
        current_game.push(line);
    }
    if !current_game.is_empty() {
        agg.process_game(&current_game);
    }

    if let Some(p) = out_path {
        agg.write_csv(p)?;
    }

    Ok(agg.total_games() as usize)
}
