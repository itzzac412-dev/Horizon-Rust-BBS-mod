# Horizon BBS (native mod)

One native **horizon-mixins** bundle (`horizon_bbs`) containing:

| Module | What it is |
|---|---|
| `guns` | Pistol, rifle, sniper, shotgun: hitscan, spread, damage falloff, headshots, ammo, cooldown, reload |
| `items`, `player` | Ammo and mag items, inventory, crafting recipes, R-to-reload from hotbar/inventory |
| `models` | The sprite-sheet guns embedded as sprites and extruded into 3D meshes |
| `chroma`, `input` | White blocks recoloured with F10; key routing for R and F10 |
| `actors` | BBS-style actors that carry and use guns: aim, fire, reload, pick up, drop, hand over, auto-combat, tick-scripted playback |
| `alkaline` | Rust Alkaline v2 policy core (terrain/entity culling decisions), moved in unchanged in behaviour |

Built from the official Horizon native mod template. Pinned to Horizon
`ba6344a59cb9e755b730dcf02285fc0d33b97d3c` and horizon-mixins `d9c83d7`.

## Guns

| Gun | Damage / hit | Pellets | Max per trigger pull | Fire interval | Mag | Reload | Range | Headshot |
|---|---|---|---|---|---|---|---|---|
| Pistol | 6 | 1 | 6 | 0.40 s | 12 | 1.5 s | 40 | x1.5 |
| Rifle | 8 | 1 | 8 | 0.15 s | 30 | 2.5 s | 80 | x1.75 |
| Sniper | 40 | 1 | 40 (100 headshot) | 1.50 s | 5 | 3.5 s | 200 | x2.5 |
| Shotgun | 4.5 | 8 | 36 point blank | 1.00 s | 6 | 3.0 s | 20 | x1.25 |

Health is 20 (10 hearts). Numbers live in `GunKind::stats()`.

### 3D models and textures

The guns use the supplied sprite sheet: pistol = row 3 small pistol, rifle = AK,
sniper = wooden bolt-action, shotgun = pump shotgun. `models::mesh_for(kind)`
extrudes the sprite into a voxel mesh (one voxel per pixel, 3 deep, hidden faces
removed). The four sprites are embedded in `runtime.rs` as indexed-colour
consts, so the model data ships inside the `.hnmod`.

`assets/` holds the same guns for BBS/Blockbench: `textures/gun_*.png` (all 19
sprites, transparent) and `models/gun_*.obj` (the four 3D guns). Regenerate with
`python3 tools/gen_gun_assets.py SHEET.png` (needs numpy, scipy, Pillow). The
sheet has a fake checkerboard baked in; it is removed by colour, so a faint light
fringe remains on some edges. Long guns are rotated level (barrel left); the
pistol is rotated and mirrored the same way.

## Ammo, magazines and crafting (shapeless)

| Output | Ingredients | Yield |
|---|---|---|
| Heavy ammo (sniper) | 1 gold ingot + 2 iron ingots | 7 |
| Rifle ammo | 1 iron ingot + 1 gold nugget | 14 |
| Shotgun shells | 6 redstone dust + 3 iron nuggets | 3 |
| Pistol ammo | 3 iron nuggets | 9 |
| Rifle mag | 6 iron ingots + 1 diamond | 1 |
| Pistol mag | 3 iron ingots + 1 gold ingot | 1 |

**R** (`Player::press_reload`) takes ammo for the held gun from the hotbar
first, then the inventory, up to what the magazine has room for, then runs the
gun's reload time. Assumption: pistols and rifles also need their **mag item**
in the inventory to reload (the mag is reusable, not consumed); sniper and
shotgun do not use mags. Say if you want mags to work differently.

## Actors and guns

Actors **do not need ammo, mags or reloading**: they fire on cooldown only.
`actors::World` holds actors, dropped guns and a tick counter. Anything that
controls an actor sends an `ActorAction`: `Equip`, `Aim`, `Look`, `Move`,
`Fire`, `Pickup`/`Interact`, `Drop`, `GiveTo`, `Engage`. `Engage(Some(id))`
makes an actor aim and shoot on its own. `World::script(tick, actor, action)`
plays actions back on a given tick, like a BBS replay. Dead actors drop their
gun so others can pick it up.

## White chroma blocks (F10)

`chroma::ChromaWorld` holds white blocks. Pressing **F10** while looking at one
(`input::handle_key(Key::F10, ...)`, reach 6) advances it: white, 7 colours,
chroma-key green, chroma-key blue, back to the normal Minecraft block, then
white again. `Item::WhiteBlock` exists but has no recipe (none was given); give
it with a command or add a recipe.

## What is and is not connected

Honest status. **Written and unit-tested logic only; not yet run in the game.**

- The only hook target verified for the pinned commit is `Game::break_block`,
  and `before_break` is deliberately inert. Nothing in Horizon calls the guns,
  actors or Alkaline yet.
- To make this playable you still need hooks into Horizon's input (fire /
  R / F10 / interact keys, a keybinding for each), its entity list (so `Actor` maps to real BBS/Horizon
  entities and health), block raycasts (pass them as `block_distance`), and
  rendering (including drawing `models::mesh_for` in the hand, and
  drawing chroma blocks in their flat colour). Those targets must be found in the pinned Horizon source and
  recorded with source hashes in `compatibility.json`; do not guess them.
- Alkaline, as in v2, is a policy core and is not wired to the renderer, so it
  gives no FPS gain by itself. The macOS FPS tooling from `rust-alkaline-v2`
  lives in `tools/macos/` with its notes in `docs/FPS.md` and
  `docs/COMPATIBILITY.md` (not shipped inside the `.hnmod`; unrun on a real Mac).
- **"Import VIA stuff to BBS" is not done.** I could not tell what VIA refers
  to (ViaVersion/ViaFabric protocol translation? something else?) and none was
  supplied. A Java mod cannot be imported into this Rust mod; tell me exactly
  what you want from it.
- **The BBS Fabric jars from `Archive.zip` are not included and cannot be.**
  `bbs`, `irlite` and `bbs-snb-addon` are Java Fabric mods for Minecraft
  1.21.x. Horizon is a Rust client; it cannot load Java mods, and porting
  them (thousands of classes) is a separate project. This mod re-creates only
  the gun and actor behaviour in Rust, modelled on BBS's `GunItem`/`ActorEntity`
  concepts.

## Build and package

Requirements: Rust 1.95.0, Python 3.12+, Git.

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
python3 tools/test_template.py
python3 tools/package.py        # -> target/horizon_bbs-0.1.0.hnmod
```

Staging against Horizon (`tools/horizon.py stage|check|test|build`) is as in the
template; the native test filter is `game::horizon_bbs::tests`. The launcher
build must use the same Horizon and horizon-mixins pins as this bundle. See
[compatibility.json](compatibility.json).

License: MIT (template and Rust Alkaline are MIT).

## Credits

- Horizon native mod template (MIT) and Rust Alkaline v2 (MIT), merged here.
- Gun sprites come from a sprite sheet supplied by the repo owner; confirm you
  have the right to publish them before making the repository public.
- The BBS, IRLights and BBS S&B Addon jars are separate Java projects by their
  own authors and are not part of this repository.
