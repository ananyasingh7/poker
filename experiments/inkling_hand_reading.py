"""Matched hand-reading training and end-to-end river policy evaluation."""

import argparse
import gzip
import hashlib
import json
import random
from collections import defaultdict
from pathlib import Path
from statistics import mean

from inkling_fresh_diagnosis import ARMS, evaluate, save
from inkling_overnight import canonical, interval, loss
from inkling_session002 import CARD_READ, PARENT, SYSTEM, record
from inkling_session003 import OUT as SESSION003
from inkling_session003 import REFERENCES, contrast, group_rows
from inkling_split_audit import views
from prepare_inkling_tune import hand_wins

OUT = Path("results/20-inkling-hand-reading")
RUN = Path("runs/inkling-tuning/004-hand")
SEEDS = (204001, 204002, 204003)
REVISION = Path("results/19-inkling-session-003-revision")


def load(name):
    with gzip.open(OUT / f"{name}.jsonl.gz", "rt") as stream:
        return list(map(json.loads, stream))


def report(name, data):
    (OUT / f"{name}.json").write_text(json.dumps(data, indent=2) + "\n")


def prepare():
    rng = random.Random(204000)
    used_boards = set()
    used_rates = {26.4, 41.2, 53.1, 62.4, 17, 21, 27, 30, 32, 37, 44, 49, 54, 59, 67}
    for path in (
        *REFERENCES,
        SESSION003 / "train.jsonl.gz",
        SESSION003 / "dev.jsonl.gz",
        REVISION / "train.jsonl.gz",
    ):
        for view in views(path):
            if "board" in view:
                used_boards.add(tuple(sorted(view["board"])))
            for field in ("fold_percent", "estimated_opponent_fold_percent"):
                if field in view:
                    used_rates.add(view[field])
    rates = rng.sample(
        [n / 10 for n in range(110, 751) if n / 10 not in used_rates], 32
    )
    dev = [
        row
        for group, rate in enumerate(rates)
        for row in group_rows(rng, group, rate, used_boards)
        if row["kind"] != "price"
    ]
    train = []
    for group in range(640):
        for deal in contrast(rng, group % 2 == 0, used_boards):
            target = "win" if hand_wins(deal) else "lose"
            view = dict(
                **deal, opponent_hand="exactly one pair of the board's highest rank"
            )
            row = record(group, "hand", view, CARD_READ, target)
            train.append(
                {
                    "messages": row["messages"]
                    + [{"role": "assistant", "content": canonical({"action": target})}]
                }
            )
    rng.shuffle(train)
    manifest = {
        "parent": PARENT,
        "shuffle_seeds": SEEDS,
        "train_examples": len(train),
        "train_boards": 640,
        "dev_boards": 32,
        "dev_examples": len(dev),
    }
    OUT.mkdir(exist_ok=True)
    for name, rows in [("train", train), ("dev", dev)]:
        save(name, rows, OUT)
        manifest[name + "_sha256"] = hashlib.sha256(
            gzip.decompress((OUT / f"{name}.jsonl.gz").read_bytes())
        ).hexdigest()
    report("manifest", manifest)


def card_key(row, view):
    return row["group"], tuple(view["board"]), tuple(view["hole"])


def conditioned_questions(first):
    reads = {}
    questions = []
    for row in first:
        if row["kind"] == "hand":
            view = json.loads(row["messages"][1]["content"])
            key = card_key(row, view)
            if key in reads or row["answer"] not in ("win", "lose"):
                raise ValueError("Duplicate or invalid card read")
            reads[key] = row["answer"]
    for row in first:
        if row["kind"] == "combined":
            deal = json.loads(row["messages"][1]["content"])
            read = reads[card_key(row, deal)]
            view = {
                "if_called": "you " + read,
                "pot_bb": deal["pot_bb"],
                "bet_bb": deal["bet_bb"],
                "fold_percent": deal["estimated_opponent_fold_percent"],
            }
            questions.append(record(row["group"], "price", view, SYSTEM, row["target"]))
    return questions


def verify(questions, answers):
    for question, answer in zip(questions, answers, strict=True):
        if any(answer[key] != value for key, value in question.items()):
            raise ValueError("Mismatched replay")


