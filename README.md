# Voxel Demolition: Oblivion Protocol

A first-person demolition sandbox built with Rust and Macroquad. Break apart
procedural buildings with ten weapons, plant C4, deploy four siege platforms,
and try eight demolition contracts with destruction, budget, time, and precision
limits. Detached structures become falling rubble; excavated terrain and volatile
materials can change how a demolition unfolds.

Geometry, sound effects, and music are generated in code. Macroquad supplies the
windowing, rendering, audio, and embedded font; no separate game asset pack is
required.

## Play

For a macOS package, extract the ZIP, move **Voxel Demolition.app** to
Applications, and open it. Apple Silicon
macOS is the qualified platform for this release candidate; other platforms need
their own build and gameplay checks.

Move with `W/A/S/D` and the mouse, choose a weapon with `1` through `0`, and fire
with the left mouse button. `F1` opens help, `Tab` opens the workbench, and `Esc`
pauses or resumes.

The [player manual](docs/manual.md) covers every control, weapon, level, contract,
workbench setting, and replay control. The workbench, help, free cursor, and
replay pause the live world and contract timer. Settings and campaign progress
last for the current session.

## Build

Install a stable Rust toolchain and use the pinned dependencies. On macOS, also
install Xcode's command-line tools. See the [developer notes](docs/development.md)
for Linux build prerequisites.

```sh
cargo build --locked --release
cargo run --locked --release
```

Add `--offline` when Cargo's dependency cache is populated. Build output goes to
`target_cli/`; the standalone executable is
`target_cli/release/voxel_demolition` (`.exe` on Windows). Set `CARGO_TARGET_DIR`
to another directory if needed.

## Development and releases

- [CI checks](https://github.com/steph4n-gh/voxel-demolition/blob/main/.github/workflows/ci.yml): tests, Clippy, the manual, and a local
  macOS candidate on GitHub-hosted Apple Silicon runners.
- [Contributing](CONTRIBUTING.md): focused fixes, checks, and contribution terms.
- [Developer notes](docs/development.md): source layout, simulation invariants,
  and test guidance.
- [Release guide](docs/release.md): verification, packaging, macOS signing and
  notarization, and qualification limits.
- [Changelog](CHANGELOG.md): changes by version.

## License

Licensed under **MIT OR Apache-2.0**; see [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE). Dependency and embedded-font attributions are
listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and included with release
packages.
