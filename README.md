# Lichess Trends Aggregator (Prototype)

This project provides a **Rust-based parser** for large [Lichess](https://database.lichess.org/) monthly game archives.  
It is designed to work inside a **Docker + docker-compose** dev environment so you can easily process `.pgn.zst` files without needing Rust or zstd installed on your host.

## What it does

Currently, the Rust program reads a PGN stream from **stdin** and counts the number of games.

The input is expected to be a `.pgn.zst` file from Lichess.

## Setup

1. Make sure you have **Docker** and **docker-compose** installed.
2. Clone this repo and go into the project folder.
3. Build the dev container:

   ```bash
   docker compose build dev
   ```

4. Make the helper script executable:

   ```bash
   chmod +x dev.sh
   ```

## Usage

Place your Lichess `.pgn.zst` files in the project folder (or subfolders).

Run the helper script:

```bash
./dev.sh [file.pgn.zst]
```

- If no file is given, it defaults to:
  ```
  lichess_db_standard_rated_2013-01.pgn.zst
  ```
- You can also set an environment variable instead of passing an argument:

  ```bash
  LICHESS_FILE=dumps/lichess_db_standard_rated_2020-12.pgn.zst ./dev.sh
  ```

The script:
- Ensures the **dev container** is running
- Pipes the chosen `.zst` file through `zstdcat`
- Runs the Rust program with that decompressed PGN stream

## Example

Count games in a monthly dump:

```bash
./dev.sh lichess_db_standard_rated_2013-01.pgn.zst
```

## Project Structure

- `src/main.rs` — Rust program (streaming PGN parser)
- `Cargo.toml` — Rust project config
- `Dockerfile.dev` — dev image with Rust + zstd + cargo-watch
- `docker-compose.yml` — defines the dev container
- `dev.sh` — wrapper script to simplify running against `.pgn.zst`

---

### Notes

- All compilation happens **inside the dev container**. No Rust toolchain needed locally.
- Code changes are hot-rebuilt via `cargo-watch`.
- The goal is to gradually extend the Rust parser to compute useful statistics from the Lichess PGN archives.
