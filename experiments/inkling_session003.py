"""Prepare, evaluate, or train Session 003 from the repository root."""

import argparse
import gzip
import hashlib
import json
import random
from pathlib import Path

from inkling_fresh_diagnosis import ARMS, RESULT, evaluate, save
from inkling_overnight import RULES, best_action, canonical
from inkling_release_challenge import sample_deal
from inkling_session002 import (
    CARD_READ,
    LEGACY,
    PARENT,
    SYSTEM,
    record,
    score,
)
from inkling_session002 import (
    OUT as PREVIOUS,
)
from inkling_split_audit import boards, pricing_cases
from prepare_inkling_tune import hand_wins

OUT = Path("results/18-inkling-session-003")
RUN = Path("runs/inkling-tuning/003-value")
SEEDS = (183001, 183002, 183003)
DECK = [r + s for r in "23456789TJQKA" for s in "cdhs"]
REFERENCES = (
    LEGACY,
    PREVIOUS / "train.jsonl.gz",
    PREVIOUS / "dev.jsonl.gz",
    RESULT / "questions.jsonl.gz",
    Path("results/15-inkling-audit/size-challenge.jsonl.gz"),
    Path("runs/inkling-sleep-v1/dev.jsonl"),
    Path("runs/inkling-sleep-v1/final-exam.jsonl"),
)


def contrast(rng, shared, excluded):
    while True:
        winner = sample_deal(rng, True, shared)
        board = winner["board"]
        key = tuple(sorted(board))
        if key in excluded:
            continue
        replacements = [
            (i, c) for i in (0, 1) for c in DECK if c not in board + winner["hole"]
        ]
        rng.shuffle(replacements)
        for i, c in replacements:
            hole = winner["hole"].copy()
            hole[i] = c
            if any(h[0] == b[0] for h in hole for b in board) != shared:
                continue
            loser = {"board": board, "hole": hole}
            try:
                wins = hand_wins(loser)
            except ValueError:
                continue  # A tie on top pair depends on unspecified kickers.
            if not wins:
                excluded.add(key)
                return loser, winner


def group_rows(rng, group, rate, excluded):
    deals = contrast(rng, group % 2 == 0, excluded)
    pot = rng.randrange(100, 2100) / 10
    fair = pot * rate / (100 - rate)
    margin = rng.uniform(0.025, 0.12) if group % 3 == 0 else rng.uniform(0.15, 0.5)
    for wins, deal in enumerate(deals):
        yield record(
            group,
            "hand",
            dict(**deal, opponent_hand="exactly one pair of the board's highest rank"),
            CARD_READ,
            "win" if wins else "lose",
        )
        for factor in (1 - margin, 1 + margin):
            bet = round(fair * factor, 2)
            target = best_action(rate / 100, wins, pot, bet)
            yield record(
                group,
                "combined",
                dict(
                    **deal, pot_bb=pot, bet_bb=bet, estimated_opponent_fold_percent=rate
                ),
                RULES,
                target,
            )
            yield record(
                group,
                "price",
                {
                    "if_called": "you win" if wins else "you lose",
                    "pot_bb": pot,
                    "bet_bb": bet,
                    "fold_percent": rate,
                },
                SYSTEM,
                target,
            )


def prepare():
    rng = random.Random(183000)
    used_boards = set()
    used_rates = {26.4, 41.2, 53.1, 62.4, 17, 21, 27, 30, 32, 37, 44, 49, 54, 59, 67}
    for path in REFERENCES:
        used_boards.update(boards(path))
        used_rates.update(float(k[1]) for k in pricing_cases(path))
    rates = rng.sample(
        [n / 10 for n in range(110, 751) if n / 10 not in used_rates], 160
    )
    dev = []
    train = []
    for group, rate in enumerate(rates):
        for row in group_rows(rng, group, rate, used_boards):
            if group < 32:
                dev.append(row)
            else:
                train.append(
                    {
                        "messages": row["messages"]
                        + [
                            {
                                "role": "assistant",
                                "content": canonical({"action": row["target"]}),
                            }
                        ]
                    }
                )
    rng.shuffle(train)
    OUT.mkdir(exist_ok=True)
    manifest = {
        "parent": PARENT,
        "shuffle_seeds": SEEDS,
        "train_groups": 128,
        "dev_groups": 32,
        "train_examples": len(train),
        "dev_examples": len(dev),
    }
    for name, rows in [("train", train), ("dev", dev)]:
        save(name, rows, OUT)
        manifest[name + "_sha256"] = hashlib.sha256(
            gzip.decompress((OUT / f"{name}.jsonl.gz").read_bytes())
        ).hexdigest()
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


def baseline():
    with gzip.open(OUT / "dev.jsonl.gz", "rt") as handle:
        rows = list(map(json.loads, handle))
    answers = evaluate("step140", ARMS["step140"], rows, RUN, OUT)
    (OUT / "baseline.json").write_text(json.dumps(score(answers), indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("prepare", "baseline", "preflight", "train"))
    mode = parser.parse_args().mode
    if mode == "prepare":
        prepare()
    elif mode == "baseline":
        baseline()
    else:
        from train_inkling_session002 import main

        main(
            mode == "train",
            out=OUT,
            run=RUN,
            seeds=SEEDS,
            required_baselines=("step140",),
            session="003",
        )
