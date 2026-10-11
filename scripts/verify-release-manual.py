#!/usr/bin/env python3
"""Check release manual parity/provenance and PDF links; optionally check live URLs."""
from __future__ import annotations

import argparse
import hashlib
import html
from html.parser import HTMLParser
import io
import json
import math
from pathlib import Path, PurePosixPath
import plistlib
import re
import subprocess
import tempfile
import tomllib
from urllib.parse import unquote, urldefrag, urlsplit
from urllib.error import HTTPError
from urllib.request import Request, urlopen
import zipfile

from pypdf import PdfReader

ROOT = Path(__file__).resolve().parents[1]
PDF_NAME = "Voxel-Demolition-Player-Manual.pdf"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def markdown_links(text):
    """Match the simple inline Markdown supported by build-manual.py."""
    headings, counts = set(), {}
    for title in re.findall(r"^#{1,6}\s+(.+)$", text, re.MULTILINE):
        slug = re.sub(r"[^\w\- ]", "", title.lower()).replace(" ", "-")
        count = counts.get(slug, 0)
        counts[slug] = count + 1
        headings.add(slug + (f"-{count}" if count else ""))
    body = False
    external, pdf_external, chapters = set(), set(), []
    for line in text.splitlines():
        if re.match(r"^##\s+1\.", line):
            body = True
        if body and line.startswith("## "):
            chapters.append(line[3:])
        for target in re.findall(r"\[[^\]]+\]\(([^)]+)\)", line):
            target = html.unescape(target)
            if target.startswith("#") and not body:
                require(unquote(target[1:]) in headings, f"Broken manual heading link: {target}")
            else:
                parts = urlsplit(target)
                require(parts.scheme in ("http", "https") and parts.hostname
                        and not parts.username and not parts.password,
                        f"Unsupported manual link (PDF requires an HTTP(S) URL): {target}")
                external.add(target)
                if body:
                    pdf_external.add(target)
    require(chapters, "Manual has no numbered chapters")
    return external, pdf_external, chapters


def pdf_links(data, expected_urls, chapters):
    reader = PdfReader(io.BytesIO(data), strict=True)
    require(not reader.is_encrypted and len(reader.pages) > 0, "Manual PDF is empty or encrypted")
    page_refs = {p.indirect_reference: i for i, p in enumerate(reader.pages)}
    named = reader.named_destinations
    internal_pages, actual_urls, outlines = set(), set(), {}

    def destination_page(dest):
        if isinstance(dest, str):
            require(dest in named, f"Missing named PDF destination: {dest}")
            page = reader.get_destination_page_number(named[dest])
        else:
            dest = dest.get_object()
            require(isinstance(dest, list) and len(dest) >= 2, "Malformed PDF destination")
            page = page_refs.get(dest[0])
        require(page is not None and 0 <= page < len(reader.pages), "PDF link points outside the manual")
        return page

    def inspect_outlines(items):
        for item in items:
            if isinstance(item, list):
                inspect_outlines(item)
            else:
                page = reader.get_destination_page_number(item)
                require(page is not None and 0 <= page < len(reader.pages), "Invalid PDF bookmark destination")
                outlines[str(item.title)] = page

    inspect_outlines(reader.outline)
    for page in reader.pages:
        box = page.mediabox
        for ref in page.get("/Annots", []):
            annot = ref.get_object()
            if annot.get("/Subtype") != "/Link":
                continue
            rect = [float(x) for x in annot.get("/Rect", [])]
            require(len(rect) == 4 and all(math.isfinite(x) for x in rect)
                    and float(box.left) - 1 <= rect[0] <= rect[2] <= float(box.right) + 1
                    and float(box.bottom) - 1 <= rect[1] <= rect[3] <= float(box.top) + 1,
                    "PDF link has an invalid or off-page rectangle")
            action = annot.get("/A", {})
            action = action.get_object() if hasattr(action, "get_object") else action
            if action.get("/S") == "/URI":
                actual_urls.add(str(action["/URI"]))
            else:
                dest = annot.get("/Dest") if "/Dest" in annot else action.get("/D")
                require(dest is not None and action.get("/S") in (None, "/GoTo"),
                        "Unsupported or missing PDF link action")
                internal_pages.add(destination_page(dest))
    require(actual_urls == expected_urls,
            f"PDF external links differ from Markdown: missing={sorted(expected_urls - actual_urls)}, "
            f"unexpected={sorted(actual_urls - expected_urls)}")
    for title in chapters:
        require(title in outlines, f"Missing PDF chapter bookmark: {title}")
        require(outlines[title] in internal_pages, f"Missing PDF contents link for chapter: {title}")
    return len(reader.pages), len(actual_urls)


class Anchors(HTMLParser):
    def __init__(self):
        super().__init__()
        self.values = set()

    def handle_starttag(self, tag, attrs):
        self.values.update(value for key, value in attrs if key in ("id", "name") and value)


def check_urls(urls):
    """GET follows redirects; unavailable URLs/fragments block publication."""
    documents = {}
    for url in sorted(urls):
        address, fragment = urldefrag(url)
        if address not in documents:
            # A real GET also handles sites that reject HEAD. Never disable TLS validation.
            try:
                response = urlopen(Request(address, headers={"User-Agent": "Voxel-Demolition-release-check"}), timeout=20)
            except HTTPError as error:
                error.close()
                raise
            with response:
                require(200 <= response.status < 300, f"Unavailable manual URL: {url}")
                require(urlsplit(response.url).scheme in ("http", "https"), "Unsupported URL redirect")
                text = response.read().decode(response.headers.get_content_charset() or "utf-8", errors="replace")
            anchors = Anchors()
            anchors.feed(text)
            documents[address] = anchors.values
        if fragment:
            fragment = unquote(fragment)
            require(fragment in documents[address] or "user-content-" + fragment in documents[address],
                    f"Missing live URL fragment: {url}")
        print(f"Link OK: {url}")


