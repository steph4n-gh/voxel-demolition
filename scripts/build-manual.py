#!/usr/bin/env python3
"""Typeset docs/manual.md as the illustrated player manual (requires ReportLab)."""
from __future__ import annotations

import argparse
import html
from pathlib import Path
import re

from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.utils import ImageReader
from reportlab.pdfbase.pdfmetrics import stringWidth
from reportlab.platypus import (
    BaseDocTemplate, Flowable, Frame, Image, KeepTogether, PageBreak,
    PageTemplate, Paragraph, Preformatted, Spacer, Table, TableStyle,
)
from reportlab.platypus.tableofcontents import TableOfContents

ROOT = Path(__file__).resolve().parents[1]
INK = colors.HexColor("#122331")
MUTED = colors.HexColor("#526978")
TEAL = colors.HexColor("#008B8F")
CYAN = colors.HexColor("#65E0D9")
ORANGE = colors.HexColor("#D25B28")
PAPER = colors.HexColor("#F1F6F6")
WIDTH, HEIGHT = 595.276, 841.89
MARGIN = 54
BODY_WIDTH = WIDTH - 2 * MARGIN

STYLES = getSampleStyleSheet()
STYLES.add(ParagraphStyle("Body", fontName="Helvetica", fontSize=10,
    leading=13, textColor=INK, spaceAfter=6, allowWidows=0, allowOrphans=0))
STYLES.add(ParagraphStyle("Chapter", fontName="Helvetica-Bold", fontSize=26,
    leading=30, textColor=INK, spaceBefore=6, spaceAfter=18, keepWithNext=True))
STYLES.add(ParagraphStyle("Subhead", fontName="Helvetica-Bold", fontSize=14,
    leading=18, textColor=TEAL, spaceBefore=12, spaceAfter=7, keepWithNext=True))
STYLES.add(ParagraphStyle("Minor", fontName="Helvetica-Bold", fontSize=11,
    leading=15, textColor=INK, spaceBefore=10, spaceAfter=5, keepWithNext=True))
STYLES.add(ParagraphStyle("Caption", fontSize=8.4, leading=12,
    textColor=MUTED, spaceBefore=6, spaceAfter=14))
STYLES.add(ParagraphStyle("Cell", fontName="Helvetica", fontSize=8.1,
    leading=11.3, textColor=INK))
STYLES.add(ParagraphStyle("CellHead", parent=STYLES["Cell"],
    fontName="Helvetica-Bold", textColor=colors.white))
STYLES.add(ParagraphStyle("BulletBody", parent=STYLES["Body"],
    leftIndent=13, firstLineIndent=-10, spaceAfter=4))
STYLES.add(ParagraphStyle("CodeBlock", fontName="Courier", fontSize=8.5,
    leading=12, textColor=INK, backColor=PAPER, borderPadding=10, spaceAfter=12))
STYLES.add(ParagraphStyle("ContentsTitle", parent=STYLES["Chapter"]))


def inline(text: str) -> str:
    """The manual uses headings, prose, lists, tables and simple inline markup."""
    text = text.translate(str.maketrans({"–": "-", "—": " - ", "→": "->",
                                       "←": "<-", "↔": "<->", "…": "..."}))
    text = html.escape(text)
    text = re.sub(r"`([^`]+)`", r'<font name="Courier">\1</font>', text)
    text = re.sub(r"\*\*([^*]+)\*\*", r"<b>\1</b>", text)
    text = re.sub(r"\*([^*]+)\*", r"<i>\1</i>", text)
    # Relative repository links remain readable in the printed edition.
    def link(match):
        label, target = match.groups()
        if target.startswith(("https://", "http://", "mailto:")):
            return f'<link href="{target}" color="#008B8F">{label}</link>'
        return label
    return re.sub(r"\[([^\]]+)\]\(([^)]+)\)", link, text)


