# Contributing

Report bugs in [GitHub Issues](https://github.com/steph4n-gh/voxel-demolition/issues).

Keep changes focused on a reproduced problem. Prefer simplifying or reusing the
existing code over adding abstractions or dependencies. For a gameplay bug,
record the level or contract, weapon and workbench settings, reproduction steps,
and expected and observed behavior. Add a focused regression test when it can
demonstrate the fix.

Before submitting a change, run the relevant tests and the normal checks:

```sh
cargo build --locked --release
cargo test --locked --all-targets -- --test-threads=1
cargo test --locked --doc
cargo clippy --locked --all-targets -- -D warnings
```

The release build is required by the executable footprint and symbol checks.
Use the repository's default `target_cli` build directory for the full suite.
Add `--offline` when dependencies are cached. The single test thread isolates
allocator tracking and timing checks. Gameplay and UI changes also need native
play checks; simulation tests alone do not verify the packaged app. See
[developer notes](docs/development.md) for targeted and longer-running checks.

Describe what changed, why, how it was checked, and any remaining limitations.
Preserve mission goals, budgets, timers, and precision requirements. Keep build
products, local validation evidence, credentials, and signing material out of
commits. Follow the [release guide](docs/release.md) when packaging a build.

Contributions use the project's **MIT OR Apache-2.0** license. Preserve original
notices for third-party code and assets; review
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) when changing dependencies.
