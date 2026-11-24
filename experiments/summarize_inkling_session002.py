"""Apply the frozen Session 002 comparison to all final replicates."""

import gzip
import hashlib
import json
import random
from pathlib import Path
from statistics import mean

from inkling_overnight import best_action, loss
from inkling_session002 import OUT, SEEDS, score
from prepare_inkling_tune import classify
from score_inkling_tune import score as historical_score


def load_predictions(name):
    with gzip.open(OUT / (name + ".jsonl.gz"), "rt") as handle:
        return [json.loads(line) for line in handle]


def group_losses(rows):
    costs = {}
    for row in rows:
        if row["kind"] != "combined":
            continue
        view = json.loads(row["messages"][1]["content"])
        _, wins = classify(view)
        cost = loss(
            row["answer"] or "check",
            view["estimated_opponent_fold_percent"] / 100,
            wins,
            view["pot_bb"],
            view["bet_bb"],
        )
        costs.setdefault(row["group"], []).append(cost)
    if len(costs) != 16 or any(len(group) != 3 for group in costs.values()):
        raise ValueError("Incomplete development groups")
    return {key: mean(values) for key, values in costs.items()}


def savings_interval(parent, candidate):
    baseline = group_losses(parent)
    tuned = group_losses(candidate)
    savings = [cost - tuned[group] for group, cost in baseline.items()]
    rng = random.Random(1622000)
    means = sorted(mean(rng.choices(savings, k=len(savings))) for _ in range(10000))
    return [means[249], means[9749]]


def separate_and_combined(rows):
    """Match each full decision to the two standalone questions about that exact spot."""
    reads = {}
    prices = {}
    decisions = []
    for row in rows:
        view = json.loads(row["messages"][1]["content"])
        if row["kind"] == "price":
            _, wins = classify(view)
            prices[row["group"], wins, view["pot_bb"], view["bet_bb"]] = (
                row["answer"] == row["target"]
            )
            continue

        deal = (row["group"], tuple(view["board"]), tuple(view["hole"]))
        if row["kind"] == "hand":
            reads[deal] = row["answer"]
        else:
            decisions.append((row, view, deal))

    both_right = []
    calculator_losses = []
    for row, view, deal in decisions:
        pot, bet = view["pot_bb"], view["bet_bb"]
        rate = view["estimated_opponent_fold_percent"] / 100
        _, wins = classify(view)
        read = reads[deal]
        if read not in {"win", "lose"}:
            raise ValueError("Missing hand read for the calculator replay")
        read_wins = read == "win"
        if read_wins == wins and prices[row["group"], wins, pot, bet]:
            both_right.append(row["answer"] == row["target"])
        action = best_action(rate, read_wins, pot, bet)
        calculator_losses.append(loss(action, rate, wins, pot, bet))
    return {
        "both_standalone_correct": len(both_right),
        "combined_errors_among_these": both_right.count(False),
        "model_read_plus_calculator_mean_loss_bb": mean(calculator_losses),
    }


def summarize():
    folder = Path("runs/inkling-tuning/002-pricing")
    manifest = json.loads((OUT / "manifest.json").read_text())
    recipe = json.loads((OUT / "render-check.json").read_text())
    questions = load_predictions("dev")
    baseline = load_predictions("step140")
    parent = score(baseline)
    controls = load_predictions("retention-questions")
    parent_history = historical_score(
        controls, load_predictions("retention-parent-local")
    )["overall_betting"]
    arms = {}
    for seed in SEEDS:
        rows = load_predictions(f"candidate-{seed}")
        if [
            {k: v for k, v in row.items() if k not in {"answer", "raw"}} for row in rows
        ] != questions:
            raise ValueError(f"Mismatched questions for seed {seed}")
        history = historical_score(controls, load_predictions(f"retention-{seed}"))[
            "overall_betting"
        ]
        metrics = score(rows)
        interval = savings_interval(baseline, rows)
        trainer = folder / str(seed) / "trainer"
        logs = list(
            map(json.loads, (trainer / "metrics.jsonl").read_text().splitlines())
        )
        if [row["step"] for row in logs] != list(range(recipe["batches"])) or sum(
            row["num_sequences"] for row in logs
        ) != manifest["train_examples"]:
            raise ValueError(f"Incomplete training log for seed {seed}")
        (final,) = [
            c
            for c in map(
                json.loads, (trainer / "checkpoints.jsonl").read_text().splitlines()
            )
            if c["name"] == "final"
        ]
        checks = {
            "lower_combined_loss": metrics["combined"]["mean_loss_bb"]
            < parent["combined"]["mean_loss_bb"],
            "hand_reading": metrics["hand"]["correct"] >= parent["hand"]["correct"],
            "bluff_pricing": metrics["price"]["pairs_correct"]
            >= parent["price"]["pairs_correct"],
            "retained_good_bluffs": history["good_bluff_bet_rate"]
            >= parent_history["good_bluff_bet_rate"] - 0.05,
            "retained_value_bets": history["value_bet_rate"]
            >= parent_history["value_bet_rate"] - 0.05,
        }
        with (folder / str(seed) / "final.gguf").open("rb") as handle:
            digest = hashlib.file_digest(handle, "sha256").hexdigest()
        arms[seed] = {
            "metrics": metrics,
            "exploratory_consistency": separate_and_combined(rows),
            "retention": history,
            "guardrails": checks,
            "paired_savings_95pct": interval,
            "examples": sum(r["num_sequences"] for r in logs),
            "updates": len(logs),
            "estimated_training_cost": logs[-1]["cost/cumulative_usd"],
            "trained_tokens": logs[-1]["tokens/cumulative/trained"],
            "checkpoint": final,
            "gguf_sha256": digest,
            "conversion": json.loads(
                (folder / str(seed) / "export/conversion-verification.json").read_text()
            ),
        }
    ordered = sorted(
        arms, key=lambda seed: (arms[seed]["metrics"]["combined"]["mean_loss_bb"], seed)
    )
    median_loss = arms[ordered[1]]["metrics"]["combined"]["mean_loss_bb"]
    chosen = next(
        seed
        for seed in SEEDS
        if arms[seed]["metrics"]["combined"]["mean_loss_bb"] == median_loss
    )
    return {
        "representative": chosen,
        "advance_to_release_evaluation": all(arms[chosen]["guardrails"].values())
        and arms[chosen]["paired_savings_95pct"][0] > 0,
        "replicates": arms,
        "parent": parent,
        "parent_exploratory_consistency": separate_and_combined(baseline),
        "parent_retention": parent_history,
        "estimated_cost": sum(arm["estimated_training_cost"] for arm in arms.values()),
    }


if __name__ == "__main__":
    (OUT / "results.json").write_text(json.dumps(summarize(), indent=2) + "\n")
