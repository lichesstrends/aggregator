use std::collections::HashMap;

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

    /// Process one game's lines: pull needed tags, compute key, update counters.
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

    /// Emit CSV with normalized percentages (keeps raw games).
    pub fn print_csv(&self) {
        println!("month,opening,white_bucket,black_bucket,games,white_pct,black_pct,draw_pct");
        let mut entries: Vec<_> = self.map.iter().collect();
        // sort by games desc for nicer viewing
        entries.sort_by_key(|(_, c)| std::cmp::Reverse(c.games));

        for (k, c) in entries {
            let (w, b, d) = c.percentages();
            println!(
                "{},{},{},{},{},{:.3},{:.3},{:.3}",
                k.month, escape_csv(&k.opening), k.w_bucket, k.b_bucket, c.games, w, b, d
            );
        }
    }

    /// Convenience function for the specific query: black 2200–2299 win % on Sicilian.
    /// We treat "Sicilian" as any opening string containing "sicilian" (case-insensitive).
    pub fn print_black_2200_sicilian_win_pct(&self) {
        let mut total = Counter::default();

        for (k, c) in &self.map {
            if k.b_bucket == 2200 && k.opening.to_lowercase().contains("sicilian") {
                total.games += c.games;
                total.white_wins += c.white_wins;
                total.black_wins += c.black_wins;
                total.draws += c.draws;
            }
        }

        let (_, black_pct, _) = total.percentages();
        println!(
            "Black 2200–2299 win % on Sicilian (all months, all white buckets): {:.3}% (n={})",
            black_pct, total.games
        );
    }
}

fn escape_csv(s: &str) -> String {
    // Quote if needed
    if s.contains(',') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Stream stdin PGN into games and feed the aggregator.
pub fn run_stream_stdin() -> std::io::Result<()> {
    use std::io::{self, BufRead};

    let stdin = io::stdin();
    let reader = io::BufReader::new(stdin.lock());

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

    // Output
    agg.print_csv();
    agg.print_black_2200_sicilian_win_pct();

    Ok(())
}
