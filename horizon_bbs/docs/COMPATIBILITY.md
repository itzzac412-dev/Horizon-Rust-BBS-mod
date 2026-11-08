# Compatibility

| Component | Status |
|---|---|
| `tools/macos/*.sh` config, diagnose, bench | Written for macOS (bash, uses only built-in tools; no Python/brew required). File-editing logic tested on fake files in Linux: dry run, apply, append of missing keys, no-trailing-newline files, repeated restore. **Not yet run on a real Mac or against the real game.** |
| Key names `enableVsync`, `maxFps`, `presentMode` | Taken from Horizon issue #11693 (options.txt / horizon-graphics.txt). Whether your build uses identical names/values is unverified until the diagnose script shows them in your files. |
| `horizon-ctl` commands used (`launch`, `options-set`, `fly`, `quit`) | From Horizon's `docs/native-client.md`. Result JSON field names are undocumented here, so results are stored raw. |
| Game home on macOS | Not documented. Scripts search `$HORIZON_HOME`, `~/.local/share/horizon` and `~/Library/Application Support` (read-only, depth 4); override with `--home`. |
| Native mod (`mixins/rust_alkaline`) | Pinned to Horizon `ba6344a`. Your launcher build is `6475f62` (mixins `d93dcab6bc82`); the pins are NOT interchangeable. The policy code is not wired to the renderer and does nothing in-game. |
| Horizon's own docs | `native-client.md` calls macOS "out of scope" for the Linux/NVIDIA-first client, while the README lists macOS Apple Silicon via the launcher. Expect rough edges. |
