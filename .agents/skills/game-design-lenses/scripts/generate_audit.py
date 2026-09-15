#!/usr/bin/env python3
"""Generate a gameplay-design audit checklist from the 100 lenses.

Selects lens groups, or the full audit, and writes a Markdown checklist with
checkboxes for each lens question. Answers are filled in during a design review.

Usage:
  generate_audit.py                       # core gameplay audit (default)
  generate_audit.py --all                 # all 100 lenses
  generate_audit.py --groups core balance # named groups
  generate_audit.py --lenses 1,7,32,33    # explicit lens numbers
  generate_audit.py --out audit.md        # output file (default: gameplay-audit.md)
  generate_audit.py --list                # list groups and lenses
"""
import argparse
import sys
from pathlib import Path

DATA = Path(__file__).parent / "data" / "lenses.json"

GROUPS = {
    "core": (1, 2, 3, 4, 5, 6, 7, 8, 9, 12, 15, 16, 18, 19, 20, 47),
    "mechanics": (21, 22, 23, 24, 25, 26, 27, 53, 71),
    "balance": (28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 45, 46),
    "interface": (48, 49, 50, 51, 52, 54, 55, 56, 57, 58, 59, 60),
    "interest": (61, 62, 63, 64),
    "story": (65, 66, 67, 68, 69, 70, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83),
    "social": (84, 85, 86, 87, 88),
    "process": (13, 14, 17, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100),
}

DEFAULT_AUDIT = ("core", "mechanics", "balance", "interface", "interest")


def load():
    import json
    return {e["n"]: e for e in json.loads(DATA.read_text(encoding="utf-8"))}


def render(lenses, title, subtitle=""):
    lines = [f"# Gameplay Design Audit — {title}", ""]
    if subtitle:
        lines += [subtitle, ""]
    lines += ["Based on the 100 lenses from *The Art of Game Design: A Book of Lenses* (Schell).",
              "Work top to bottom. For each question, answer concretely for the current build;",
              "flag ❌ where the answer is no or unknown — those are the design risks.", ""]
    for n in lenses:
        e = lenses[n]
        lines.append(f"## Lens #{n} — {e['title']}")
        if e.get("intro"):
            lines.append(e["intro"])
        lines.append("")
        for q in e["questions"]:
            lines.append(f"- [ ] {q}")
        lines.append("")
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser(description="Generate a lens-based gameplay audit checklist")
    ap.add_argument("--all", action="store_true", help="all 100 lenses")
    ap.add_argument("--groups", nargs="*", choices=sorted(GROUPS), help="named lens groups")
    ap.add_argument("--lenses", help="comma-separated lens numbers")
    ap.add_argument("--out", default="gameplay-audit.md", help="output file")
    ap.add_argument("--list", action="store_true", help="list groups and quit")
    args = ap.parse_args()

    lenses = load()

    if args.list:
        for g, nums in GROUPS.items():
            print(f"{g}: lens {', '.join(map(str, nums))}")
        return 0

    if args.all:
        nums = list(range(1, 101))
        title = "All 100 Lenses"
    elif args.lenses:
        nums = [int(x) for x in args.lenses.split(",") if x.strip().isdigit()]
        title = "Selected Lenses " + ", ".join(map(str, nums))
    elif args.groups:
        nums = []
        for g in args.groups:
            nums.extend(GROUPS[g])
        title = " + ".join(g.title() for g in args.groups)
    else:
        nums = []
        for g in DEFAULT_AUDIT:
            nums.extend(GROUPS[g])
        title = "Core Gameplay Audit"
        nums = sorted(set(nums))

    nums = sorted(set(nums))
    if not nums:
        print("no lenses selected", file=sys.stderr)
        return 1

    text = render(lenses, title)
    Path(args.out).write_text(text, encoding="utf-8")
    print(f"wrote {args.out}: {len(nums)} lenses, "
          f"{sum(len(lenses[n]['questions']) for n in nums)} questions")
    return 0


if __name__ == "__main__":
    sys.exit(main())