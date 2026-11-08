# Alkaline FPS guide (MacBook Air M2, macOS)

## What is actually limiting you (evidence, not guesses)

| Symptom | Cause | Source |
|---|---|---|
| FPS stuck at ~60 even with VSync off, even at tiny render distance | Horizon creates the Metal swapchain as `Fifo` (vsync) regardless of `enableVsync:false` | Horizon issues #1711 (M3 Air, macOS 26.2) and #11693 |
| FPS rises past the refresh rate after you change any setting in-game | The present mode is only applied on a settings change, not at startup | #11693 (one user's report; not yet confirmed on an M2) |
| 26 FPS at render distance 80 | Workload: ~25,000 chunks in view. Not a cap | your screenshot; Horizon renders ~190-300 FPS offscreen at distance 16 on an M3 Air (#1711) |

A 60 Hz panel can only *show* 60 unique frames per second. Rendering at 120 internally lowers input latency and smooths frame pacing, but you will not see 120 distinct frames, and with vsync off you may see tearing. If you want the coolest, quietest option, use a cap of 60.

## Install (nothing is overwritten; every change is backed up)

All scripts are in `tools/macos/`. Run from the project folder. Make them runnable once:

    chmod +x tools/macos/*.sh

(`chmod +x` only marks files as runnable.)

1. **Diagnose (read-only).** Reads settings and logs, changes nothing:

       tools/macos/alkaline-diagnose.sh

   Look at the "Surface line". `Fifo` confirms the known bug. Add `--gpu` for an optional GPU/CPU power sample; that one asks for your password because Apple's `powermetrics` needs root to read GPU counters.

2. **Measure BEFORE.** Plug in the charger, close other apps:

       tools/macos/alkaline-bench.sh --label before --rd 16

   It makes a backup, flies a fixed path at targets 60/90/120/144 and saves raw results to `~/.alkaline-bench/`.

3. **Preview the config change (dry run, writes nothing):**

       tools/macos/alkaline-fps-config.sh --fps 120

4. **Quit Horizon completely**, then apply (backs up first):

       tools/macos/alkaline-fps-config.sh --fps 120 --apply

   It changes exactly three values: `enableVsync:false`, `maxFps:120` (options.txt) and `presentMode:immediate` (horizon-graphics.txt). Other lines are untouched.

5. **Startup-bug workaround.** Start the game, load your world, then open Options and change any setting (for example move FOV one notch and back). Per #11693 this made the present mode take effect. Confirm with F3, or re-run the diagnose script after quitting and check the surface line and the fps in step 6.

6. **Measure AFTER** with `--label after` and compare the two result files. Achieved average FPS should track each target and never exceed it.

If the game has a different home folder than the scripts find, add `--home /path/to/folder` (the folder holding `options.txt` and `logs/`).

## Uninstall / undo

    tools/macos/alkaline-fps-config.sh --list
    tools/macos/alkaline-fps-config.sh --restore            # newest original backup
    tools/macos/alkaline-fps-config.sh --restore 20261107-124651   # a specific one

Restore copies the saved files back (and first saves your current ones as `*-pre-restore`). Nothing is ever deleted. To remove Alkaline itself, delete this folder yourself; the scripts install nothing elsewhere. Backups live in `~/.alkaline-backups/`, results in `~/.alkaline-bench/`.

## Reading results

* Achieved FPS at target 120 well below 120 -> workload-bound: lower render distance (try 24-32), or the chip is throttling (see diagnose: Low Power Mode, `CPU_Speed_Limit`).
* Achieved FPS far above the target -> the limiter is not engaged.
* Achieved FPS locked near 60 at every target -> present mode is still `Fifo`; redo step 5.
