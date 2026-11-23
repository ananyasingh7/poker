"""Prepare an immutable, offline session bundle for the synthetic river drill."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import random
import re
import tempfile

from inkling_overnight import canonical

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "examples/14-inkling-finetuning/session-001.json"


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prompt_key(messages):
    return canonical([messages[0]["content"], json.loads(messages[1]["content"])])


def hand_wins(view):
    # In this generator Seat 1 has exactly top pair on an unpaired board.
    cards = view["hole"] + view["board"]
    ranks = ["23456789TJQKA".index(c[0]) + 2 for c in cards]
    counts = Counter(ranks)
    unique = set(ranks)
    if 14 in unique:
        unique.add(1)
    straight = any(set(range(low, low + 5)) <= unique for low in range(1, 11))
    if len(set(cards)) != 7:
        raise ValueError("Invalid deal")
    if (
        straight
        or max(Counter(c[1] for c in cards).values()) >= 5
        or max(counts.values()) >= 3
        or sum(v >= 2 for v in counts.values()) >= 2
    ):
        return True
    # Remaining generated pairs are below top pair; a tied pair would require kickers.
    pairs = [rank for rank, count in counts.items() if count == 2]
    if pairs and pairs[0] == max(ranks[2:]):
        raise ValueError("Unexpected kicker-dependent matchup")
    return bool(pairs and pairs[0] > max(ranks[2:]))


def classify(view):
    if "if_called" in view:
        return "plain", view["if_called"] == "you win"
    win = hand_wins(view)
    if "opponent_hand" in view:
        return "hand_read", win
    if "your_hand" in view:
        if win != ("beats Seat 1" in view["your_hand"]):
            raise ValueError("Incorrect hand-strength hint")
        return "table", win
    return "cards_only", win


def prepare(config_path, output_root):
    config = json.loads(config_path.read_text())
    name = config["session"]
    if not re.fullmatch(r"[a-zA-Z0-9_-]+", name):
        raise ValueError("Invalid session name")
    data = ROOT / config["dataset"]
    exam = data / "final-exam.jsonl"
    exam_hash = digest(exam)
    lock = json.loads(
        (ROOT / "results/13-inkling-sleep/final-exam-manifest.json").read_text()
    )
    if exam_hash != lock["sha256"]:
        raise ValueError("Final exam differs from its original lock")
    # Leakage check uses normalized inputs only.
    reserved = {
        prompt_key(r["messages"])
        for f in ("dev.jsonl", exam.name)
        for r in rows(data / f)
    }
    counts, seen, train = Counter(), set(), []
    for row in rows(data / "train.jsonl"):
        msgs = row["messages"]
        if [m["role"] for m in msgs] != ["system", "user", "assistant"]:
            raise ValueError("Invalid conversation")
        key = prompt_key(msgs)
        if key in reserved or key in seen:
            raise ValueError("Repeated or reserved training prompt")
        seen.add(key)
        view = json.loads(msgs[1]["content"])
        fmt, win = classify(view)
        if fmt == "hand_read":
            target = "win" if win else "lose"
            category = target
        else:
            pct = view.get("fold_percent", view.get("estimated_opponent_fold_percent"))
            gain = (
                (100 - pct) * view["bet_bb"]
                if win
                else pct * view["pot_bb"] - (100 - pct) * view["bet_bb"]
            )
            if abs(gain) < 1e-8:
                raise ValueError("Unexpected tie in this threshold dataset")
            target = "bet" if gain > 0 else "check"
            category = (
                "value" if win else ("good_bluff" if target == "bet" else "bad_bluff")
            )
        if json.loads(msgs[-1]["content"]) != {"action": target}:
            raise ValueError("Wrong training target")
        if fmt in config["formats"]:
            counts[f"{fmt}/{category}"] += 1
            train.append(row)
    if set(config["formats"]) != {k.split("/")[0] for k in counts}:
        raise ValueError("Missing requested format")
    random.Random(config["shuffle_seed"]).shuffle(train)
    output_root.mkdir(parents=True, exist_ok=True)
    dest = output_root / name
    with tempfile.TemporaryDirectory(dir=output_root) as tmp:
        bundle = Path(tmp) / "bundle"
        bundle.mkdir()
        (bundle / "train.jsonl").write_text("".join(canonical(r) + "\n" for r in train))
        manifest = {
            "config": config,
            "examples": len(train),
            "categories": dict(sorted(counts.items())),
            "source_train_sha256": digest(data / "train.jsonl"),
            "dev_sha256": digest(data / "dev.jsonl"),
            "locked_exam_sha256": exam_hash,
            "train_sha256": digest(bundle / "train.jsonl"),
        }
        (bundle / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        if not dest.exists():
            bundle.rename(dest)
        elif any(digest(dest / f.name) != digest(f) for f in bundle.iterdir()):
            raise ValueError(
                "Session already exists with different content; use a fresh session ID"
            )
    print(f"{dest}: {len(train)} verified examples")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", nargs="?", type=Path, default=DEFAULT)
    parser.add_argument(
        "--output-root", type=Path, default=ROOT / "runs/inkling-tuning"
    )
    args = parser.parse_args()
    prepare(args.config, args.output_root)