class Diagram(Flowable):
    """Original schematic: the three steps of controlled demolition."""
    def __init__(self):
        super().__init__()
        self.width, self.height = BODY_WIDTH, 132

    def draw(self):
        c = self.canv
        c.setFillColor(PAPER)
        c.roundRect(0, 0, self.width, self.height, 8, stroke=0, fill=1)
        stages = [("01  READ THE LOAD", "Find the supports.", False),
                  ("02  BREAK THE PATH", "Remove a connection.", True),
                  ("03  LET IT SETTLE", "Watch the result.", True)]
        for i, (title, caption, cut) in enumerate(stages):
            x = 15 + i * 163
            for row in range(4):
                for col in range(4):
                    if cut and row == 0 and col in (1, 2):
                        continue
                    c.setFillColor(TEAL if row else ORANGE)
                    yy = 53 + row * 12 + (6 if i == 2 else 0)
                    c.rect(x + 24 + col * 13, yy, 11, 10, stroke=0, fill=1)
            c.setStrokeColor(MUTED)
            c.line(x + 17, 49, x + 90, 49)
            c.setFillColor(INK)
            c.setFont("Helvetica-Bold", 8.1)
            c.drawString(x, 28, title)
            c.setFont("Helvetica", 8.2)
            c.setFillColor(MUTED)
            c.drawString(x, 14, caption)
        c.setFont("Helvetica", 16)
        c.setFillColor(TEAL)
        c.drawString(148, 79, ">")
        c.drawString(311, 79, ">")


class ManualDocument(BaseDocTemplate):
    def __init__(self, path: Path):
        super().__init__(str(path), pagesize=(WIDTH, HEIGHT),
            title="Voxel Demolition: Oblivion Protocol - Player Manual",
            author="Voxel Demolition contributors", subject="Player guide and reference",
            leftMargin=MARGIN, rightMargin=MARGIN, topMargin=62, bottomMargin=54)
        self.chapter = "PLAYER MANUAL"
        self.bookmark_index = 0
        frame = Frame(MARGIN, 54, BODY_WIDTH, HEIGHT - 116,
                      leftPadding=0, rightPadding=0, topPadding=0, bottomPadding=0)
        self.addPageTemplates(PageTemplate(id="Manual", frames=frame,
                                          onPage=self.decorate))

    def beforeDocument(self):
        self.chapter = "PLAYER MANUAL"
        self.bookmark_index = 0

    def decorate(self, c, _doc):
        if self.page == 1:
            c.setFillColor(INK)
            c.rect(0, 0, WIDTH, HEIGHT, stroke=0, fill=1)
            # A small voxel skyline echoes the procedural game without extra art.
            for col in range(18):
                h = (col * 7 % 11) + 2
                for row in range(h):
                    c.setFillColor(colors.HexColor("#1B404C") if row % 3 else TEAL)
                    c.rect(22 + col * 32, 57 + row * 13, 25, 9, stroke=0, fill=1)
            c.setFillColor(CYAN)
            c.setFont("Helvetica-Bold", 10)
            c.drawString(MARGIN, HEIGHT - 79, "THE OFFICIAL PLAYER MANUAL")
            c.setFillColor(colors.white)
            c.setFont("Helvetica-Bold", 39)
            c.drawString(MARGIN, HEIGHT - 137, "VOXEL")
            c.drawString(MARGIN, HEIGHT - 181, "DEMOLITION")
            c.setFillColor(CYAN)
            c.setFont("Helvetica", 16)
            c.drawString(MARGIN, HEIGHT - 213, "OBLIVION PROTOCOL")
            img = ROOT / "docs/manual-assets/neo-tokyo.jpg"
            if img.exists():
                iw, ih = ImageReader(str(img)).getSize()
                c.drawImage(str(img), MARGIN, 258, width=BODY_WIDTH,
                            height=BODY_WIDTH * ih / iw, mask="auto")
            c.setFillColor(colors.white)
            c.setFont("Helvetica-Bold", 17)
            c.drawString(MARGIN, 220, "Plan the cut. Control the collapse.")
            c.setFont("Helvetica", 10.5)
            c.setFillColor(CYAN)
            c.drawString(MARGIN, 196, "Quick start / arsenal / contracts / tactics / reference")
            c.setFillColor(colors.white)
            c.setFont("Helvetica", 8.5)
            c.drawString(MARGIN, 35, "PLAYER EDITION  /  OCTOBER 2026")
            return
        c.setStrokeColor(TEAL)
        c.setLineWidth(1)
        c.line(MARGIN, HEIGHT - 38, WIDTH - MARGIN, HEIGHT - 38)
        c.setFillColor(MUTED)
        c.setFont("Helvetica-Bold", 7.3)
        c.drawString(MARGIN, HEIGHT - 29, "VOXEL DEMOLITION / OBLIVION PROTOCOL")
        c.setFont("Helvetica", 7.2)
        c.drawRightString(WIDTH - MARGIN, HEIGHT - 29, "PLAYER FIELD GUIDE")
        c.setStrokeColor(colors.HexColor("#CFDEDF"))
        c.line(MARGIN, 39, WIDTH - MARGIN, 39)
        c.setFont("Helvetica", 8)
        c.drawString(MARGIN, 25, "PLAYER MANUAL  /  2026 EDITION")
        c.drawRightString(WIDTH - MARGIN, 25, f"{self.page:02d}")

    def afterFlowable(self, flowable):
        if isinstance(flowable, Paragraph) and flowable.style.name in ("Chapter", "Subhead"):
            level = 0 if flowable.style.name == "Chapter" else 1
            title = flowable.getPlainText()
            if level == 0:
                self.chapter = title
            self.bookmark_index += 1
            key = f"heading-{self.bookmark_index}"
            self.canv.bookmarkPage(key)
            self.canv.addOutlineEntry(title, key, level=level, closed=(level == 0))
            if level == 0:
                self.notify("TOCEntry", (level, title, self.page, key))


