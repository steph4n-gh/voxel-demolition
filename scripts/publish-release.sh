#!/usr/bin/env bash
# Upload a verified local release as a draft; recheck that draft before publication.
set -euo pipefail
cd "$(dirname "$0")/.."
case "${1:-}" in
    ""|--publish) ;;
    --help|-h)
        echo "Usage: $0 [--publish]"
        echo "Default: create a new draft from the locally notarized dist ZIP, checksum and PDF."
        echo "--publish: download and recheck an existing draft before making it public."
        echo "Requires clean source at the version tag, Python manual dependencies, gh and macOS."
        exit 0 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
esac
[[ $# -le 1 ]] || { echo "Usage: $0 [--publish]" >&2; exit 1; }
[[ "${GITHUB_ACTIONS:-}" != true ]] || { echo "Releases must be prepared and published locally." >&2; exit 1; }
"${PYTHON:-python3}" - "${1:-}" <<'PY'
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import zipfile

root = Path.cwd()
publish = sys.argv[1] == "--publish"

def output(*args):
    return subprocess.check_output(args, text=True).strip()

def require(condition, message):
    if not condition:
        sys.exit(message)

require(sys.platform == "darwin", "Local macOS release verification is required.")
require(not output("git", "status", "--porcelain"), "Commit all source changes before release preparation.")
version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
tag = "v" + version
revision = output("git", "rev-parse", "HEAD")
require(output("git", "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}") == revision,
        "Current source must match the qualified version tag; old release tags are not moved.")
stem = f"voxel-demolition-{version}-aarch64-apple-darwin-notarized"
names = [stem + ".zip", stem + "-SHA256SUMS.txt", "Voxel-Demolition-Player-Manual.pdf"]

def check(directory):
    subprocess.run([sys.executable, "scripts/verify-release-manual.py",
                    "--archive", str(directory / names[0]), "--checksum", str(directory / names[1]),
                    "--pdf", str(directory / names[2]), "--check-links", "--macos"], check=True)
    with zipfile.ZipFile(directory / names[0]) as archive:
        info = archive.read(stem + "/BUILD_INFO.txt").decode()
        require(f"Git revision: {revision}\n" in info and "Working tree changes present: no\n" in info,
                "The packaged source revision must match the clean qualified tag.")

local = root / "dist"
# All content/link/signature checks precede the first GitHub mutation.
check(local)
repo = output("gh", "repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner")
remote_revision = output("gh", "api", f"repos/{repo}/commits/{tag}", "--jq", ".sha")
require(remote_revision == revision, "The GitHub tag differs from the local qualified tag.")
pages = json.loads(output("gh", "api", "--paginate", "--slurp", f"repos/{repo}/releases"))
existing = [release for page in pages for release in page if release["tag_name"] == tag]

with tempfile.TemporaryDirectory(prefix="voxel-publish-") as directory:
    directory = Path(directory)
    if not publish:
        require(not existing, "A draft or published release already exists; refusing replacement.")
        notes = directory / "release-notes.md"
        notes.write_text(f"Apple Silicon macOS release from `{revision}`.\n\n"
                         "Locally signed with Developer ID, notarized by Apple and stapled. "
                         "Bundled and downloadable player manuals match; checksums and manual links passed.\n\n"
                         "Before publication, review this draft and smoke-test the exact extracted app. "
                         "See docs/release.md for qualification and platform limits.\n")
        subprocess.run(["gh", "release", "create", tag, *[str(local / name) for name in names],
                        "--repo", repo, "--verify-tag", "--draft", "--title", f"Voxel Demolition {version}",
                        "--notes-file", str(notes)], check=True)
        print("Draft created. Review and smoke-test its assets, then run scripts/publish-release.sh --publish.")
    else:
        require(len(existing) == 1 and existing[0]["draft"], "An existing unpublished draft is required.")
        require({asset["name"] for asset in existing[0]["assets"]} == set(names)
                and len(existing[0]["assets"]) == len(names), "Draft must contain exactly the verified triplet.")
        subprocess.run(["gh", "release", "download", tag, "--repo", repo, "--dir", str(directory),
                        *[arg for name in names for arg in ("--pattern", name)]], check=True)
        for name in names:
            require((directory / name).read_bytes() == (local / name).read_bytes(),
                    f"Draft asset differs from the locally verified release: {name}")
        check(directory)
        # Refuse a changed draft/tag between download and publication.
        latest = json.loads(output("gh", "api", f"repos/{repo}/releases/{existing[0]['id']}"))
        def asset_identity(release):
            return sorted((a["id"], a["name"], a["size"], a.get("digest"), a["state"]) for a in release["assets"])
        require(latest["draft"] and latest["tag_name"] == tag and asset_identity(latest) == asset_identity(existing[0]),
                "Draft changed during verification; retry.")
        require(output("gh", "api", f"repos/{repo}/commits/{tag}", "--jq", ".sha") == revision, "Tag changed during verification.")
        subprocess.run(["gh", "release", "edit", tag, "--repo", repo, "--draft=false"], check=True)
        print(f"Published https://github.com/{repo}/releases/tag/{tag}")
PY
