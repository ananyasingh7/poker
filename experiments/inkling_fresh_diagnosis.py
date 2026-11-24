"""Replay four fixed local adapters on fresh, matched Session 002 development questions."""

import argparse
import gzip
import hashlib
import json
import random
from pathlib import Path
from statistics import mean

from evaluate_inkling_session002 import RUN, local_server
from inkling_overnight import canonical, loss
from inkling_session002 import LEGACY, OUT, SEEDS, group_examples, score
from inkling_split_audit import boards, pricing_cases
from inkling_transfer_audit import decide
from prepare_inkling_tune import classify

RESULT = Path("results/17-inkling-fresh-diagnosis")
CACHE = Path("runs/inkling-fresh-diagnosis")
ARMS = {
    "step140": Path("runs/inkling-tuning/001-river/selected-adapter/poker.gguf"),
    **{str(seed): RUN / str(seed) / "final.gguf" for seed in SEEDS},
}


def save(name, rows):
    path = RESULT / f"{name}.jsonl.gz"
    data = "".join(canonical(row) + "\n" for row in rows).encode()
    if path.exists() and gzip.decompress(path.read_bytes()) != data:
        raise ValueError(f"Changed frozen artifact: {path}")
    tmp = path.with_suffix(".tmp")
    tmp.write_bytes(gzip.compress(data, mtime=0))
    tmp.replace(path)


def questions():
    rng = random.Random(1724001)
    excluded_boards = set()
    used_rates = {26.4, 41.2, 53.1, 62.4, 17, 21, 27, 32, 37, 44, 49, 54, 59, 67, 30}
    for path in [
        LEGACY,
        OUT / "train.jsonl.gz",
        OUT / "dev.jsonl.gz",
        Path("results/15-inkling-audit/size-challenge.jsonl.gz"),
        Path("runs/inkling-sleep-v1/dev.jsonl"),
        Path("runs/inkling-sleep-v1/final-exam.jsonl"),
    ]:
        excluded_boards.update(boards(path))
        used_rates.update(float(key[1]) for key in pricing_cases(path))
    rates = rng.sample(
        [v / 10 for v in range(110, 751) if v / 10 not in used_rates], 32
    )
    return [
        row
        for group, rate in enumerate(rates)
        for row in group_examples(rng, group, rate, excluded_boards)
    ]


def evaluate(name, adapter, rows):
    folder = CACHE / name
    folder.mkdir(parents=True, exist_ok=True)
    journal = folder / "answers.jsonl"
    with adapter.open("rb") as handle:
        identity = hashlib.file_digest(handle, "sha256").hexdigest()
    fingerprint = hashlib.sha256((identity + canonical(rows)).encode()).hexdigest()
    saved = (
        [json.loads(line) for line in journal.read_text().splitlines()]
        if journal.exists()
        else []
    )
    if len(saved) > len(rows) or any(
        r["fingerprint"] != fingerprint or r["index"] != i for i, r in enumerate(saved)
    ):
        raise ValueError("Journal belongs to a different replay")
    answers = [r["response"] for r in saved]
    if len(answers) < len(rows):
        with local_server(folder, adapter), journal.open("a") as log:
            for i in range(len(answers), len(rows)):
                row = rows[i]
                system, user = row["messages"]
                response = decide(
                    {
                        "kind": row["kind"],
                        "system": system["content"],
                        "view": json.loads(user["content"]),
                    },
                    1,
                )
                log.write(
                    canonical(
                        {"index": i, "fingerprint": fingerprint, "response": response}
                    )
                    + "\n"
                )
                log.flush()
                answers.append(response)
    outputs = [dict(**r, **a) for r, a in zip(rows, answers, strict=True)]
    save(name, outputs)
    return outputs


def diagnose(rows):
    reads = {}
    prices = {}
    for row in rows:
        v = json.loads(row["messages"][1]["content"])
        if row["kind"] == "hand":
            reads[row["group"], tuple(v["board"]), tuple(v["hole"])] = row
        elif row["kind"] == "price":
            prices[
                row["group"], v["if_called"] == "you win", v["pot_bb"], v["bet_bb"]
            ] = row
    ledger = []
    for row in rows:
        if row["kind"] != "combined":
            continue
        v = json.loads(row["messages"][1]["content"])
        _, wins = classify(v)
        hand = reads[row["group"], tuple(v["board"]), tuple(v["hole"])]
        price = prices[row["group"], wins, v["pot_bb"], v["bet_bb"]]
        cost = loss(
            row["answer"] or "check",
            v["estimated_opponent_fold_percent"] / 100,
            wins,
            v["pot_bb"],
            v["bet_bb"],
        )
        if None in (hand["answer"], price["answer"], row["answer"]):
            category = "invalid"
        elif row["answer"] == row["target"]:
            category = "correct"
        elif hand["answer"] == hand["target"]:
            category = (
                "combined_only" if price["answer"] == price["target"] else "price_wrong"
            )
        else:
            category = (
                "hand_wrong" if price["answer"] == price["target"] else "both_wrong"
            )
        ledger.append(
            {
                "group": row["group"],
                "view": v,
                "hand_answer": hand["answer"],
                "hand_target": hand["target"],
                "price_answer": price["answer"],
                "target": row["target"],
                "answer": row["answer"],
                "category": category,
                "loss_bb": cost,
            }
        )
    return ledger


def paired_savings(parent, candidate):
    """Resample whole groups, keeping the two bluff prices and value hand together."""

    def by_group(ledger):
        groups = {}
        for row in ledger:
            groups.setdefault(row["group"], []).append(row["loss_bb"])
        if len(groups) != 32 or any(len(v) != 3 for v in groups.values()):
            raise ValueError("Incomplete fresh replay")
        return {group: mean(values) for group, values in groups.items()}

    before, after = by_group(parent), by_group(candidate)
    differences = [before[group] - after[group] for group in sorted(before)]
    rng = random.Random(1724002)
    draws = sorted(
        mean(rng.choices(differences, k=len(differences))) for _ in range(10000)
    )
    return {"mean_bb": mean(differences), "interval_95_bb": [draws[249], draws[9749]]}


def main(rescore):
    RESULT.mkdir(exist_ok=True)
    path = RESULT / "questions.jsonl.gz"
    if not rescore:
        save("questions", questions())
    with gzip.open(path, "rt") as handle:
        rows = list(map(json.loads, handle))
    scores = {}
    ledgers = {}
    for name, adapter in ARMS.items():
        if rescore:
            with gzip.open(RESULT / f"{name}.jsonl.gz", "rt") as handle:
                outputs = list(map(json.loads, handle))
            if [
                {k: r[k] for k in q} for q, r in zip(rows, outputs, strict=True)
            ] != rows:
                raise ValueError("Response/question mismatch")
        else:
            outputs = evaluate(name, adapter, rows)
        ledger = diagnose(outputs)
        ledgers[name] = ledger
        save(f"{name}-decisions", ledger)
        categories = {}
        for row in ledger:
            bucket = categories.setdefault(row["category"], {"count": 0, "loss_bb": 0})
            bucket["count"] += 1
            bucket["loss_bb"] += row["loss_bb"]
        scores[name] = {
            "metrics": score(outputs),
            "full_decisions": len(ledger),
            "categories": categories,
        }
        print(name, scores[name], flush=True)
    for name, ledger in ledgers.items():
        scores[name]["savings_vs_step140"] = paired_savings(ledgers["step140"], ledger)
    (RESULT / "scores.json").write_text(json.dumps(scores, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rescore", action="store_true")
    main(parser.parse_args().rescore)
