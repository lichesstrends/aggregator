# Development Notes

## Publishing a new release

1. **Update version** in `Cargo.toml`:
   ```toml
   version = "0.3.0"
   ```

2. **Build locally** to verify it compiles:
   ```bash
   ./build.sh
   ./target/lta --help
   ```

3. **Commit and push**:
   ```bash
   git add .
   git commit -m "v0.3.0: <description>"
   git push origin main
   ```

4. **Create and push tag**:
   ```bash
   git tag v0.3.0
   git push origin v0.3.0
   ```

   This triggers the `release.yml` workflow which builds and publishes a GitHub Release.

5. **Verify** the release appears in GitHub Releases with the binary attached.

## Re-tagging (if release failed)

If you need to redo a release:

```bash
# Delete tag locally and remotely
git tag -d v0.3.0
git push origin --delete v0.3.0

# Make fixes, commit, push
git add .
git commit -m "fix: ..."
git push origin main

# Recreate tag
git tag v0.3.0
git push origin v0.3.0
```

## Running locally

```bash
# Build
./build.sh

# Dry run (no DB writes)
./target/lta --remote --until 2013-02 -v

# Save to database
./target/lta --remote --save -v
```

Environment variables are loaded from `.env` automatically.