def verify(archive, pdf, checksum, root=ROOT, live=False, macos=False):
    root = Path(root)
    data = Path(pdf).read_bytes()
    digest = sha(data)
    archive = Path(archive)
    require(Path(checksum).read_text() == f"{sha(archive.read_bytes())}  {archive.name}\n",
            "ZIP checksum does not match the exact archive/filename")
    markdown = (root / "docs/manual.md").read_bytes()
    external, expected_urls, chapters = markdown_links(markdown.decode())
    pages, url_count = pdf_links(data, expected_urls, chapters)
    with zipfile.ZipFile(archive) as z:
        names = z.namelist()
        require(len(names) == len(set(names)), "Duplicate ZIP entries")
        for entry in z.infolist():
            path = PurePosixPath(entry.filename)
            require(not path.is_absolute() and ".." not in path.parts and "\\" not in entry.filename
                    and (entry.external_attr >> 16) & 0o170000 != 0o120000,
                    "Unsafe ZIP path or symlink")
        require(z.testzip() is None, "ZIP CRC failure")
        tops = {name.split("/")[0] for name in names if not name.startswith("__MACOSX/")}
        require(len(tops) == 1, "ZIP must have one release directory")
        prefix = tops.pop() + "/"
        resources = prefix + "Voxel Demolition.app/Contents/Resources/"
        copies = [prefix + "docs/"]
        if prefix + "Voxel Demolition.app/Contents/Info.plist" in names:
            copies.append(resources + "docs/")
        for path in copies:
            require(z.read(path + PDF_NAME) == data, f"Bundled/downloadable PDFs differ: {path}")
            require(z.read(path + "manual.md") == markdown, f"Bundled Markdown is stale: {path}")
        infos = [z.read(path + "BUILD_INFO.txt").decode()
                 for path in [prefix] + ([resources] if len(copies) == 2 else [])]
        for info in infos:
            require(f"Player manual PDF SHA-256: {digest}\n" in info
                    and f"Player manual PDF bytes: {len(data)}\n" in info, "Manual BUILD_INFO is stale")
        manifest = z.read(prefix + "SOURCE_SHA256SUMS.txt").decode()
        rows = {}
        for line in manifest.splitlines():
            match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
            require(match is not None, "Malformed source manifest")
            require(match[2] not in rows, "Duplicate source manifest path")
            rows[match[2]] = match[1]
        inputs = ["docs/manual.md", "scripts/build-manual.py"]
        inputs += [path for path in subprocess.check_output(
            ["git", "ls-files", "-z", "--", "docs/manual-assets"], cwd=root, text=True).split("\0") if path]
        for path in inputs:
            require(rows.get(path) == sha((root / path).read_bytes()), f"Stale manual input: {path}")
        require(all(f"Source manifest SHA-256: {sha(manifest.encode())}\n" in info for info in infos),
                "Source manifest hash differs")
        if macos:
            require(len(copies) == 2, "Missing macOS app/manual")
            receipt = json.loads(z.read(prefix + "NOTARIZATION_SUBMISSION.json"))
            log = json.loads(z.read(prefix + "NOTARIZATION_LOG.json"))
            require(receipt.get("status") == "Accepted" and log.get("status") == "Accepted"
                    and receipt.get("id") == log.get("jobId") and not log.get("issues"), "Notarization not accepted")
            with tempfile.TemporaryDirectory(prefix="voxel-release-check-") as directory:
                subprocess.run(["ditto", "-x", "-k", str(archive.resolve()), directory], check=True)
                app = Path(directory) / prefix / "Voxel Demolition.app"
                plist = plistlib.loads((app / "Contents/Info.plist").read_bytes())
                version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
                require(plist["CFBundleShortVersionString"] == version, "App version differs from source")
                signature = subprocess.run(["codesign", "-dvv", str(app)], check=True, capture_output=True, text=True)
                require("Authority=Developer ID Application:" in signature.stderr
                        and "runtime" in signature.stderr, "Developer ID/hardened runtime signature required")
                for args in [["codesign", "--verify", "--deep", "--strict", str(app)],
                             ["xcrun", "stapler", "validate", str(app)],
                             ["spctl", "--assess", "--type", "execute", str(app)]]:
                    subprocess.run(args, check=True)
    if live:
        check_urls(external)
    print(f"Manual verified: {pages} pages, {url_count} external targets, identical PDF copies; SHA256 {digest}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--pdf", type=Path, required=True)
    parser.add_argument("--checksum", type=Path, required=True)
    parser.add_argument("--check-links", action="store_true")
    parser.add_argument("--macos", action="store_true", help="Require locally verifiable Developer ID/notarization/stapling")
    args = parser.parse_args()
    try:
        verify(args.archive, args.pdf, args.checksum, live=args.check_links, macos=args.macos)
    except Exception as error:
        parser.exit(1, f"Release manual check failed: {error}\n")


if __name__ == "__main__":
    main()
