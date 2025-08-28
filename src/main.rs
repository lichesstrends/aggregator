use std::io::{self, BufRead};

/// Simple container for any per-run state (e.g., counters, stats)
struct RunState {
    games: u64,
}

impl RunState {
    fn new() -> Self {
        Self { games: 0 }
    }
}

/// Process one game's lines (PGN block). Extend this later to parse headers/moves.
fn process_game(game_lines: &[String], state: &mut RunState) {
    // For now we just count; you could parse headers here if needed.
    // Example: detect malformed game if no moves, etc.
    if !game_lines.is_empty() {
        state.games += 1;
    }
}

/// Return true if this line indicates the start of a new PGN game.
fn is_game_start(line: &str) -> bool {
    line.starts_with("[Event ")
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let reader = io::BufReader::new(stdin.lock());

    let mut state = RunState::new();
    let mut current_game: Vec<String> = Vec::with_capacity(256);

    for line_res in reader.lines() {
        let line = line_res?;

        // If we hit a new [Event ...] header and already buffered lines,
        // finalize the previous game and start a new one.
        if is_game_start(&line) && !current_game.is_empty() {
            process_game(&current_game, &mut state);
            current_game.clear();
        }

        current_game.push(line);
    }

    // Flush last buffered game at EOF
    if !current_game.is_empty() {
        process_game(&current_game, &mut state);
    }

    println!("{}", state.games);
    Ok(())
}