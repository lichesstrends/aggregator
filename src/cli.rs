use std::path::PathBuf;

pub struct Cli {
    pub out: Option<PathBuf>,
    pub ingest_remote: bool,
    pub since: Option<String>,   // "YYYY-MM" (lower bound, inclusive) for remote filtering only
    pub until: Option<String>,   // "YYYY-MM" (upper bound, inclusive) for remote filtering only
    pub remote_url: String,      // base URL override; expects {url}/list.txt and {url}/sha256sums.txt
    pub verbose: bool,
    pub save: bool,
    pub help: bool,
    pub files: Vec<PathBuf>,     // local files (compressed .zst) to process
}

pub fn parse() -> Cli {
    let mut out: Option<PathBuf> = None;
    let mut ingest_remote = false;
    let mut since: Option<String> = None;
    let mut until: Option<String> = None;
    let mut remote_url = String::new(); // default from config
    let mut verbose = false;
    let mut save = false;
    let mut help = false;
    let mut files = Vec::new();

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" | "-o" => {
                if let Some(p) = it.next() { out = Some(PathBuf::from(p)); }
            }
            "--ingest-remote" | "--remote" => ingest_remote = true,
            "--since" | "--from" => {
                if let Some(m) = it.next() { since = Some(m); }
            }
            "--until" => {
                if let Some(m) = it.next() { until = Some(m); }
            }
            "--remote-url" => {
                if let Some(u) = it.next() { remote_url = u; }
            }
            "--verbose" | "-v" => verbose = true,
            "--save" => save = true,
            "--help" | "-h" => help = true,
            "--" => { files.extend(it.map(PathBuf::from)); break; }
            _ if arg.starts_with('-') => { /* ignore unknown */ }
            other => files.push(PathBuf::from(other)),
        }
    }

    Cli { out, ingest_remote, since, until, remote_url, verbose, save, help, files }
}

pub fn print_help() {
    eprintln!(
r#"LichessTrends Aggregator

Usage:
  Local compressed file(s) (.zst):
    aggregator [--out OUT.csv|OUTDIR/] file1.zst [file2.zst ...] [--save] [-v]

  Remote ingest (stream from Lichess without saving the .zst):
    aggregator --remote [--since YYYY-MM] [--until YYYY-MM] [--out OUTDIR/] [--remote-url URL] [--save] [-v]

Options:
  --remote, --ingest-remote     Stream monthly dumps (oldest → newest).
  --since YYYY-MM, --from       Start from this month (inclusive) [remote filter only].
  --until YYYY-MM               Stop after this month (inclusive) [remote filter only].
  --out, -o PATH                CSV output.
                                - local: directory or file; if directory, one CSV per input.
                                - remote: directory for one CSV per month,
  --remote-url URL              Base URL that provides [URL]/list.txt and [URL]/sha256sums.txt
  -v, --verbose                 Detailed timings/logs.
  --save                        Persist to DATABASE_URL (run migrations, write rows).
  -h, --help                    Show this help.

Notes:
  • Default is DRY-RUN: no DB connection, no migrations, no writes.  
  • When --save is used, both remote and local runs are recorded in the ingestions table.
  • Ingestions are keyed by content hash (sha256 of the compressed file). We skip duplicates.
  • Configure processing and DB batch sizes in config.toml.
"#);
}
