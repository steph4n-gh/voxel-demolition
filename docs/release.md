# Release guide

The macOS distribution uses one `release` build for Apple Silicon
(`aarch64-apple-darwin`). Each candidate requires a clean committed source tree,
fresh archive verification and native smoke testing before distribution. Player
instructions are in [the manual](manual.md).

## One build, chosen by measurement

The former size-focused `release-retro` profile used `opt-level = "z"`; the
retained `release` profile uses `opt-level = 3`. Both use LTO, one codegen unit,
stripped symbols and panic abort. Four measured pairs on the Apple M4 Pro used
balanced AB/BA order and a full-process warmup for each profile. Benchmark source
commit: `8eadeec67c28850480892553d147ffb170f62775`.

| Headless CPU workload | Release median | Former retro median | Paired median speedup |
| --- | ---: | ---: | ---: |
| 20 full-grid clears | 14.968 ms | 126.709 ms | 8.46x |
| Generated district and mesh rebuild | 61.934 ms | 161.985 ms | 2.62x |
| Warmed Corporate Drop updates | 7.971 s | 24.832 s | 3.10x |

The Corporate workload measured 968 frames after 120 warmup frames. Operation
counts, generated geometry and final state hashes matched between profiles.
These measurements cover simulation, generation and meshing on this host, not
native rendering FPS, GPU work or cold app startup.

The old retro build saved 557,888 raw executable bytes and 375,551 compressed
archive bytes. That saving does not justify the measured runtime cost, so the
game ships the faster release build only. Benchmark commands, source hashes, balanced
run logs and detailed counts remain in
`dist/validation/build-comparison/receipt.json`.

Qualification distinguishes headless simulation from native input/rendering
checks. Native coverage is macOS 27.0 on Apple M4 Pro; older macOS versions,
Intel Macs, Windows and Linux need their own checks. The packager reads the
Mach-O deployment target into the app's minimum OS metadata; that value does not
prove compatibility testing. Record commands, counts, source hashes, screenshots
and bounded gameplay results in each candidate's ignored `dist/validation/`
receipts. Simulations do not establish a complete human campaign playthrough.

## Build and qualify

Use the pinned `Cargo.lock` and an installed Rust toolchain with cached
dependencies. Python 3 is required for packaging. Work in an unsynchronized
checkout, using the default ignored `target_cli` directory so the footprint and
symbol checks can inspect the release executable. App assembly and signing use
the system temporary directory to avoid Finder/iCloud metadata.

```sh
export CARGO_BUILD_JOBS=4
cargo build --locked --offline --release
cargo test --locked --offline --all-targets --no-fail-fast -- --test-threads=1
cargo test --locked --offline --lib campaign::tests:: -- --include-ignored --nocapture --test-threads=1
cargo test --locked --offline --test corporate_precision -- --ignored --nocapture --test-threads=1
cargo test --locked --offline --doc
cargo clippy --locked --offline --all-targets -- -D warnings
```

