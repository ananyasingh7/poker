"""Prepare and baseline Session 002. Run from repository root, without Tinker."""

import argparse
import gzip
import hashlib
import json
import random
from collections import defaultdict
from pathlib import Path
from urllib.request import urlopen

from inkling_bluff_practice import SYSTEM
from inkling_overnight import RULES, best_action, canonical, loss
from inkling_release_challenge import sample_deal
from inkling_sleep_study import CARD_READ
from inkling_split_audit import boards, pricing_cases
from inkling_transfer_audit import decide
from prepare_inkling_tune import classify

OUT = Path("results/16-inkling-session-002")
LEGACY = Path("results/14-inkling-finetuning/train.jsonl.gz")
SEEDS = (1622101, 1622102, 1622103)
PARENT = "tinker://e70a1d2c-a779-5de0-ad50-db4344636389:train:0/weights/000140"


def record(group, kind, view, system, target):
    return dict(
        group=group,
        kind=kind,
        target=target,
        messages=[
            dict(role="system", content=system),
            dict(role="user", content=canonical(view)),
        ],
    )


def group_examples(rng, group, rate, excluded_boards):
    deals = []
    for wins in (False, True):
        while True:
            deal = sample_deal(rng, wins, group % 2 == 0)
            board = tuple(sorted(deal["board"]))
            if board not in excluded_boards:
                excluded_boards.add(board)
                break
        deals.append(deal)
        view = dict(
            **deal, opponent_hand="exactly one pair of the board's highest rank"
        )
        yield record(group, "hand", view, CARD_READ, "win" if wins else "lose")

    pot = rng.randrange(100, 2100) / 10
    fair = pot * rate / (100 - rate)
    margin = rng.uniform(0.025, 0.12) if group % 3 == 0 else rng.uniform(0.15, 0.5)
    cheap, expensive = [round(fair * factor, 2) for factor in (1 - margin, 1 + margin)]
    for wins, bet in [
        (False, cheap),
        (False, expensive),
        (True, rng.choice([cheap, expensive])),
    ]:
        answer = best_action(rate / 100, wins, pot, bet)
        plain = dict(
            if_called="you win" if wins else "you lose",
            pot_bb=pot,
            bet_bb=bet,
            fold_percent=rate,
        )
        yield record(group, "price", plain, SYSTEM, answer)
        visible = dict(
            **deals[int(wins)],
            pot_bb=pot,
            bet_bb=bet,
            estimated_opponent_fold_percent=rate,
        )
        yield record(group, "combined", visible, RULES, answer)


def write_split(name, rows):
    data = "".join(canonical(row) + "\n" for row in rows).encode()
    path = OUT / (name + ".jsonl.gz")
    if path.exists() and gzip.decompress(path.read_bytes()) != data:
        raise ValueError("Frozen split changed: " + name)
    path.write_bytes(gzip.compress(data, mtime=0))
    return hashlib.sha256(data).hexdigest()


def prepare():
    rng = random.Random(1622000)
    excluded_boards = set()
    excluded_rates = set()
    for path in (
        LEGACY,
        Path("results/15-inkling-audit/size-challenge.jsonl.gz"),
        Path("runs/inkling-sleep-v1/dev.jsonl"),
        Path("runs/inkling-sleep-v1/final-exam.jsonl"),
    ):
        excluded_boards.update(boards(path))
        excluded_rates.update(float(key[1]) for key in pricing_cases(path))
    excluded_rates.update(
        [26.4, 41.2, 53.1, 62.4, 17, 21, 27, 32, 37, 44, 49, 54, 59, 67]
    )
    rates = [n / 10 for n in range(110, 751) if n / 10 not in excluded_rates]
    rng.shuffle(rates)
    groups = [
        list(group_examples(rng, i, rate, excluded_boards))
        for i, rate in enumerate(rates[:272])
    ]
    dev = [row for group in groups[:16] for row in group]
    train = []
    for group in groups[16:]:
        for row in group:
            train.append(
                dict(
                    messages=row["messages"]
                    + [
                        dict(
                            role="assistant",
                            content=canonical({"action": row["target"]}),
                        )
                    ]
                )
            )
    legacy = [json.loads(line) for line in gzip.open(LEGACY, "rt")]
    # Stratify rehearsal so every historical format is represented.
    by_format = defaultdict(list)
    for row in legacy:
        view = json.loads(row["messages"][1]["content"])
        by_format[classify(view)[0]].append(row)
    for rows in by_format.values():
        train.extend(rng.sample(rows, 64))
    rng.shuffle(train)
    return dict(
        parent=PARENT,
        shuffle_seeds=SEEDS,
        train_examples=len(train),
        dev_examples=len(dev),
        dev_groups=16,
        train_groups=256,
        rehearsal_examples=256,
        train_sha256=write_split("train", train),
        dev_sha256=write_split("dev", dev),
    )


