# Developer notes

The [player manual](manual.md) describes gameplay. This document is a source map
and a short guide to the invariants most likely to affect a repair.

## Build prerequisites

Use a stable Rust toolchain. macOS builds need Xcode's command-line tools. On
Ubuntu, Macroquad's windowing and audio dependencies require these packages:

```sh
sudo apt install pkg-config libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev
```

Packaging also requires Python 3. Platform qualification and signing tools are
covered in the [release guide](release.md).

## Source layout

| Files in `src/` | Responsibility |
| --- | --- |
| `main.rs`, `controls.rs`, `player.rs` | Frame loop, input gating, movement, camera, and resets |
| `voxel.rs`, `world_gen.rs` | Materials, static grid, damage, structural connectivity, meshes, and level generation |
| `physics.rs` | Detached chunks, contacts, rubble support, impact damage, and visual effects |
| `weapon.rs`, `siege.rs` | Handheld weapons, C4, projectiles, deployed batteries, and secondary reactions |
| `campaign.rs` | Contract state, spending, timers, destruction, collateral, and grades |
| `fluid.rs` | Water, molten slag, toxic gas, and structural hazard feedback |
| `replay.rs` | Recorded entity snapshots, playback, and projectile tracking |
| `customizer.rs`, `ui.rs` | Workbench, HUD, help, and notifications |
| `audio.rs`, `music.rs`, `skyline.rs` | Procedural sound, music, and distant scenery |

`lib.rs` exposes the subsystems to tests. `main.rs` remains the authority for the
live frame order: physics, weapon, siege, and fluid updates precede structural
island extraction; detached islands are spawned into physics before campaign
grading. Help, the workbench, free cursor, and replay pause live simulation.

## Simulation invariants

- The grid uses 0.5 m voxels, with 192 × 192 horizontal cells and voxel Y in
  `[-48, 224)`. Use the constants and coordinate conversion functions in
  `voxel.rs`; subterranean Y is signed and storage applies an offset.
- Use grid mutation methods so health, mesh dirtiness, structural dirty bounds,
  and voxel counters remain consistent. `solid_count()` counts structural
  voxels above Y = 0; terrain excavation alone does not satisfy a demolition
  contract.
- Static damage and detached-chunk damage share material resistance rules.
  Geometry changes must preserve surviving chunk health and update mass,
  bounds, exposed faces, and rubble support. Bedrock is destructible with high
  resistance.
- Structural detachment must create falling chunks before grading. Precision
  contracts wait for falling debris to settle and score transformed settled
  voxel positions against the drop zone. Accepted shots and C4 charges consume
  budgets; changing a wire-saw plane does not.
- Collision and settlement are gameplay approximations. Preserve rotated
  geometry, external contact support, and contact tolerances when changing
  them; proximity to a chunk's own cached rubble is not external support.
- Cosmetic effects must not consume the random stream used by destruction
  physics. Reset all transient state when changing or restarting a level.
- Replay records entity positions and approximate rotated chunk bounds. It
  draws the current static terrain, not a historical copy of all destroyed
  voxels or particles.

## Tests

Run the normal checks in [CONTRIBUTING.md](https://github.com/steph4n-gh/voxel-demolition?tab=contributing-ov-file). Start with a
subsystem or integration target while developing a fix:

```sh
cargo test --locked --lib physics::tests -- --test-threads=1
cargo test --locked --test e2e_destruction -- --test-threads=1
```

Full generated-world campaign checks are deliberately ignored in the normal
suite. Run them explicitly when changing destruction, collapse, scoring, or
campaign behavior:

```sh
cargo test --locked --test corporate_precision -- --ignored --test-threads=1
cargo test --locked --lib campaign::tests -- --include-ignored --test-threads=1
```

The Corporate Drop fixture uses legal workbench settings and the live loop's
player, muzzle, cooldown, feedback, cost, and grading order. It asserts the
mission's unchanged limits and requires a settled win. Other campaign probes
also preserve failed strategies; a probe run is not proof that every contract
has been completed by a human player.

Subsystem unit tests live beside their implementations in `src/`. Most
integration fixtures in `tests/` use the same library. The three `g4_m4_*`
frustum harnesses import source modules to keep drawing disabled under
`cfg(test)`; their embedded unit tests repeat. Use `SoundSystem::empty()` in
headless fixtures, and run allocator and timing tests with one test thread.

## Performance and packaging

Reuse existing scratch buffers where practical, but do not promise that every
frame or structural operation allocates nothing. Initialization, mesh rebuilds,
UI text, chunk changes, and replay snapshots can allocate. Simulation timing
budgets do not measure rendered frame rate; retain the workload and host context
with a performance claim.

Release qualification and signed macOS packaging are described in
[the release guide](release.md). Keep local receipts under ignored
`dist/validation/`; Git history preserves previous documentation and code.
