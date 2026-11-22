"""Exploratory probability/payoff controls. Run after read_then_bet.py starts."""
import itertools
import json
from pathlib import Path
import random
import sys

import read_then_bet as experiment


def run(directory):
    directory.mkdir(parents=True, exist_ok=False)
    design = {"exploratory": True,
              "trigger": "The first three main checkpoints ignored numeric 80% fold rates.",
              "seed": 93101, "fold_probabilities": [.2, .8],
              "variants": ["numeric", "plain_language", "calculated_payoffs"],
              "probes": experiment.PROBES,
              "rules": experiment.RULES,
              "payoff_units": "Net bb, including the five bb already contributed."}
    (directory / "design.json").write_text(json.dumps(design, indent=2) + "\n")
    rows = []
    suits = random.Random(93101).sample("cdhs", 4)
    experiment.cache = {}
    with (directory / "trace.jsonl").open("x") as experiment.model.trace:
        for probability in (.2, .8):
            for template, strong in itertools.product(experiment.PROBES, (False, True)):
                cards = experiment.model.deal(template, strong, suits)
                contexts = {
                    "numeric": {"estimated_opponent_fold_percent": int(probability * 100)},
                    "plain_language": {"opponent_information":
                                       f"Seat 1 folds to {int(probability * 100)} percent of pot-sized river bets. "
                                       "This is their current policy and is independent of your cards."},
                    "calculated_payoffs": {
                        "estimated_opponent_fold_percent": int(probability * 100),
                        "expected_net_bb": {action: round(experiment.expected_value(action, probability, strong), 5)
                                            for action in ("check", "bet")}},
                }
                for variant, evidence in contexts.items():
                    action = experiment.choose(cards, strong, evidence)
                    rows.append({"true_fold_percent": int(probability * 100), "cards": cards,
                                 "strong": strong, "variant": variant, "evidence": evidence,
                                 "action": action, "loss_bb": experiment.score(action, probability, strong)})
                (directory / "results.json").write_text(json.dumps(rows, indent=2) + "\n")
                print(probability, strong, [r['action'] for r in rows[-3:]], flush=True)
    return rows


def run_read_controls(directory, source):
    directory.mkdir(parents=True, exist_ok=False)
    tapes = [json.loads(line) for line in (source / "trace.jsonl").read_text().splitlines()]
    hands = [hand for row in tapes if row["kind"] == "tape" and row["seed"] == 93103
             for hand in row["hands"]]
    assert len(hands) == 24
    folds = sum(h["opponent_response"] == "fold" for h in hands)
    counts = {"bets_faced": len(hands), "folds": folds, "calls": len(hands) - folds}
    cases = {"raw_history": {"completed_hands": hands}, "counts_only": counts,
             "counts_and_rate": {**counts, "observed_fold_percent": 100 * folds / len(hands)},
             "mirror_counts": {"bets_faced": len(hands), "folds": counts["calls"], "calls": folds}}
    design = {"exploratory": True, "source": str(source), "seed": 93103,
              "trigger": "Qwen estimated 80 percent folds from 22 calls and 2 folds.",
              "read_prompt": experiment.READ, "cases": cases}
    (directory / "design.json").write_text(json.dumps(design, indent=2) + "\n")
    experiment.cache = {}
    rows = []
    with (directory / "trace.jsonl").open("x") as experiment.model.trace:
        for variant, evidence in cases.items():
            rows.append({"variant": variant, "evidence": evidence,
                         "fold_percent": experiment.predict(evidence)})
    (directory / "results.json").write_text(json.dumps(rows, indent=2) + "\n")
    print([(r["variant"], r["fold_percent"]) for r in rows], flush=True)


def run_prompt_controls(directory, source):
    directory.mkdir(parents=True, exist_ok=False)
    original = json.loads((source / "design.json").read_text())
    prompt = experiment.READ.replace(
        "A probability of 80 means 80 percent of such bets get folded. ",
        "Report a percentage from zero to one hundred. ")
    cases = {key: original["cases"][key] for key in ("counts_only", "counts_and_rate", "mirror_counts")}
    design = {"exploratory": True, "trigger": "Check the influence of the example percentage.",
              "prompt": prompt, "cases": cases}
    (directory / "design.json").write_text(json.dumps(design, indent=2) + "\n")
    experiment.cache = {}
    rows = []
    with (directory / "trace.jsonl").open("x") as experiment.model.trace:
        for variant, evidence in cases.items():
            answer = experiment.ask(prompt, evidence, "fold_percent", [str(i) for i in range(0, 101, 5)])
            rows.append({"variant": variant, "evidence": evidence, "fold_percent": int(answer)})
    (directory / "results.json").write_text(json.dumps(rows, indent=2) + "\n")
    print([(row["variant"], row["fold_percent"]) for row in rows], flush=True)


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[3] == "--no-example":
        run_prompt_controls(Path(sys.argv[1]), Path(sys.argv[2]))
    elif len(sys.argv) == 3:
        run_read_controls(Path(sys.argv[1]), Path(sys.argv[2]))
    else:
        run(Path(sys.argv[1]))