def evaluate(candidate_seed=None):
    rows = [json.loads(line) for line in gzip.open(OUT / "dev.jsonl.gz", "rt")]
    adapters = json.load(urlopen("http://127.0.0.1:9933/lora-adapters"))
    expected = Path(
        "runs/inkling-tuning/001-river/selected-adapter/poker.gguf"
    ).resolve()
    arms = [("original", 0), ("step140", 1)]
    if candidate_seed is not None:
        expected = Path(
            "runs/inkling-tuning/002-pricing", str(candidate_seed), "final.gguf"
        ).resolve()
        arms = [(f"candidate-{candidate_seed}", 1)]
    if (
        len(adapters) != 1
        or Path(adapters[0]["path"]).resolve() != expected
        or adapters[0]["id"] != 0
    ):
        raise ValueError("Unexpected local adapter")
    for name, scale in arms:
        output = []
        for row in rows:
            system, user = row["messages"]
            prediction = decide(
                dict(
                    kind=row["kind"],
                    system=system["content"],
                    view=json.loads(user["content"]),
                ),
                scale,
            )
            output.append(dict(**row, **prediction))
        write_split(name, output)
        print(name, score(output), flush=True)


def score(rows):
    metrics = {}
    for kind in ["hand", "price", "combined"]:
        subset = [row for row in rows if row["kind"] == kind]
        result = dict(
            correct=sum(r["target"] == r["answer"] for r in subset),
            total=len(subset),
            invalid=sum(r["answer"] is None for r in subset),
        )
        if kind != "hand":
            pairs = defaultdict(list)
            losses = []
            bets = defaultdict(list)
            for row in subset:
                view = json.loads(row["messages"][1]["content"])
                _, wins = classify(view)
                rate = view.get(
                    "fold_percent", view.get("estimated_opponent_fold_percent")
                )
                losses.append(
                    loss(
                        row["answer"] or "check",
                        rate / 100,
                        wins,
                        view["pot_bb"],
                        view["bet_bb"],
                    )
                )
                category = (
                    "value"
                    if wins
                    else ("good_bluff" if row["target"] == "bet" else "bad_bluff")
                )
                bets[category].append(row["answer"] == "bet")
                if not wins:
                    pairs[row["group"]].append(row["answer"] == row["target"])
            assert all(len(v) == 2 for v in pairs.values())
            result.update(
                pairs_correct=sum(all(v) for v in pairs.values()),
                pairs=len(pairs),
                mean_loss_bb=sum(losses) / len(losses),
                bet_rates={
                    key: sum(values) / len(values) for key, values in bets.items()
                },
            )
        metrics[kind] = result
    return metrics


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--baseline", action="store_true")
    mode.add_argument("--candidate-seed", type=int, choices=SEEDS)
    args = parser.parse_args()
    if args.baseline or args.candidate_seed:
        evaluate(args.candidate_seed)
    else:
        print(json.dumps(prepare(), indent=2))
