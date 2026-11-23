"""Audit independent pricing cases, including repeated scales and stronger hands."""

import argparse
import gzip
import json
from fractions import Fraction
from pathlib import Path

from prepare_inkling_tune import classify


def pricing_key(view):
    _, wins = classify(view)
    if "bet_bb" not in view:
        return None
    ratio = Fraction(str(view["bet_bb"])) / Fraction(str(view["pot_bb"]))
    rate = view.get("fold_percent", view.get("estimated_opponent_fold_percent"))
    return ratio, Fraction(str(rate)), wins


def views(path):
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt") as stream:
        for line in stream:
            yield json.loads(json.loads(line)["messages"][1]["content"])


def pricing_cases(path):
    return {key for view in views(path) if (key := pricing_key(view)) is not None}


def boards(path):
    return {tuple(sorted(view["board"])) for view in views(path) if "board" in view}


def require_novel(reference, candidate):
    if not candidate:
        raise ValueError("No betting decisions in challenge")
    overlap = reference & candidate
    if overlap:
        raise ValueError(f"{len(overlap)} normalized pricing cases overlap")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--require-novel", action="store_true")
    parser.add_argument(
        "--release",
        action="store_true",
        help="Also require disjoint fold rates and boards",
    )
    args = parser.parse_args()
    reference = pricing_cases(args.reference)
    candidate = pricing_cases(args.candidate)
    print(
        f"Reference {len(reference)}, candidate {len(candidate)}, shared {len(reference & candidate)}"
    )
    if args.require_novel or args.release:
        require_novel(reference, candidate)
    if args.release:
        shared_rates = {key[1] for key in reference} & {key[1] for key in candidate}
        shared_boards = boards(args.reference) & boards(args.candidate)
        if shared_rates or shared_boards:
            raise ValueError(
                f"Release overlap: {len(shared_rates)} rates, {len(shared_boards)} boards"
            )
