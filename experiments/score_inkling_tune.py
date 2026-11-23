"""Score saved river responses offline. Input JSONL: group, format, answer."""

import argparse
from collections import defaultdict
import json
from pathlib import Path
from statistics import mean

from inkling_overnight import atomic, loss
from prepare_inkling_tune import prompt_key, rows


def score(questions, predictions):
    answers = {}
    for r in predictions:
        key = r["group"], r["format"]
        if key in answers:
            raise ValueError(f"Duplicate prediction {key}")
        answers[key] = r["answer"]
    visible = {
        r["group"]: json.loads(r["messages"][1]["content"])
        for r in questions
        if r["format"] == "plain"
    }
    results = {}
    for fmt in ("plain", "table", "cards_only", "hand_read"):
        errors = 0
        cost, categories, pairs, card_answers = (
            [],
            defaultdict(list),
            defaultdict(dict),
            {},
        )
        for r in questions:
            if r["format"] != fmt:
                continue
            answer = answers.pop((r["group"], fmt))
            if fmt == "hand_read":
                errors += answer not in ("win", "lose")
                # Repeated card questions count once. Average repeats if the serving
                # runtime produced different answers to the identical prompt.
                card_answers.setdefault(
                    (r["target"], prompt_key(r["messages"])), []
                ).append(answer == r["target"])
                continue
            if answer not in ("check", "bet"):
                errors += 1
                answer = "check"
            v = visible[r["group"]]
            win = v["if_called"] == "you win"
            cost.append(
                loss(answer, v["fold_percent"] / 100, win, v["pot_bb"], v["bet_bb"])
            )
            category = (
                "value"
                if win
                else ("good_bluff" if r["target"] == "bet" else "bad_bluff")
            )
            categories[category].append(answer == "bet")
            if not win:
                pairs[v["pot_bb"], v["bet_bb"]][r["target"]] = answer
        metrics = {"invalid": errors}
        if fmt == "hand_read":
            for label in ("win", "lose"):
                hits = [
                    mean(v)
                    for (target, _), v in card_answers.items()
                    if target == label
                ]
                metrics[label + "_accuracy"] = mean(hits)
                metrics[label + "_unique"] = len(hits)
        else:
            metrics["mean_loss_bb"] = mean(cost)
            metrics.update({k + "_bet_rate": mean(v) for k, v in categories.items()})
            metrics["pairs_correct"] = sum(
                v == {"check": "check", "bet": "bet"} for v in pairs.values()
            )
            metrics["pairs_total"] = len(pairs)
        results[fmt] = metrics
    if answers:
        raise ValueError("Predictions include questions outside the evaluation set")
    bets = [results[f] for f in ("plain", "table", "cards_only")]
    results["overall_betting"] = {
        metric: mean(r[metric] for r in bets)
        for metric in (
            "mean_loss_bb",
            "good_bluff_bet_rate",
            "value_bet_rate",
            "bad_bluff_bet_rate",
        )
    }
    return results


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("questions", type=Path)
    p.add_argument("predictions", type=Path)
    p.add_argument("output", type=Path)
    args = p.parse_args()
    atomic(args.output, score(rows(args.questions), rows(args.predictions)))
