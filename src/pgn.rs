use std::collections::HashMap;

/// Parse PGN headers from a game's lines into a map (Tag -> Value).
pub fn parse_headers(game_lines: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in game_lines {
        let line = line.trim();
        if !(line.starts_with('[') && line.ends_with(']')) {
            // beyond headers
            continue;
        }
        // format: [Tag "Value"]
        // find first space
        if let Some(space_idx) = line.find(' ') {
            let tag = &line[1..space_idx];
            if let (Some(fq_rel), Some(lq)) = (line[space_idx..].find('"'), line.rfind('"')) {
                let fq = space_idx + fq_rel;
                if lq > fq {
                    let val = &line[(fq + 1)..lq];
                    map.insert(tag.to_string(), val.to_string());
                }
            }
        }
    }
    map
}

/// Consider a line that starts a new game.
pub fn is_game_start(line: &str) -> bool {
    line.starts_with("[Event ")
}

/// Extract YYYY-MM month from UTCDate or Date ("YYYY.MM.DD").
/// Returns "unknown" if absent/malformed.
pub fn month_from_headers(h: &HashMap<String, String>) -> String {
    let date = h.get("UTCDate").or_else(|| h.get("Date"));
    if let Some(d) = date {
        // expected "YYYY.MM.DD"
        if d.len() >= 7 && d.chars().nth(4) == Some('.') && d.chars().nth(7) == Some('.') {
            let y = &d[0..4];
            let m = &d[5..7];
            if y.chars().all(|c| c.is_ascii_digit()) && m.chars().all(|c| c.is_ascii_digit()) {
                return format!("{}-{}", y, m);
            }
        }
    }
    "unknown".to_string()
}

/// Opening name from headers; fallback to ECO; else "Unknown".
pub fn opening_from_headers(h: &HashMap<String, String>) -> String {
    if let Some(o) = h.get("Opening") {
        return o.clone();
    }
    if let Some(eco) = h.get("ECO") {
        return eco.clone();
    }
    "Unknown".to_string()
}

/// Result string (e.g., "1-0", "0-1", "1/2-1/2"), or "*" if absent.
pub fn result_from_headers(h: &HashMap<String, String>) -> String {
    h.get("Result").cloned().unwrap_or_else(|| "*".to_string())
}

/// Parse ELO as u16 if present and valid.
pub fn parse_elo(s: Option<&String>) -> Option<u16> {
    s.and_then(|x| x.parse::<u16>().ok())
}

/// Bucket ELO into 100-pt chunks: 2200..2299 => 2200, None => 0.
pub fn elo_bucket(elo: Option<u16>) -> u16 {
    elo.map(|e| (e / 100) * 100).unwrap_or(0)
}