Generate the PDF player manual before packaging with a Python environment that
has [ReportLab](https://docs.reportlab.com/install/open_source_installation/)
installed. For example, keep the typesetting dependency in a
temporary virtual environment:

```sh
python3 -m venv /tmp/voxel-demolition-manual
/tmp/voxel-demolition-manual/bin/python -m pip install reportlab==4.4.9
/tmp/voxel-demolition-manual/bin/python scripts/build-manual.py
```

The packager requires `output/pdf/Voxel-Demolition-Player-Manual.pdf` alongside
the tracked Markdown, generator and screenshot/provenance sources. ReportLab is
only needed to generate the PDF; packaging an existing verified PDF uses Python
3's standard library. Release provenance must come from a clean, committed
source tree.

## Package macOS

`./scripts/package-release.sh` creates a local candidate with an ad hoc app
seal. `--sign` instead
requires an installed Developer ID Application identity and signs with hardened
runtime and a secure timestamp. A signed-only candidate has no Apple
notarization ticket. Use `--notarize` for distribution:

```sh
MACOS_SIGNING_IDENTITY='<installed Developer ID Application name or SHA-1>' \
MACOS_NOTARY_PROFILE=voxel-demolition \
./scripts/package-release.sh --notarize
```

Use a valid installed Developer ID certificate/private key and existing
notarytool Keychain profile. Keep secrets outside the repository. List identities
with `security find-identity -v -p codesigning`. Apple's [Developer ID certificate guide](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/)
and [notarytool credential guide](https://developer.apple.com/documentation/technotes/tn3147-migrating-to-the-latest-notarization-tool)
describe setup; an App Store Connect **team** API key can be stored in a
Keychain profile without placing secrets in release commands.
Set `MACOS_NOTARY_KEYCHAIN` when the profile is in a file-based Keychain; the
packager forwards that path to credential validation, submission and log retrieval.

Assembly, signing and notarization occur in the system temporary directory.
Only disallowed FinderInfo/ResourceFork attributes are stripped from the newly
assembled app before signing. The script preserves each submission ZIP,
receipt, stderr, exit code and log under
`dist/validation/<archive-name>/attempt-*`, requires Apple acceptance without
reported issues, staples the app, checks strict signatures and Gatekeeper, then
creates the final ZIP and SHA-256 checksum file in `dist/`.

The single archive is named
`voxel-demolition-<version>-<host>[-signed|-notarized].zip`; the default local
candidate has no signing suffix. The archive contains the app, a standalone
executable, player PDF/Markdown, lightweight maintainer docs and license notices.

Distribute the stapled `.app` inside its ZIP. Apple cannot staple a ticket to a
ZIP or standalone executable; the separately supplied executable uses online
ticket lookup. The app includes player documentation and dependency notices.
`BUILD_INFO.txt`, `SOURCE_SHA256SUMS.txt`, `SIGNING.txt` and the Apple receipts
identify the actual build and distribution state. Follow Apple's
[notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow)
when producing new artifacts.

## GitHub Actions

[CI](https://github.com/steph4n-gh/voxel-demolition/blob/main/.github/workflows/ci.yml) runs on pushes to `main`, pull requests and
manual dispatch. It uses Rust 1.99.0 with Clippy, Python 3.12 and ReportLab 4.4.9,
fetches locked dependencies, builds the release executable, then runs the full suite, all 16 campaign tests
including ignored qualifications, the dedicated Corporate Drop fixture, doc
tests and Clippy. It generates the manual and uploads exactly the local candidate
ZIP, its checksum file and the PDF. This candidate has an ad hoc app seal.
CI uses read-only repository access, two Cargo build jobs and the default ignored
`target_cli` build directory on the disposable runner. Superseded CI runs are canceled.

GitHub currently documents `macos-15` as an Apple M1 ARM64 runner for both
[public and private repositories](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
Its speed and memory differ from the local M4 Pro qualification host. Keep timing
assertions unchanged until actual hosted measurements establish any problem;
hosted tests do not replace native input, rendering or manual review.

[macOS draft release](https://github.com/steph4n-gh/voxel-demolition/blob/main/.github/workflows/macos-release.yml) is manual-only
from `main`. Supply an existing `vX.Y.Z` tag whose commit is on `origin/main` and
whose version matches `Cargo.toml`. It checks out that exact commit, repeats
qualification and generates the PDF before importing credentials. Existing draft
or published releases are refused; the signed `v0.2.3` tag and its private release
assets remain unchanged. Workflow/script changes require a new qualified version
tag for a new packaged release.

Before using signing automation, configure the `macos-release` environment with a
deployment rule allowing only the `main` branch and required reviewers where
supported. Private environments need GitHub Pro, Team or Enterprise; required
reviewers for private repositories need Enterprise. See GitHub's
[environment availability](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments).
Install these **environment** secrets and variables through GitHub settings:

| Kind | Names | Value |
| --- | --- | --- |
| Secrets | `MACOS_CERTIFICATE_P12_BASE64`, `MACOS_CERTIFICATE_PASSWORD` | Base64 Developer ID certificate/private-key export and its password |
| Secret | `APPLE_API_PRIVATE_KEY_BASE64` | Base64 App Store Connect team API private key |
| Variables | `MACOS_SIGNING_IDENTITY`, `APPLE_API_KEY_ID`, `APPLE_API_ISSUER_ID` | Certificate name or SHA-1, API key ID and issuer ID |

No cloud signing credentials are installed by these source changes, and hosted
signing has not been verified. The workflow keeps decoded files and an unlocked
temporary Keychain on the isolated runner, authorizes `codesign` access,
uses the explicit Keychain for notarytool and removes credentials in
an always-run cleanup step. It creates only a GitHub **draft**, attaching the
notarized ZIP, checksum and PDF. Download and verify those exact assets, perform
native smoke testing and inspect the manual before publishing manually.
Failed runs retain only notarization receipts, logs, stderr, exit status and
Gatekeeper diagnostics as an Actions artifact; credential files and submission
ZIPs are excluded.

## Public distribution

Before publishing, audit the exact committed source/history and verify a fresh
clone. Keep the repository private until its source and assets are ready.
Changing visibility exposes source and Actions history/logs; GitHub documents the
[visibility effects](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility).

Create the version tag from the qualified commit and prepare a GitHub release
with the notarized archive, checksum file and player manual. Review the complete
draft before publication, then download the released assets into a clean
location, compare their hashes, extract the app and repeat launch/Gatekeeper
checks. GitHub's [release guide](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository)
describes draft releases and attaching assets. Public visibility, tag creation
and release publication are separate actions from local preparation.

The project is dual-licensed under MIT or Apache-2.0. Both project license files
and the pinned dependency notices accompany distributions. Review new dependency
licenses when changing `Cargo.lock`; packaging rejects unknown missing notices.
