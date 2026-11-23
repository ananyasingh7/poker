"""Compare historical reads with a Bayesian observer of the same abrupt-game evidence."""

import json
from collections import defaultdict
from pathlib import Path
from statistics import mean

from inkling_own_choice import count_estimate


def compare(path):
    games = defaultdict(list)
    for line in path.read_text().splitlines():
        row = json.loads(line)
        if row["module"] == "abrupt" and row["arm"] == "model":
            games[row["seed"]].append(row)
    scores = defaultdict(list)
    for hands in games.values():
        # Uniform prior over the four protocol conditions and possible switch markers.
        hypotheses = [
            (before, after, switch)
            for before in (0.2, 0.8)
            for after in (0.2, 0.8)
            for switch in range(42, 59)
        ]
        weights = [1 / len(hypotheses)] * len(hypotheses)
        observations = []
        for row in sorted(hands, key=lambda item: item["hand"]):
            rates = [
                before if row["hand"] <= marker else after
                for before, after, marker in hypotheses
            ]
            posterior_rate = sum(weight * rate for weight, rate in zip(weights, rates))
            estimates = {
                "model": row["estimate"] / 100,
                "recent20": count_estimate(observations, 20) / 100,
                "bayes": posterior_rate,
            }
            for name, estimate in estimates.items():
                scores[name].append(abs(estimate - row["p"]) * 100)
            response = row["observed"]["opponent_response"]
            observations.append(row["observed"])
            if response in ("call", "fold"):
                weights = [
                    weight * (rate if response == "fold" else 1 - rate)
                    for weight, rate in zip(weights, rates)
                ]
                total = sum(weights)
                weights = [weight / total for weight in weights]
    return {
        "games": len(games),
        "hands": len(scores["model"]),
        "mean_absolute_error_percentage_points": {
            name: mean(errors) for name, errors in scores.items()
        },
    }


if __name__ == "__main__":
    print(json.dumps(compare(Path("runs/inkling-sleep-v1/hand.jsonl")), indent=2))
