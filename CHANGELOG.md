# Changelog

## 0.2.5 - 2026-10-10

### Changed

- Regenerate the player PDF during packaging and export identical copies for the
  app, ZIP and downloadable manual.
- Check manual source hashes, PDF navigation, ZIP checksums and live links before
  creating a release draft and again before publishing its downloaded assets.
- Keep Developer ID signing and Apple notarization local; remove the hosted
  signing workflow.
- Add a source-linked engineering article and gameplay gallery.

### Fixed

- Repair GitHub documentation navigation, manual and source links, and the
  engineering diagram's rendering.
- Use GitHub's current README overview URL in the player PDF.

## 0.2.4 - 2026-10-10

### Changed

- Add GitHub-hosted macOS checks and a manual workflow for signed, notarized
  draft releases. Cloud signing requires separately configured credentials.
- Support an explicit notarytool Keychain for isolated release runners.
- Remove device metadata from manual screenshots and regenerate the player PDF.

## 0.2.3 - Release candidate - 2026-10-10

### Changed

- Ship one optimized release build. Paired CPU workloads showed the former
  size-focused build was substantially slower for a small download saving.
- Add a complete illustrated player manual, a printable PDF, and concise
  developer and release guides. Remove obsolete overlapping documentation.
- Reuse the library in 20 integration targets, preserving every gameplay check
  while avoiding thousands of repeated subsystem test executions.
- Include the player manual in macOS app Resources and record its digest in
  release provenance.

### Fixed

- Label the H-key fluid action as a hazard discharge, matching its behavior.
- Retain Makepad's MIT notice for copied code in Miniquad's Apple platform helpers.

## 0.2.2 - Release candidate - 2026-10-10

### Fixed

- Sweep player dash movement and check ceilings to prevent crossing solid voxels.
- Make projectiles and fluid follow carved terrain, and show excavated holes in the
  decorative ground.
- Apply reload checks to the selected siege battery so a command cannot fire a
  different battery.
- Raycast rotated debris chunks in their local coordinates and select the nearest
  hit.
- Keep replay playback from advancing the live world, draw approximate recorded
  debris bounds, and correct replay timeline, flight camera, and reset behavior.
- Charge campaign weapon costs only for accepted shots and check collateral
  against each settled debris voxel.
- Show remaining contract time and the C4 allocation in the contract HUD.
- Correct contract map assignments and briefings to describe the measured district
  goals. Grade after newly detached structures enter the physics simulation.
- Clear laser and charged-railgun state when changing weapon chassis or presets.
- Match player, projectile, fluid, and rubble contact to the square exterior slab.
- Prevent the HUD help button from receiving hidden cursor clicks during firing
  or through the workbench.
- Allow stationary, supported rubble to settle on upper floors so precision
  contracts can finish, and retain balanced slab contacts to prevent artificial
  impact kick and spin.
- Apply Solstice and Event Horizon damage to detached debris, preserve damaged
  voxel health through detachment, slicing, and fracture, and propagate volatile
  material reactions from their actual moving positions.
- Preserve mass accumulated by crushing when debris fractures into smaller pieces.
- Recheck debris support and motion after body collisions before allowing it to
  settle, so collision corrections cannot leave sleeping debris suspended.
- Respect Event Horizon impulse and blast-radius settings captured at launch.
- Refresh rubble support after carving or slicing debris, and prevent cosmetic
  electrical jitter from advancing the simulation's random sequence.
- Connect master, siege, structural-stress, and anti-stacking audio controls to
  their actual gain paths; label fixed synthesizer values as read-only.
- Fix narrow text ellipsis, wrapping of unbroken text, and overlapping workbench
  labels.

### Changed

- Reset the voxel grid with contiguous memory fills and reduce temporary music
  buffers during startup.
- Prepare host-native standard and size-optimized release archives, with a macOS
  application bundle, build information, dependency notices, and SHA-256 checksums.
- Seal the macOS app with hardened runtime, derive its minimum OS from the
  executable, and provide a Developer ID signing and notarization workflow.
- Include the project's MIT and Apache 2.0 license texts and remove placeholder
  author and repository metadata.

Versions 0.2.2 and 0.2.3 were private release candidates. See
[the release guide](docs/release.md) for qualification details.