def summarize(first, second):
    verify(conditioned_questions(first), second)
    reads = [row for row in first if row["kind"] == "hand"]
    combined = [row for row in first if row["kind"] == "combined"]
    result = {
        "reads_correct": sum(row["target"] == row["answer"] for row in reads),
        "reads": len(reads),
        "false_wins": sum(
            row["target"] == "lose" and row["answer"] == "win" for row in reads
        ),
        "false_losses": sum(
            row["target"] == "win" and row["answer"] == "lose" for row in reads
        ),
    }
    groups = defaultdict(list)
    for row in reads:
        groups[row["group"]].append(row["target"] == row["answer"])
    result["both_reads_correct"] = sum(all(values) for values in groups.values())
    for mode, answers in [
        ("direct", combined),
        ("read_then_act", second),
        ("always_bet", [{"answer": "bet"}] * len(combined)),
    ]:
        board_losses = defaultdict(list)
        counts = {"winning_bets": 0, "expensive_bluffs": 0, "cheap_bluffs": 0}
        for row, reply in zip(combined, answers, strict=True):
            view = json.loads(row["messages"][1]["content"])
            wins = hand_wins(view)
            action = reply["answer"]
            if action not in ("bet", "check"):
                raise ValueError("Invalid betting action")
            bucket = (
                "winning_bets"
                if wins
                else ("cheap_bluffs" if row["target"] == "bet" else "expensive_bluffs")
            )
            counts[bucket] += action == "bet"
            board_losses[row["group"]].append(
                loss(
                    action,
                    view["estimated_opponent_fold_percent"] / 100,
                    wins,
                    view["pot_bb"],
                    view["bet_bb"],
                )
            )
        if sorted(board_losses) != list(range(32)) or any(
            len(v) != 4 for v in board_losses.values()
        ):
            raise ValueError("Incomplete board groups")
        per_board = [mean(board_losses[group]) for group in sorted(board_losses)]
        result[mode] = dict(
            **counts,
            decisions=len(answers),
            mean_loss_bb=mean(per_board),
            loss_bb_by_board=per_board,
        )
    return result


def replay(name, adapter=None):
    questions = load("dev")
    if adapter is None:
        first = load(name)
        verify(questions, first)
        return summarize(first, load(name + "-read-then-act"))
    first = evaluate(name, adapter, questions, RUN, OUT)
    second = evaluate(
        name + "-read-then-act", adapter, conditioned_questions(first), RUN, OUT
    )
    return summarize(first, second)


def savings(before, after):
    return interval(
        [
            a - b
            for a, b in zip(
                before["loss_bb_by_board"], after["loss_bb_by_board"], strict=True
            )
        ]
    )


def compare(parent, scores):
    representative = sorted(
        scores, key=lambda seed: (scores[seed]["read_then_act"]["mean_loss_bb"], seed)
    )[len(scores) // 2]
    for candidate in scores.values():
        candidate["savings_vs_parent"] = {
            mode: savings(parent[mode], candidate["read_then_act"])
            for mode in ("direct", "read_then_act")
        }
        candidate["direct_savings_vs_parent_direct"] = savings(
            parent["direct"], candidate["direct"]
        )
    selected = scores[representative]
    policy = selected["read_then_act"]
    advance = (
        selected["reads_correct"] > parent["reads_correct"]
        and all(v["interval_95"][0] > 0 for v in selected["savings_vs_parent"].values())
        and policy["mean_loss_bb"] < parent["always_bet"]["mean_loss_bb"]
        and policy["winning_bets"] >= parent["direct"]["winning_bets"]
        and policy["expensive_bluffs"] <= parent["direct"]["expensive_bluffs"]
    )
    return {"by_seed": scores, "representative": representative, "advance": advance}


def main(mode, wait=False):
    if mode == "prepare":
        prepare()
    elif mode == "baseline":
        parent = replay("step140", ARMS["step140"])
        parent["read_then_act_savings_vs_direct"] = savings(
            parent["direct"], parent["read_then_act"]
        )
        report("baseline", parent)
    elif mode in ("score", "rescore"):
        from evaluate_inkling_session002 import export, final_checkpoint

        parent = replay("step140")
        scores = {}
        for seed in SEEDS:
            folder = RUN / str(seed)
            adapter = (
                export(folder, final_checkpoint(folder, wait))
                if mode == "score"
                else None
            )
            scores[seed] = replay(str(seed), adapter)
        report("results", compare(parent, scores))
    else:
        from train_inkling_session002 import main as render

        render(
            False,
            out=OUT,
            run=RUN,
            seeds=SEEDS,
            session="004-hand",
            price_per_million=0,
        )
        if mode == "train":
            from train_inkling_free import train

            if not (OUT / "baseline.json").is_file():
                raise ValueError("Complete both local baseline policies first")
            train(OUT, RUN, OUT, "004-hand", seeds=SEEDS, external_final_only=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "mode",
        choices=("prepare", "baseline", "preflight", "train", "score", "rescore"),
    )
    parser.add_argument(
        "--wait", action="store_true", help="During scoring, wait for each final export"
    )
    args = parser.parse_args()
    main(args.mode, args.wait)
