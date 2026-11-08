# Validation

Be careful with this file: it records only what was actually run.

| Check | Result |
|---|---|
| `python3 tools/package.py` | Run. Produces `target/horizon_bbs-0.1.0.hnmod` (sha256 in the hand-off notes) |
| Bracket/brace balance of `runtime.rs` | Run (crude check only) |
| `tools/gen_gun_assets.py` | Run: 19 sprites split, 4 levelled, textures/OBJ/Rust sprite consts written; sprites viewed by eye |
| `cargo fmt` / `clippy` / `cargo test` | **NOT RUN** - no Rust toolchain or network was available when this was written |
| `tools/test_template.py`, `test_stage.py`, `horizon.py check/test` | **NOT RUN** |
| In-game behaviour | **NOT RUN**; no hooks into guns/actors exist yet |

The unit tests in `runtime.rs` were written to pass by hand-checking the maths
(recipes and yields, hotbar-first ammo use, reload needing a mag, actors firing without ammo, F10 cycle, mesh face counts, damage ordering, falloff, spread cone, cooldown/reload, pickup/drop/give,
scripted playback, auto-combat kill, Alkaline decisions), but they have never
been compiled. Expect to fix small compile or clippy errors on first
`cargo test`.
