"""Build the frozen, variable-price release challenge. No model requests."""

import gzip
import hashlib
import json
import random
from fractions import Fraction
from pathlib import Path

from inkling_overnight import RULES, canonical, best_action
from inkling_split_audit import pricing_cases, pricing_key, require_novel
from prepare_inkling_tune import hand_wins

SEED = 1631031


def sample_deal(rng, wins, shared_rank):
    deck = [rank + suit for rank in "23456789TJQKA" for suit in "cdhs"]
    while True:
        drawn = rng.sample(deck, 7)
        board, hole = drawn[:5], drawn[5:]
        ranks = {card[0] for card in board}
        # An unpaired board and at most three cards per suit allow exactly top pair.
        if (
            len(ranks) != 5
            or max(sum(card[1] == s for card in board) for s in "cdhs") > 3
        ):
            continue
        if bool(ranks & {card[0] for card in hole}) != shared_rank:
            continue
        view = dict(board=board, hole=hole)
        try:
            if hand_wins(view) == wins:
                # A straight on the board would make the opponent description impossible.
                values = {"23456789TJQKA".index(rank) + 2 for rank in ranks}
                if 14 in values:
                    values.add(1)
                if not any(set(range(low, low + 5)) <= values for low in range(1, 11)):
                    return view
        except ValueError:
            continue  # Top-pair kicker is unspecified.


def generate(reference):
    rng = random.Random(SEED)
    questions = []
    used_rates = set()
    reference_rates = {key[1] for key in reference}
    # Exclude development probes, including the original four size-switch rates.
    reference_rates.update(
        Fraction(str(rate))
        for rate in (26.4, 41.2, 53.1, 62.4, 17, 21, 27, 32, 37, 44, 49, 54, 59, 67)
    )
    while len(used_rates) < 96:
        percent = rng.randint(115, 735) / 10
        if percent in used_rates or Fraction(str(percent)) in reference_rates:
            continue
        pot = rng.randint(120, 1800) / 10
        fair = pot * percent / (100 - percent)
        prices = [
            round(fair * rng.uniform(0.60, 0.985), 2),
            round(fair * rng.uniform(1.015, 1.4), 2),
        ]
        shared_rank = len(used_rates) % 2 == 0
        weak = sample_deal(rng, False, shared_rank)
        strong = sample_deal(rng, True, shared_rank)
        used_rates.add(percent)
        for deal, bets in [(weak, prices), (strong, [rng.choice(prices)])]:
            for bet in bets:
                view = dict(
                    **deal,
                    pot_bb=pot,
                    bet_bb=bet,
                    estimated_opponent_fold_percent=percent,
                )
                target = best_action(percent / 100, hand_wins(view), pot, bet)
                questions.append(
                    dict(
                        group=len(used_rates),
                        format="cards_only",
                        target=target,
                        messages=[
                            dict(role="system", content=RULES),
                            dict(role="user", content=canonical(view)),
                        ],
                    )
                )
    keys = {pricing_key(json.loads(row["messages"][1]["content"])) for row in questions}
    require_novel(reference, keys)
    rng.shuffle(questions)
    return questions


if __name__ == "__main__":
    folder = Path("results/15-inkling-audit")
    reference = pricing_cases(Path("results/14-inkling-finetuning/train.jsonl.gz"))
    rows = generate(reference)
    data = "".join(canonical(row) + "\n" for row in rows).encode()
    (folder / "size-challenge.jsonl.gz").write_bytes(gzip.compress(data, mtime=0))
    print(len(rows), hashlib.sha256(data).hexdigest())
