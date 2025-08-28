mod aggregator;
mod model;
mod pgn;

fn main() -> std::io::Result<()> {
    aggregator::run_stream_stdin()
}
