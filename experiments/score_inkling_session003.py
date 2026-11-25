"""Export each completed Session 003 adapter and apply the frozen development criteria."""

import gzip
import json
import random
from statistics import mean, median

from evaluate_inkling_session002 import export, final_checkpoint, local_server
from inkling_fresh_diagnosis import RESULT, diagnose, evaluate, save
from inkling_session002 import OUT as PREVIOUS
from inkling_session002 import score
from inkling_session003 import OUT, RUN, SEEDS
from inkling_transfer_audit import decide
from score_inkling_tune import score as score_controls


def read(path):
    with gzip.open(path, "rt") as handle:
        return list(map(json.loads, handle))


def decision_costs(rows):
    ledger = diagnose(rows)
    groups = {}
    for row in ledger:
        groups.setdefault(row["group"], []).append(row["loss_bb"])
    if len(groups) != 32 or any(len(values) != 4 for values in groups.values()):
        raise ValueError("Expected four decisions in each of 32 groups")
    return {group: mean(values) for group, values in groups.items()}, mean(
        r["loss_bb"] for r in ledger if r["hand_target"] == "win"
    )


def summarize(parent, candidate):
    before, parent_value = decision_costs(parent)
    after, value = decision_costs(candidate)
    savings = [before[g] - after[g] for g in sorted(before)]
    rng = random.Random(183000)
    draws = sorted(mean(rng.choices(savings, k=len(savings))) for _ in range(10000))
    return {
        "scores": score(candidate),
        "winner_loss_bb": value,
        "parent_winner_loss_bb": parent_value,
        "savings_bb": mean(savings),
        "savings_95_bb": [draws[249], draws[9749]],
    }


def always_bet(rows):
    # The policy only answers betting questions; score the full-decision format.
    combined = [dict(row, answer="bet") for row in rows if row["kind"] == "combined"]
    return score(
        [dict(row, answer="bet") for row in rows if row["kind"] == "price"] + combined
    )["combined"]


def main(out=OUT, run=RUN, fixed=OUT, compare_always_bet=False, wait=False):
    dev = read(fixed / "dev.jsonl.gz")
    parent = read(fixed / "step140.jsonl.gz")
    parent_scores = score(parent)
    reps = {}
    for seed in SEEDS:
        folder = run / str(seed)
        adapter = export(folder, final_checkpoint(folder, wait))
        candidates = evaluate(str(seed), adapter, dev, run / "evaluation", out)
        reps[seed] = summarize(parent, candidates)
    middle = median(rep["scores"]["combined"]["mean_loss_bb"] for rep in reps.values())
    chosen = next(
        s for s in SEEDS if reps[s]["scores"]["combined"]["mean_loss_bb"] == middle
    )
    rep = reps[chosen]
    adapter = run / str(chosen) / "final.gguf"
    old = evaluate(
        f"fresh-control-{chosen}",
        adapter,
        read(RESULT / "questions.jsonl.gz"),
        run / "evaluation",
        out,
    )
    old_scores = score(old)["combined"]
    previous = json.loads((RESULT / "scores.json").read_text())
    controls = read(PREVIOUS / "retention-questions.jsonl.gz")
    predictions = []
    with local_server(run / str(chosen), adapter):
        for row in controls:
            system, user = row["messages"]
            answer = decide(
                {
                    "kind": "hand" if row["format"] == "hand_read" else "combined",
                    "system": system["content"],
                    "view": json.loads(user["content"]),
                },
                1,
            )
            predictions.append(dict(group=row["group"], format=row["format"], **answer))
    save(f"historical-control-{chosen}", predictions, out)
    historical = score_controls(controls, predictions)["overall_betting"]
    checks = {
        "lower_winner_loss": rep["winner_loss_bb"] < rep["parent_winner_loss_bb"],
        "positive_paired_savings": rep["savings_95_bb"][0] > 0,
        "hand_reading": rep["scores"]["hand"]["correct"]
        >= parent_scores["hand"]["correct"],
        "bluff_pairs": rep["scores"]["combined"]["pairs_correct"]
        >= parent_scores["combined"]["pairs_correct"],
        "earlier_bluff_pairs": old_scores["pairs_correct"]
        >= previous["1622101"]["metrics"]["combined"]["pairs_correct"],
        "earlier_loss": old_scores["mean_loss_bb"]
        <= previous["step140"]["metrics"]["combined"]["mean_loss_bb"],
        "historical_good_bluffs": historical["good_bluff_bet_rate"] >= 0.95,
        "historical_value": historical["value_bet_rate"] >= 0.95,
    }
    if compare_always_bet:
        benchmark = always_bet(dev)
        for seed in SEEDS:
            reps[seed]["always_bet_loss_bb"] = benchmark["mean_loss_bb"]
            reps[seed]["savings_vs_always_bet_bb"] = (
                benchmark["mean_loss_bb"]
                - reps[seed]["scores"]["combined"]["mean_loss_bb"]
            )
        checks["beats_always_bet"] = rep["savings_vs_always_bet_bb"] > 0
    report = {
        "replicates": reps,
        "representative": chosen,
        "guardrails": checks,
        "earlier_river": old_scores,
        "historical": historical,
        "advance_to_release_evaluation": all(checks.values()),
    }
    (out / "results.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
