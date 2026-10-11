# Release guide

The macOS distribution uses one `release` build for Apple Silicon
(`aarch64-apple-darwin`). Each candidate requires a clean committed source tree,
fresh archive verification and native smoke testing before distribution. Player
instructions are in [the manual](manual.md).

## One build, chosen by measurement

The former size-focused `release-retro` profile used `opt-level = "z"`; the
retained `release` profile uses `opt-level = 3`. Both use LTO, one codegen unit,
stripped symbols and panic abort. Four measured pairs on the Apple M4 Pro used
balanced AB/BA order and a full-process warmup for each profile. These are
historical local qualification measurements from source commit
`8eadeec67c28850480892553d147ffb170f62775`, retained in private history rather than
the public source snapshot.

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
game ships the faster release build only. Benchmark commands, source hashes,
balanced run logs and detailed counts remain in the local, ignored
`dist/validation/build-comparison/receipt.json`; that receipt and the exact paired
runner are not part of the public repository. Public
[performance fixtures](../tests/performance_benchmarks.rs) and the
[Corporate Drop fixture](../tests/corporate_precision.rs) support further
measurements, but do not reproduce the historical paired experiment by themselves.

Qualification distinguishes headless simulation from native input/rendering
checks. Native coverage is macOS 27.0 on Apple M4 Pro; older macOS versions,
Intel Macs, Windows and Linux need their own checks. The packager reads the
Mach-O deployment target into the app's minimum OS metadata; that value does not
prove compatibility testing. Record commands, counts, source hashes, screenshots
and bounded gameplay results in each candidate's ignored `dist/validation/`
receipts. Simulations do not establish a complete human campaign playthrough.

## Build and qualify

Use the pinned `Cargo.lock` and an installed Rust toolchain with cached
dependencies. Python 3.11 or newer is required for packaging. Work in an
unsynchronized checkout, using the default ignored `target_cli` directory so the footprint and
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

Packaging requires [ReportLab](https://docs.reportlab.com/install/open_source_installation/)
4.4.9 for typesetting and pypdf 6.19.0 for verification. Keep these dependencies
in a temporary virtual environment and select it with `PYTHON`:

```sh
python3 -m venv /tmp/voxel-demolition-manual
/tmp/voxel-demolition-manual/bin/python -m pip install reportlab==4.4.9 pypdf==6.19.0
export PYTHON=/tmp/voxel-demolition-manual/bin/python
```

The packager regenerates the player PDF from the tracked Markdown, generator,
screenshots and provenance sources every time. It stages one PDF and copies its
exact bytes into the app, the archive's `docs/` directory and
`dist/Voxel-Demolition-Player-Manual.pdf` for the separate download. The PDF and
Markdown hashes are recorded in the package provenance. Release provenance must
come from a clean, committed source tree; an old PDF in `output/pdf/` cannot
satisfy packaging.

## Package macOS

`./scripts/package-release.sh` creates a local candidate with an ad hoc app
seal. `--sign` instead
requires an installed Developer ID Application identity and signs with hardened
runtime and a secure timestamp. A signed-only candidate has no Apple
notarization ticket. Use `--notarize` for distribution:

```sh
MACOS_SIGNING_IDENTITY='<installed Developer ID Application name or SHA-1>' \
MACOS_NOTARY_PROFILE=voxel-demolition \
PYTHON=/tmp/voxel-demolition-manual/bin/python \
./scripts/package-release.sh --notarize
```

Use a valid installed Developer ID certificate/private key and existing
notarytool Keychain profile on the local Mac. Keep signing and notarization
credentials in its Keychain, outside the repository and GitHub Actions. List
identities with `security find-identity -v -p codesigning`. Apple's [Developer ID certificate guide](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/)
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

Packaging then verifies the ZIP checksum, source provenance, bundled Markdown
and PDF hashes, equality of all PDF copies, PDF structure and internal links,
and Markdown link targets. The exported manual and archive are one release
candidate; do not substitute a separately generated PDF after packaging.

Before uploading, check the live external URLs and HTML fragment targets too:

```sh
version="$("$PYTHON" -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')"
"$PYTHON" scripts/verify-release-manual.py \
  --archive "dist/voxel-demolition-$version-aarch64-apple-darwin-notarized.zip" \
  --pdf dist/Voxel-Demolition-Player-Manual.pdf \
  --checksum "dist/voxel-demolition-$version-aarch64-apple-darwin-notarized-SHA256SUMS.txt" \
  --check-links
```

Use the filenames produced for the candidate's version and host. Failed links,
missing fragments or mismatched manuals stop verification. A successful link
check establishes availability at that time; it cannot keep external pages from
changing later.

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
manual dispatch. It uses Rust 1.99.0 with Clippy, Python 3.12, ReportLab 4.4.9
and pypdf 6.19.0. It fetches locked dependencies, builds the release executable,
then runs the full suite, all 16 campaign tests
including ignored qualifications, the dedicated Corporate Drop fixture, doc
tests and Clippy. It runs the release manual verifier tests, packages a freshly
generated manual, and verifies PDF parity, source provenance and live links
before uploading the candidate ZIP, its checksum file and the exact exported
PDF. This candidate has an ad hoc app seal.
CI uses read-only repository access, two Cargo build jobs and the default ignored
`target_cli` build directory on the disposable runner. Superseded CI runs are canceled.

GitHub currently documents `macos-15` as an Apple M1 ARM64 runner for both
[public and private repositories](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
Its speed and memory differ from the local M4 Pro qualification host. Keep timing
assertions unchanged until actual hosted measurements establish any problem;
hosted tests do not replace native input, rendering or manual review.

Signing, notarization and GitHub release publication run locally through
`scripts/package-release.sh` and `scripts/publish-release.sh`. There is no hosted
signing workflow or credential upload step.

## Public distribution

Audit the exact committed source/history and verify a fresh clone for each
release. Create the matching `vX.Y.Z` tag from the qualified commit and check it
out with a clean tree, then package it locally with `--notarize`.
From that same checkout, use the authenticated GitHub CLI and local publication
wrapper:

```sh
PYTHON=/tmp/voxel-demolition-manual/bin/python ./scripts/publish-release.sh
```

The wrapper infers the `vX.Y.Z` tag from `Cargo.toml`, requires the clean checkout
to match that tag, verifies the candidate and live links before any GitHub
mutation, and creates a draft with the notarized ZIP, its checksum file and the
exact exported PDF. It checks that the tag and
candidate belong to the qualified source. Existing drafts and published releases
are refused when preparing a new draft; this workflow does not replace or move a
public version tag.

Review the complete draft, inspect the PDF layout, and perform native smoke
testing on the downloaded app. Then publish through the same wrapper:

```sh
PYTHON=/tmp/voxel-demolition-manual/bin/python ./scripts/publish-release.sh --publish
```

The publishing pass downloads the existing draft assets into a fresh location,
requires the same bytes as the local ZIP, checksum and PDF, and checks their
parity and live links. It also verifies the downloaded app signature, notarization
ticket and Gatekeeper acceptance before making the release public. Download
the published assets again, compare hashes, extract the app and repeat
launch/Gatekeeper checks. GitHub's [release guide](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository)
describes draft releases and attaching assets. Manual and native review remain
part of release qualification; link and PDF checks do not exercise gameplay or
judge page layout.

The project is dual-licensed under MIT or Apache-2.0. Both project license files
and the pinned dependency notices accompany distributions. Review new dependency
licenses when changing `Cargo.lock`; packaging rejects unknown missing notices.
