mod aggregator;
mod cli;
mod model;
mod pgn;

use std::path::Path;

fn main() -> std::io::Result<()> {
    let args = cli::parse();
    // Run the aggregator on stdin, optionally writing CSV
    let total_games = aggregator::run_stream_stdin(args.out.as_deref().map(Path::new))?;
    // Print only the game count (so wrappers can consume it)
    println!("{}", total_games);
    Ok(())
}
