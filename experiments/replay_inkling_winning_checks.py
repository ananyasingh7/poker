"""Audit saved Session 003 decisions. No model or network calls."""

import json
from collections import Counter

from inkling_fresh_diagnosis import diagnose, save
from inkling_overnight import loss
from inkling_session003_revision import FIXED, OUT, SEEDS
from score_inkling_session003 import read


def audit(path):
    ledger = diagnose(read(path))
    if any(row["category"] == "invalid" for row in ledger):
        raise ValueError(f"Incomplete diagnostic answers in {path}")
    misses = [r for r in ledger if r["hand_target"] == "win" and r["answer"] == "check"]
    category_costs = Counter()
    for row in misses:
        category_costs[row["category"]] += row["loss_bb"]

    # Counterfactual uses only the model's own read to change its action.
    # Include losing hands as well so mistaken 'win' answers can hurt the policy.
    replay_loss = 0
    newly_costly_bets = 0
    for row in ledger:
        view = row["view"]
        action = "bet" if row["hand_answer"] == "win" else row["answer"]
        cost = loss(
            action,
            view["estimated_opponent_fold_percent"] / 100,
            row["hand_target"] == "win",
            view["pot_bb"],
            view["bet_bb"],
        )
        replay_loss += cost
        newly_costly_bets += cost > row["loss_bb"]
    return misses, {
        "winning_opportunities": sum(r["hand_target"] == "win" for r in ledger),
        "missed_decisions": len(misses),
        "distinct_missed_deals": len({r["group"] for r in misses}),
        "miss_counts": Counter(r["category"] for r in misses),
        "miss_cost_bb": category_costs,
        "original_mean_loss_bb": sum(r["loss_bb"] for r in ledger) / len(ledger),
        "win_read_override_mean_loss_bb": replay_loss / len(ledger),
        "newly_costly_bets": newly_costly_bets,
    }


def main():
    rep = json.loads((OUT / "results.json").read_text())["representative"]
    paths = [
        FIXED / "step140.jsonl.gz",
        *[OUT / f"{s}.jsonl.gz" for s in SEEDS],
        OUT / f"fresh-control-{rep}.jsonl.gz",
    ]
    summary = {}
    cases = []
    for path in paths:
        arm = path.name.removesuffix(".jsonl.gz")
        misses, summary[arm] = audit(path)
        cases.extend(
            {"arm": arm, **row} for row in sorted(misses, key=lambda r: -r["loss_bb"])
        )
    save("missed-winning-bets", cases, OUT)
    (OUT / "winning-checks.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
