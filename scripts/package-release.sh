#!/usr/bin/env bash
# Build and package this host's release candidate. Requires Rust, Cargo and Python 3.
set -euo pipefail
cd "$(dirname "$0")/.."

sign=0
notarize=0
case "${1:-}" in
    "") ;;
    --sign) sign=1 ;;
    --notarize) sign=1; notarize=1 ;;
    --help|-h)
        echo "Usage: $0 [--sign|--notarize]"
        echo "Default: build a local candidate; the macOS app bundle receives an ad hoc seal."
        echo "--sign: requires MACOS_SIGNING_IDENTITY; builds a Developer ID-signed candidate without notarization."
        echo "--notarize: requires MACOS_SIGNING_IDENTITY and MACOS_NOTARY_PROFILE."
        echo "MACOS_NOTARY_KEYCHAIN optionally selects the Keychain containing that profile."
        exit 0
        ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
esac
if [[ $# -gt 1 ]]; then
    echo "Usage: $0 [--sign|--notarize]" >&2
    exit 1
fi

manual_pdf="output/pdf/Voxel-Demolition-Player-Manual.pdf"
if [[ ! -s "$manual_pdf" ]]; then
    echo "Missing player manual PDF: $manual_pdf. Generate it with scripts/build-manual.py in a ReportLab-enabled Python environment; see docs/release.md." >&2
    exit 1
fi

host_target="$(rustc -vV | sed -n 's/^host: //p')"
if [[ -z "$host_target" ]]; then
    echo "Could not determine the Rust host target." >&2
    exit 1
fi

if [[ "$sign" == 1 ]]; then
    if [[ "$host_target" != *apple-darwin ]]; then
        echo "Developer ID signing requires a macOS host." >&2
        exit 1
    fi
    : "${MACOS_SIGNING_IDENTITY:?Set MACOS_SIGNING_IDENTITY to an installed Developer ID Application identity.}"
    security find-identity -v -p codesigning | python3 -c '
import re, sys
selected = sys.argv[1]
identities = re.findall(r"\b([0-9A-Fa-f]{40}) \"([^\"]+)\"", sys.stdin.read())
if not any(name.startswith("Developer ID Application:") and selected in (digest, name) for digest, name in identities):
    sys.exit("The selected Developer ID Application identity is not valid in the current Keychain.")
' "$MACOS_SIGNING_IDENTITY"
fi
if [[ "$notarize" == 1 ]]; then
    : "${MACOS_NOTARY_PROFILE:?Set MACOS_NOTARY_PROFILE to an existing notarytool Keychain profile.}"
    xcrun --find notarytool > /dev/null
    # Validate the selected profile before building or uploading any artifact.
    notary_history_args=(history --keychain-profile "$MACOS_NOTARY_PROFILE" --output-format json)
    if [[ -n "${MACOS_NOTARY_KEYCHAIN:-}" ]]; then
        notary_history_args+=(--keychain "$MACOS_NOTARY_KEYCHAIN")
    fi
    xcrun notarytool "${notary_history_args[@]}" > /dev/null
fi

mkdir -p dist
metadata_file="$(mktemp "${TMPDIR:-/tmp}/voxel-release-metadata.XXXXXX")"
trap 'rm -f "$metadata_file"' EXIT
cargo metadata --locked --offline --filter-platform "$host_target" --format-version 1 > "$metadata_file"
cargo build --locked --offline --release --target "$host_target"

python3 - "$metadata_file" "$host_target" "$sign" "$notarize" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

metadata = json.loads(Path(sys.argv[1]).read_text())
host_target = sys.argv[2]
sign = sys.argv[3] == "1"
notarize = sys.argv[4] == "1"
artifact_suffix = "-notarized" if notarize else "-signed" if sign else ""
root = Path.cwd()
dist = root / "dist"
package = next(p for p in metadata["packages"] if Path(p["manifest_path"]) == root / "Cargo.toml")
version = package["version"]
target_dir = Path(metadata["target_directory"]) / host_target
suffix = ".exe" if "windows" in host_target else ""
exe_name = "voxel_demolition" + suffix
documents = ["README.md", "CONTRIBUTING.md", "CHANGELOG.md", "LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES.md", "docs/manual.md", "docs/development.md", "docs/release.md"]
for name in documents:
    if not (root / name).is_file():
        sys.exit(f"Missing release document: {name}")

manual_pdf = root / "output/pdf/Voxel-Demolition-Player-Manual.pdf"
manual_pdf_name = "docs/Voxel-Demolition-Player-Manual.pdf"
manual_pdf_hash = hashlib.sha256(manual_pdf.read_bytes()).hexdigest()
manual_assets = [root / name for name in subprocess.check_output(
    ["git", "ls-files", "-z", "--", "docs/manual-assets"], text=True,
).split("\0") if name]
player_documents = ["docs/manual.md", manual_pdf_name]
if "manual-assets/" in (root / "docs/manual.md").read_text():
    player_documents += [str(path.relative_to(root)) for path in manual_assets]

supplements = root / "scripts" / "third-party-licenses"
supplement_files = {
    ("dasp_frame", "0.11.0"): supplements / "dasp_frame-0.11.0-LICENSE-MIT",
    ("dasp_sample", "0.11.0"): supplements / "dasp_sample-0.11.0-LICENSE-MIT",
    ("malloc_buf", "0.0.6"): supplements / "MIT-TERMS.txt",
    ("quad-rand", "0.2.3"): supplements / "MIT-TERMS.txt",
    ("quad-snd", "0.2.8"): root / "LICENSE-APACHE",
}
dependencies = sorted((p for p in metadata["packages"] if p["id"] != package["id"]), key=lambda p: (p["name"], p["version"]))

def command_output(*args):
    result = subprocess.run(args, cwd=root, capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else "unavailable"

def make_archive(stage, archive_path):
    if archive_path.exists():
        archive_path.unlink()
    if host_target.endswith("apple-darwin"):
        # Preserve the macOS metadata used by signing and stapling.
        subprocess.run(["ditto", "-c", "-k", "--keepParent", str(stage), str(archive_path)], check=True)
    else:
        with zipfile.ZipFile(archive_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9, strict_timestamps=False) as archive:
            for path in sorted(stage.rglob("*")):
                if path.is_file():
                    archive.write(path, path.relative_to(stage.parent))

revision = command_output("git", "rev-parse", "HEAD")
git_status = command_output("git", "status", "--porcelain")
rust_version = command_output("rustc", "-vV")
cargo_version = command_output("cargo", "--version")
source_paths = [root / "Cargo.toml", root / "Cargo.lock", root / ".cargo/config.toml", root / "scripts/package-release.sh", root / "scripts/build-manual.py"]
source_paths += sorted((root / "src").rglob("*.rs")) + sorted((root / "tests").rglob("*.rs"))
source_paths += [root / name for name in documents]
source_paths += manual_assets
source_paths += sorted(path for path in supplements.rglob("*") if path.is_file())
source_manifest = "".join(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n" for path in source_paths)
source_manifest_hash = hashlib.sha256(source_manifest.encode()).hexdigest()

# Assemble and sign outside synchronized folders, which can reintroduce Finder metadata.
with tempfile.TemporaryDirectory(prefix="voxel-release-") as staging_directory:
    profile = "release"
    executable = target_dir / profile / exe_name
    if not executable.is_file():
        sys.exit(f"Missing build output: {executable}")
    archive_name = f"voxel-demolition-{version}-{host_target}" + artifact_suffix
    stage = Path(staging_directory) / archive_name
    stage.mkdir()
    shutil.copy2(executable, stage / exe_name)
    for name in documents:
        destination = stage / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / name, destination)
    shutil.copy2(manual_pdf, stage / manual_pdf_name)
    for name in player_documents:
        if name in documents or name == manual_pdf_name:
            continue
        destination = stage / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / name, destination)

    license_root = stage / "THIRD_PARTY_LICENSES"
    license_root.mkdir()
    dependency_lines = [
        "# Packaged dependency notices", "", f"Target: `{host_target}`", "",
        "Cargo's host-target dependency graph, including build dependencies.", "",
        "| Package | Version | Declared license | Authors from package manifest | Source |",
        "| --- | --- | --- | --- | --- |",
    ]
    for dependency in dependencies:
        name, dep_version = dependency["name"], dependency["version"]
        source_root = Path(dependency["manifest_path"]).parent
        dest = license_root / f"{name}-{dep_version}"
        dest.mkdir()
        shutil.copy2(source_root / "Cargo.toml", dest / "Cargo.toml")
        license_files = [
            f for f in source_root.rglob("*") if f.is_file()
            and (f.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE"))
                 or f.name.lower().endswith(".license"))
        ]
        if license_files:
            for license_file in license_files:
                destination = dest / license_file.relative_to(source_root)
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(license_file, destination)
        else:
            supplement = supplement_files.get((name, dep_version))
            if supplement is None or not supplement.is_file():
                sys.exit(f"Review missing license notice for {name} {dep_version} before packaging.")
            shutil.copy2(supplement, dest / supplement.name)
            (dest / "NOTICE-SOURCE.txt").write_text(
                "This crate archive supplies no standalone license text.\n"
                "See the packaged THIRD_PARTY_NOTICES.md for the supplement's provenance.\n"
                "The original Cargo.toml preserves upstream license and author declarations.\n"
            )
        if not dependency["license"]:
            sys.exit(f"Review missing license declaration for {name} {dep_version} before packaging.")
        if (name, dep_version) == ("miniquad", "0.4.11"):
            shutil.copy2(supplements / "miniquad-0.4.11-Makepad-LICENSE-MIT", dest / "Makepad-LICENSE-MIT")
        authors = "; ".join(dependency["authors"]).replace("|", "\\|")
        url = dependency.get("repository") or dependency.get("homepage") or f"https://crates.io/crates/{name}/{dep_version}"
        dependency_lines.append(f"| {name} | {dep_version} | {dependency['license']} | {authors} | {url} |")

    shutil.copy2(supplements / "ProggyClean-LICENSE-MIT", license_root / "ProggyClean-LICENSE-MIT")
    (license_root / "DEPENDENCIES.md").write_text("\n".join(dependency_lines) + "\n")
    build_info = (
        f"Voxel Demolition {version}\nProfile: {profile}\n"
        f"Rust host target: {host_target}\nGit revision: {revision}\n"
        f"Working tree changes present: {'yes' if git_status else 'no'}\n"
        f"Built executable bytes: {executable.stat().st_size}\n"
        f"Built executable SHA-256: {hashlib.sha256(executable.read_bytes()).hexdigest()}\n"
        f"Player manual PDF bytes: {manual_pdf.stat().st_size}\n"
        f"Player manual PDF SHA-256: {manual_pdf_hash}\n"
        f"Cargo: {cargo_version}\n\n{rust_version}\n"
        "\nBuilt with --locked --offline. This script does not run release qualification checks.\n"
    )
    (stage / "SOURCE_SHA256SUMS.txt").write_text(source_manifest)
    build_info += f"\nSource manifest SHA-256: {source_manifest_hash}\n"
    (stage / "BUILD_INFO.txt").write_text(build_info)

    if host_target.endswith("apple-darwin"):
        build_version = subprocess.check_output(["xcrun", "vtool", "-show-build", str(executable)], text=True)
        minimum_versions = re.findall(r"^\s*minos\s+(\d+(?:\.\d+)*)\s*$", build_version, re.MULTILINE)
        if not minimum_versions:
            sys.exit("Could not determine the executable's macOS deployment target.")
        minimum_os = max(minimum_versions, key=lambda value: tuple(map(int, value.split("."))))
        build_info += f"\nMach-O minimum macOS version: {minimum_os}\n"
        build_info += "Distribution signing: " + ("Developer ID Application\n" if sign else "ad hoc local app bundle\n")
        build_info += "Notarization: " + ("see NOTARIZATION_SUBMISSION.json and NOTARIZATION_LOG.json\n" if notarize else "not requested\n")
        (stage / "BUILD_INFO.txt").write_text(build_info)
        app = stage / "Voxel Demolition.app" / "Contents"
        (app / "MacOS").mkdir(parents=True)
        resources = app / "Resources"
        resources.mkdir()
        shutil.copy2(executable, app / "MacOS" / "voxel_demolition")
        for name in ["LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES.md", "BUILD_INFO.txt"] + player_documents:
            destination = resources / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(stage / name, destination)
        shutil.copytree(license_root, resources / "THIRD_PARTY_LICENSES")
        with (app / "Info.plist").open("wb") as plist_file:
            plistlib.dump({
                "CFBundleExecutable": "voxel_demolition",
                "CFBundleIdentifier": "local.voxel-demolition",
                "CFBundleName": "Voxel Demolition",
                "CFBundleDisplayName": "Voxel Demolition",
                "CFBundlePackageType": "APPL",
                "CFBundleShortVersionString": version,
                "CFBundleVersion": version,
                "LSMinimumSystemVersion": minimum_os,
                "NSHighResolutionCapable": True,
            }, plist_file)
        signing_identity = os.environ["MACOS_SIGNING_IDENTITY"] if sign else "-"
        timestamp = "--timestamp" if sign else "--timestamp=none"
        if sign:
            subprocess.run(["codesign", "--force", "--sign", signing_identity, "--options", "runtime", timestamp, str(stage / exe_name)], check=True)
            subprocess.run(["codesign", "--verify", "--strict", "--verbose=2", str(stage / exe_name)], check=True)
        # Finder/iCloud may attach attributes that codesign forbids. Strip only those
        # from this newly assembled bundle before signing; preserve other metadata.
        for attribute in ["com.apple.FinderInfo", "com.apple.ResourceFork"]:
            subprocess.run(["xattr", "-dr", attribute, str(app.parent)], check=True)
        # The app contains one executable and system frameworks only; no broad entitlements are needed.
        subprocess.run(["codesign", "--force", "--sign", signing_identity, "--options", "runtime", timestamp, str(app.parent)], check=True)
        subprocess.run(["codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app.parent)], check=True)
        signature = subprocess.run(["codesign", "-dvv", str(app.parent)], capture_output=True, text=True, check=True)
        (stage / "SIGNING.txt").write_text(signature.stdout + signature.stderr)

        if notarize:
            submission_archive = Path(staging_directory) / (archive_name + "-submission.zip")
            make_archive(stage, submission_archive)
            notary_evidence_root = dist / "validation" / archive_name
            notary_evidence_root.mkdir(parents=True, exist_ok=True)
            notary_evidence = Path(tempfile.mkdtemp(prefix="attempt-", dir=notary_evidence_root))
            print(f"Notarization evidence: {notary_evidence.relative_to(root)}", flush=True)
            shutil.copy2(submission_archive, notary_evidence / submission_archive.name)
            profile = os.environ["MACOS_NOTARY_PROFILE"]
            notary_keychain_args = ["--keychain", os.environ["MACOS_NOTARY_KEYCHAIN"]] if os.environ.get("MACOS_NOTARY_KEYCHAIN") else []
            result = subprocess.run([
                "xcrun", "notarytool", "submit", str(submission_archive),
                "--keychain-profile", profile, "--wait", "--output-format", "json",
            ] + notary_keychain_args, capture_output=True, text=True)
            (notary_evidence / "NOTARIZATION_SUBMISSION.json").write_text(result.stdout)
            (notary_evidence / "NOTARIZATION_STDERR.txt").write_text(result.stderr)
            (notary_evidence / "NOTARIZATION_EXIT_CODE.txt").write_text(f"{result.returncode}\n")
            (stage / "NOTARIZATION_SUBMISSION.json").write_text(result.stdout)
            if result.stderr:
                print(result.stderr, file=sys.stderr, end="")
            if result.returncode:
                sys.exit(f"Notarization did not complete successfully; retained submission and output in {notary_evidence}.")
            receipt = json.loads(result.stdout)
            subprocess.run([
                "xcrun", "notarytool", "log", receipt["id"],
                "--keychain-profile", profile, str(notary_evidence / "NOTARIZATION_LOG.json"),
            ] + notary_keychain_args, check=True)
            shutil.copy2(notary_evidence / "NOTARIZATION_LOG.json", stage / "NOTARIZATION_LOG.json")
            log = json.loads((stage / "NOTARIZATION_LOG.json").read_text())
            if receipt.get("status") != "Accepted" or log.get("issues"):
                sys.exit(f"Notarization was rejected or reports issues; review submission and log in {notary_evidence}.")
            subprocess.run(["xcrun", "stapler", "staple", str(app.parent)], check=True)
            subprocess.run(["xcrun", "stapler", "validate", str(app.parent)], check=True)
            subprocess.run(["codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app.parent)], check=True)
            assessment = subprocess.run(["spctl", "--assess", "--type", "execute", "--verbose=2", str(app.parent)], capture_output=True, text=True)
            (notary_evidence / "GATEKEEPER.txt").write_text(assessment.stdout + assessment.stderr)
            (stage / "GATEKEEPER.txt").write_text(assessment.stdout + assessment.stderr)
            assessment.check_returncode()
            submission_archive.unlink()

        # These hashes describe the distributed copies after signing, separately from build outputs.
        build_info += f"\nPackaged standalone executable bytes: {(stage / exe_name).stat().st_size}\n"
        build_info += f"Packaged standalone executable SHA-256: {hashlib.sha256((stage / exe_name).read_bytes()).hexdigest()}\n"
        build_info += f"Bundle executable bytes: {(app / 'MacOS' / 'voxel_demolition').stat().st_size}\n"
        build_info += f"Bundle executable SHA-256: {hashlib.sha256((app / 'MacOS' / 'voxel_demolition').read_bytes()).hexdigest()}\n"
        (stage / "BUILD_INFO.txt").write_text(build_info)

    archive_path = Path(staging_directory) / (archive_name + ".zip")
    make_archive(stage, archive_path)
    archive_hash = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    print(f"Prepared {archive_path.name} ({archive_path.stat().st_size:,} bytes; executable {executable.stat().st_size:,} bytes)")

    # Publish only after assembly, signing and verification have completed.
    final_archive_name = archive_path.name
    shutil.move(str(archive_path), str(dist / final_archive_name))

checksum_path = dist / (f"voxel-demolition-{version}-{host_target}" + artifact_suffix + "-SHA256SUMS.txt")
checksum_path.write_text(f"{archive_hash}  {final_archive_name}\n")
print(checksum_path.relative_to(root))
PY
