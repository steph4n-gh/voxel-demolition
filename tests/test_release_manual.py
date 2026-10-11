"""Regression checks for stale manuals and broken links blocking release uploads."""
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import io
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch
import zipfile

from pypdf import PdfReader, PdfWriter
from pypdf.generic import ArrayObject, NameObject, NumberObject
from reportlab.pdfgen.canvas import Canvas

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("release_manual", ROOT / "scripts/verify-release-manual.py")
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class HTTPFixture(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/redirect":
            self.send_response(302)
            self.send_header("Location", "/ok")
            self.end_headers()
        elif self.path == "/ok":
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(b'<h1 id="section">Guide</h1>')
        else:
            self.send_error(404)

    def log_message(self, *args):
        pass


class ReleaseManualTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), HTTPFixture)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.address = f"http://127.0.0.1:{cls.server.server_port}"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="voxel-manual-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "docs/manual-assets").mkdir(parents=True)
        (self.root / "scripts").mkdir()
        (self.root / "dist").mkdir()
        (self.root / ".gitignore").write_text("dist/\n")
        (self.root / "Cargo.toml").write_text('[package]\nversion="1.0.0"\n')
        for script in ["build-manual.py", "verify-release-manual.py", "publish-release.sh"]:
            shutil.copy2(ROOT / "scripts" / script, self.root / "scripts" / script)
        (self.root / "docs/manual-assets/README.md").write_text("Fixture image provenance\n")
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        subprocess.run(["git", "add", "docs/manual-assets"], cwd=self.root, check=True)
        self.stem = "voxel-demolition-1.0.0-aarch64-apple-darwin-notarized"
        self.archive = self.root / "dist" / (self.stem + ".zip")
        self.checksum = self.root / "dist" / (self.stem + "-SHA256SUMS.txt")
        self.pdf = self.root / "dist" / checker.PDF_NAME
        self.bundle()

    def bundle(self, url=None, revision="fixture"):
        url = url or self.address + "/ok#section"
        (self.root / "docs/manual.md").write_text(
            f"# Manual\n[Contents](#1-start)\n\n## 1. Start\nRead the [guide]({url}).\n")
        output = io.BytesIO()
        canvas = Canvas(output, pagesize=(300, 400))
        canvas.drawString(20, 350, "Contents")
        canvas.linkRect("", "start", (20, 320, 100, 335), relative=0)
        canvas.showPage()
        canvas.bookmarkPage("start")
        canvas.addOutlineEntry("1. Start", "start")
        canvas.drawString(20, 350, "1. Start")
        # A wrapped URL produces more than one annotation for the same target.
        for y in [320, 300]:
            canvas.linkURL(url, (20, y, 180, y + 12), relative=0)
        canvas.save()
        self.pdf.write_bytes(output.getvalue())
        inputs = ["docs/manual.md", "scripts/build-manual.py", "docs/manual-assets/README.md"]
        manifest = "".join(f"{hashlib.sha256((self.root / name).read_bytes()).hexdigest()}  {name}\n" for name in inputs)
        info = (f"Player manual PDF SHA-256: {hashlib.sha256(self.pdf.read_bytes()).hexdigest()}\n"
                f"Player manual PDF bytes: {self.pdf.stat().st_size}\n"
                f"Source manifest SHA-256: {hashlib.sha256(manifest.encode()).hexdigest()}\n"
                f"Git revision: {revision}\nWorking tree changes present: no\n")
        self.members = {
            "SOURCE_SHA256SUMS.txt": manifest.encode(), "BUILD_INFO.txt": info.encode(),
            "docs/" + checker.PDF_NAME: self.pdf.read_bytes(),
            "docs/manual.md": (self.root / "docs/manual.md").read_bytes(),
            "Voxel Demolition.app/Contents/Info.plist": plistlib.dumps({"CFBundleShortVersionString": "1.0.0"}),
            "NOTARIZATION_SUBMISSION.json": json.dumps({"id": "fixture", "status": "Accepted"}).encode(),
            "NOTARIZATION_LOG.json": json.dumps({"jobId": "fixture", "status": "Accepted", "issues": None}).encode(),
        }
        for name in ["BUILD_INFO.txt", "docs/manual.md", "docs/" + checker.PDF_NAME]:
            self.members["Voxel Demolition.app/Contents/Resources/" + name] = self.members[name]
        self.write_zip()

    def write_zip(self):
        with zipfile.ZipFile(self.archive, "w", zipfile.ZIP_DEFLATED) as archive:
            for name, data in self.members.items():
                archive.writestr(self.stem + "/" + name, data)
        self.update_checksum()

    def update_checksum(self):
        self.checksum.write_text(f"{hashlib.sha256(self.archive.read_bytes()).hexdigest()}  {self.archive.name}\n")

    def verify(self, **kwargs):
        checker.verify(self.archive, self.pdf, self.checksum, root=self.root, **kwargs)

    def test_valid_copies_destinations_and_wrapped_urls(self):
        (self.root / "docs/manual-assets/.DS_Store").write_bytes(b"untracked metadata")
        self.verify(live=True)

    def test_each_bundled_pdf_must_match(self):
        for name in ["docs/" + checker.PDF_NAME, "Voxel Demolition.app/Contents/Resources/docs/" + checker.PDF_NAME]:
            with self.subTest(name=name):
                self.bundle()
                self.members[name] += b"stale PDF"
                self.write_zip()
                with self.assertRaisesRegex(ValueError, "PDFs differ"):
                    self.verify()

    def test_replaced_downloadable_pdf_rejected(self):
        self.pdf.write_bytes(self.pdf.read_bytes() + b"swapped download")
        with self.assertRaisesRegex(ValueError, "PDFs differ"):
            self.verify()

    def test_changed_markdown_rejected(self):
        with (self.root / "docs/manual.md").open("a") as stream:
            stream.write("Changed gameplay instructions.\n")
        with self.assertRaisesRegex(ValueError, "Markdown is stale"):
            self.verify()

    def test_changed_generator_or_asset_rejected(self):
        for name in ["scripts/build-manual.py", "docs/manual-assets/README.md"]:
            with self.subTest(name=name):
                self.bundle()
                path = self.root / name
                before = path.read_bytes()
                path.write_bytes(before + b"\nchanged input")
                with self.assertRaisesRegex(ValueError, "Stale manual input"):
                    self.verify()
                path.write_bytes(before)

    def test_both_build_info_manifest_hashes_checked(self):
        for name in ["BUILD_INFO.txt", "Voxel Demolition.app/Contents/Resources/BUILD_INFO.txt"]:
            with self.subTest(name=name):
                self.bundle()
                self.members[name] = self.members[name].replace(b"Source manifest SHA-256:", b"Old manifest SHA-256:")
                self.write_zip()
                with self.assertRaisesRegex(ValueError, "Source manifest hash differs"):
                    self.verify()

    def test_wrong_checksum_rejected(self):
        self.checksum.write_text("0" * 64 + "  " + self.archive.name + "\n")
        with self.assertRaisesRegex(ValueError, "checksum"):
            self.verify()

    def test_bad_markdown_fragment_and_silently_dropped_relative_link(self):
        with self.assertRaisesRegex(ValueError, "heading link"):
            checker.markdown_links("[Contents](#missing)\n## 1. Start\n")
        with self.assertRaisesRegex(ValueError, "Unsupported manual link"):
            checker.markdown_links("## 1. Start\n[Source](../src/main.rs)\n")

    def test_missing_pdf_url_and_invalid_internal_destination(self):
        external, expected, chapters = checker.markdown_links((self.root / "docs/manual.md").read_text())
        for broken_internal in [False, True]:
            writer = PdfWriter()
            writer.clone_document_from_reader(PdfReader(self.pdf))
            if broken_internal:
                annot = writer.pages[0]["/Annots"][0].get_object()
                annot[NameObject("/Dest")] = ArrayObject([NumberObject(999), NameObject("/Fit")])
            else:
                writer.pages[1][NameObject("/Annots")] = ArrayObject([])
            output = io.BytesIO()
            writer.write(output)
            with self.assertRaisesRegex(ValueError, "outside the manual" if broken_internal else "external links differ"):
                checker.pdf_links(output.getvalue(), expected, chapters)

    def test_unsafe_and_duplicate_zip_entries_rejected(self):
        for name in ["../escape", "docs/manual.md"]:
            self.bundle()
            with zipfile.ZipFile(self.archive, "a") as archive:
                archive.writestr(self.stem + "/" + name, b"extra")
            self.update_checksum()
            with self.assertRaisesRegex(ValueError, "Unsafe ZIP" if name.startswith("..") else "Duplicate ZIP"):
                self.verify()

    def test_live_redirect_and_fragment(self):
        checker.check_urls({self.address + "/redirect#section"})
        with self.assertRaisesRegex(ValueError, "Missing live URL fragment"):
            checker.check_urls({self.address + "/ok#missing"})

    def test_live_404_and_timeout_fail_closed(self):
        with self.assertRaises(Exception):
            checker.check_urls({self.address + "/404"})
        with patch.object(checker, "urlopen", side_effect=TimeoutError("fixture timeout")):
            with self.assertRaises(TimeoutError):
                checker.check_urls({self.address + "/ok"})

    @unittest.skipUnless(sys.platform == "darwin", "Local macOS publisher")
    def test_broken_live_link_stops_publisher_before_github_mutation(self):
        self.bundle(self.address + "/404")
        tools = self.root / "dist/tools"
        tools.mkdir()
        for name, body in {
            "codesign": 'printf "Authority=Developer ID Application: Fixture\\nruntime\\n" >&2',
            "xcrun": "exit 0", "spctl": "exit 0",
            "gh": f'touch "{self.root}/dist/github-called"; exit 99',
        }.items():
            tool = tools / name
            tool.write_text("#!/bin/sh\n" + body + "\n")
            tool.chmod(0o755)
        subprocess.run(["git", "add", "."], cwd=self.root, check=True)
        subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                        "commit", "-qm", "Fixture"], cwd=self.root, check=True)
        subprocess.run(["git", "tag", "v1.0.0"], cwd=self.root, check=True)
        env = dict(os.environ, PYTHON=sys.executable, PATH=str(tools) + os.pathsep + os.environ["PATH"])
        env.pop("GITHUB_ACTIONS", None)
        result = subprocess.run(["bash", "scripts/publish-release.sh"], cwd=self.root,
                                env=env, text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("404", result.stderr)
        self.assertFalse((self.root / "dist/github-called").exists())


if __name__ == "__main__":
    unittest.main()
