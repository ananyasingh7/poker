"""Summarize paired development diagnostics, preserving independent game units."""

import json
from statistics import mean
from inkling_transfer_audit import OUTPUT


def summarize(rows):
    scores = {}
    for arm in ("original", "step140"):
        subset = [r for r in rows if r["arm"] == arm]
        prices = [r for r in subset if r["kind"] == "price"]
        cases = {r["group"] for r in prices}

        def pair_correct(rate):
            pair = [r for r in prices if r["group"] == rate]
            assert len(pair) == 2 and {r["target"] for r in pair} == {"bet", "check"}
            return all(r["answer"] == r["target"] for r in pair)

        hands = [r for r in subset if r["kind"] == "hand"]
        games = [r for r in subset if r["kind"] == "game"]
        replays = [r for r in subset if r["kind"] == "replay"]
        scores[arm] = dict(
            at_30=[r["answer"] for r in prices if r["group"] == 30],
            novel_price_pairs_correct=sum(
                pair_correct(rate) for rate in cases if rate != 30
            ),
            novel_pairs=len(cases) - 1,
            hand_errors=[r["group"] for r in hands if r["answer"] != r["target"]],
            correct_replay_checks=sum(r["answer"] == "check" for r in replays),
            replays=len(replays),
            invalid=sum(r["answer"] is None for r in subset),
            game_loss_by_seed={
                seed: mean(r["loss_bb"] for r in games if r["group"] == seed)
                for seed in sorted({r["group"] for r in games})
            },
            game_mean_loss=mean(r["loss_bb"] for r in games),
            game_same_read_calculator_loss=mean(r["calculator_loss_bb"] for r in games),
        )
    return scores


if __name__ == "__main__":
    scores = summarize(json.loads((OUTPUT / "predictions.json").read_text()))
    print(json.dumps(scores, indent=2))
