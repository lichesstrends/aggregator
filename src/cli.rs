use std::path::PathBuf;

pub struct Cli {
    pub out: Option<PathBuf>,
    pub ingest_remote: bool,
    pub until: Option<String>, // "YYYY-MM"
    pub list_url: String,      // list.txt endpoint
    pub verbose: bool,
    pub save: bool,
    pub help: bool,
}

pub fn parse() -> Cli {
    let mut out: Option<PathBuf> = None;
    let mut ingest_remote = false;
    let mut until: Option<String> = None;
    let mut list_url = "https://database.lichess.org/standard/list.txt".to_string();
    let mut verbose = false;
    let mut save = false;
    let mut help = false;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" | "-o" => {
                if let Some(p) = it.next() { out = Some(PathBuf::from(p)); }
            }
            "--ingest-remote" | "--remote" => ingest_remote = true,
            "--until" => {
                if let Some(m) = it.next() { until = Some(m); }
            }
            "--list-url" => {
                if let Some(u) = it.next() { list_url = u; }
            }
            "--verbose" | "-v" => verbose = true,
            "--save" => save = true,
            "--help" | "-h" => help = true,
            _ => {}
        }
    }

    Cli { out, ingest_remote, until, list_url, verbose, save, help }
}

pub fn print_help() {
    eprintln!(
r#"Lichess Trends Aggregator

Usage:
  Local file(s):
    aggregator [--out agg.csv] [file1.zst [file2.zst ...]] [--save] [-v]

  Remote ingest (stream from Lichess without saving .zst):
    aggregator --remote [--until YYYY-MM] [--out OUT] [--list-url URL] [--save] [-v]

Options:
  --remote, --ingest-remote   Stream monthly dumps (oldest -> newest).
  --until YYYY-MM             Stop after this month (inclusive) in remote mode.
  --out, -o PATH              CSV output.
                              - local: a file path (e.g., out/agg.csv)
                              - remote: directory for one CSV per month,
                                        or base filename (becomes base-YYYY-MM.ext)
  --list-url URL              Override the Lichess list.txt endpoint.
  -v, --verbose               Detailed timings/logs.
  --save                      Persist to DATABASE_URL (run migrations, write rows).
  -h, --help                  Show this help.

Notes:
  • Default is DRY-RUN: no DB connection, no migrations, no writes.
  • Configure defaults in config.toml (bucket size, list URL, batch size, rayon threads).
"#);
}