def markdown_table(lines):
    rows = [[cell.strip() for cell in line.strip().strip("|").split("|")]
            for line in lines]
    rows = [row for row in rows if not all(re.fullmatch(r":?-+:?", c) for c in row)]
    cols = len(rows[0])
    for row in rows:
        if len(row) != cols:
            raise ValueError(f"Unequal table columns: {row}")
    weights = [max(min(len(row[i]), 55) for row in rows) for i in range(cols)]
    weights = [max(9, x) for x in weights]
    minimums = []
    for col in range(cols):
        words = " ".join(row[col] for row in rows).replace("**", "").replace("`", "").split()
        longest = max((stringWidth(word, "Helvetica", 8.1) for word in words), default=20)
        minimums.append(max(34, min(160, longest + 14)))
    available = max(0, BODY_WIDTH - sum(minimums))
    widths = [minimums[i] + available * weights[i] / sum(weights) for i in range(cols)]
    if sum(widths) > BODY_WIDTH:
        widths = [w * BODY_WIDTH / sum(widths) for w in widths]
    cells = [[Paragraph(inline(cell), STYLES["CellHead" if j == 0 else "Cell"])
              for cell in row] for j, row in enumerate(rows)]
    table = Table(cells, colWidths=widths, repeatRows=1, hAlign="LEFT")
    table.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, 0), INK),
        ("ROWBACKGROUNDS", (0, 1), (-1, -1), [colors.white, PAPER]),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 7),
        ("RIGHTPADDING", (0, 0), (-1, -1), 7),
        ("TOPPADDING", (0, 0), (-1, -1), 4.5),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 4.5),
        ("LINEBELOW", (0, 0), (-1, 0), 1.5, TEAL),
        ("LINEBELOW", (0, 1), (-1, -1), .3, colors.HexColor("#D4E1E2")),
    ]))
    return [KeepTogether([table, Spacer(1, 10)])]


def figure(name, caption):
    path = ROOT / "docs/manual-assets" / name
    if not path.exists():
        return []
    iw, ih = ImageReader(str(path)).getSize()
    return [KeepTogether([Image(str(path), width=BODY_WIDTH,
                               height=BODY_WIDTH * ih / iw),
                          Paragraph(caption, STYLES["Caption"])])]


def build(source: Path, output: Path):
    output.parent.mkdir(parents=True, exist_ok=True)
    story = [Spacer(1, 1), PageBreak(),
             Paragraph("Find your next move", STYLES["ContentsTitle"]),
             Paragraph("Start with the quick start, then keep the controls and arsenal pages "
                       "nearby. The contract guide explains what a legitimate win requires; "
                       "the tactics chapter helps you shape the collapse.", STYLES["Body"])]
    toc = TableOfContents()
    toc.levelStyles = [ParagraphStyle("TOCChapter", fontName="Helvetica-Bold", fontSize=10,
        leading=14, spaceBefore=7, textColor=INK),
        ParagraphStyle("TOCSubhead", fontName="Helvetica", fontSize=8.8, leading=11.8,
        leftIndent=12, spaceBefore=2, textColor=MUTED)]
    story.extend([toc, Spacer(1, 22), Diagram(),
                  Paragraph("Controlled demolition starts with the load path, then a deliberate cut, then observation of the collapse.",
                            STYLES["Caption"]), PageBreak()])
    lines = source.read_text().splitlines()
    i, chapter, first = 0, 0, True
    while i < len(lines):
        line = lines[i].strip()
        # Markdown's clickable contents and introduction are represented above.
        if first and not re.match(r"^##\s+1\.", line):
            i += 1
            continue
        if not line or line == "---":
            i += 1
            continue
        heading = re.match(r"^(#{1,4})\s+(.+)", line)
        if heading:
            depth, title = len(heading[1]), heading[2]
            if depth == 1:
                i += 1
                continue
            if depth == 2:
                if not first:
                    story.append(PageBreak())
                first = False
                chapter += 1
                story.append(Paragraph(f"FIELD GUIDE / {chapter:02d}", STYLES["Caption"]))
                story.append(Paragraph(inline(title), STYLES["Chapter"]))
                lower = title.lower()
                if "workbench" in lower:
                    story.extend(figure("workbench.jpg", "The workbench: choose a tab, then adjust only the settings you need."))
                elif "corporate" in lower:
                    story.extend(figure("corporate-drop.jpg", "Corporate Drop: the contract panel tracks destruction, collateral, ordnance and real time."))
            else:
                story.append(Paragraph(inline(title), STYLES["Subhead" if depth == 3 else "Minor"]))
            i += 1
            continue
        if line.startswith("|"):
            block = []
            while i < len(lines) and lines[i].strip().startswith("|"):
                block.append(lines[i]); i += 1
            story.extend(markdown_table(block))
            continue
        if line.startswith("```"):
            block = []
            i += 1
            while i < len(lines) and not lines[i].strip().startswith("```"):
                block.append(lines[i]); i += 1
            story.append(Preformatted("\n".join(block), STYLES["CodeBlock"]))
            i += 1
            continue
        bullet = re.match(r"^(?:[-*]\s+|(\d+)\.\s+)(.+)", line)
        if bullet:
            prefix = f"{bullet[1]}. " if bullet[1] else "- "
            story.append(Paragraph(inline(prefix + bullet[2]), STYLES["BulletBody"]))
            i += 1
            continue
        block = [line.removeprefix("> ")]
        i += 1
        while i < len(lines) and lines[i].strip() and not re.match(
            r"^(#{1,4}\s|\||```|[-*]\s|\d+\.\s|---)", lines[i].strip()):
            block.append(lines[i].strip().removeprefix("> ")); i += 1
        story.append(Paragraph(inline(" ".join(block)), STYLES["Body"]))
    # Keep short reference introductions with their table, including the heading.
    grouped, i = [], 0
    while i < len(story):
        flow = story[i]
        if isinstance(flow, Paragraph) and flow.style.name in ("Subhead", "Minor"):
            j = i + 1
            while j < len(story) and j <= i + 3 and isinstance(story[j], Paragraph) and story[j].style.name == "Body":
                j += 1
            if j < len(story) and isinstance(story[j], KeepTogether):
                grouped.append(KeepTogether(story[i:j] + story[j]._content))
                i = j + 1
                continue
        grouped.append(flow)
        i += 1
    ManualDocument(output).multiBuild(grouped)
    print(output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=ROOT / "docs/manual.md")
    parser.add_argument("--output", type=Path,
                        default=ROOT / "output/pdf/Voxel-Demolition-Player-Manual.pdf")
    args = parser.parse_args()
    build(args.source, args.output)
