"""Matched local development diagnostics before Session 002. Execute from the repo root."""

import json
import random
from pathlib import Path
from urllib.request import Request, urlopen

from inkling_overnight import RULES, best_action, canonical, cards, loss
from inkling_own_choice import count_estimate
from prepare_inkling_tune import hand_wins

OUTPUT = Path("results/15-inkling-audit")
URL = "http://127.0.0.1:9933"
BOARD = ["Ac", "8h", "6d", "3s", "2c"]
GAME_SEEDS = range(1600200, 1600204)


def price_questions():
    # Fixed 30% caller plus ten held-out rates, each with a different pair of prices.
    rates = [30, 17, 21, 27, 32, 37, 44, 49, 54, 59, 67]
    for index, rate in enumerate(rates):
        pot = 10 if index == 0 else 40 + 3 * index
        fair = pot * rate / (100 - rate)
        bets = (
            [2.5, 10]
            if index == 0
            else [
                round(fair * (0.65 + 0.025 * index), 2),
                round(fair * (1.04 + 0.035 * index), 2),
            ]
        )
        for bet in bets:
            view = dict(
                board=BOARD,
                hole=["Kh", "9d"],
                pot_bb=pot,
                bet_bb=bet,
                estimated_opponent_fold_percent=rate,
            )
            yield dict(
                kind="price",
                group=rate,
                view=view,
                system=RULES,
                target=best_action(rate / 100, False, pot, bet),
            )


def hand_questions():
    cases = [
        ("bottom pair", BOARD, ["2d", "Kd"]),
        ("middle pair", BOARD, ["8s", "Qd"]),
        ("straight", BOARD, ["4d", "5s"]),
        ("overpair", ["9s", "8h", "6c", "4s", "2d"], ["Qh", "Qc"]),
        ("flush", ["Kc", "8c", "6c", "4h", "2s"], ["Qc", "Tc"]),
        ("two pair", BOARD, ["8s", "3h"]),
        ("unpaired", BOARD, ["Kh", "9d"]),
        ("set", BOARD, ["6h", "6s"]),
    ]
    for name, board, hole in cases:
        view = dict(
            board=board,
            hole=hole,
            opponent_hand="exactly one pair of the board's highest rank",
        )
        yield dict(
            kind="hand",
            group=name,
            view=view,
            system="Compare your river hand with the described opponent. Return action as win or lose if called.",
            target="win" if hand_wins(view) else "lose",
        )


def replay_questions():
    seen = set()
    path = Path("runs/inkling-sleep-v1/hand.jsonl")
    for line in path.open():
        row = json.loads(line)
        if (
            row["arm"] == "model"
            and row["module"] == "abrupt"
            and row["condition"] == "folder_to_caller"
            and row["hand"] > row["switch"] + 40
            and not row["strong"]
            and row["estimate"] < 50
            and row["seed"] not in seen
        ):
            seen.add(row["seed"])
            yield dict(
                kind="replay",
                group=row["seed"],
                hand=row["hand"],
                view=row["visible"],
                system=RULES,
                target="check",
                recorded=row["action"],
            )


def decide(question, scale):
    options = ["win", "lose"] if question["kind"] == "hand" else ["bet", "check"]
    schema = dict(
        type="object",
        properties={"action": dict(type="string", enum=options)},
        required=["action"],
        additionalProperties=False,
    )
    messages = [
        dict(role="system", content=question["system"]),
        dict(role="user", content=canonical(question["view"])),
        dict(role="assistant", content="{"),
    ]
    payload = dict(
        model="inkling-poker-001",
        messages=messages,
        temperature=0,
        seed=14001,
        max_tokens=64,
        lora=[dict(id=0, scale=scale)],
        response_format=dict(
            type="json_schema",
            json_schema=dict(name="action", strict=True, schema=schema),
        ),
    )
    req = Request(
        URL + "/v1/chat/completions",
        json.dumps(payload).encode(),
        {"Content-Type": "application/json"},
    )
    choice = json.load(urlopen(req, timeout=120))["choices"][0]
    try:
        action = json.loads(choice["message"]["content"])["action"]
    except (ValueError, KeyError, TypeError):
        action = None
    valid = action in options and choice["finish_reason"] == "stop"
    return dict(answer=action if valid else None, raw=choice["message"]["content"])


def games(scale):
    for seed in GAME_SEEDS:
        rng = random.Random(seed)
        history = []
        for hand in range(16):
            # Fixed exogenous tape, independent of each player's decisions.
            strong = rng.random() < 0.35
            deal = cards(rng, strong)
            ratio = rng.choice([0.25, 0.6, 1, 1.4])
            draw = rng.random()
            fold_rate = (
                (0.2 if hand < 8 else 0.8) if seed % 2 else (0.8 if hand < 8 else 0.2)
            )
            estimate = count_estimate(history, 6)
            bet = 10 * ratio
            view = dict(
                **deal,
                pot_bb=10,
                bet_bb=bet,
                estimated_opponent_fold_percent=round(estimate, 1),
            )
            question = dict(
                kind="game",
                group=seed,
                hand=hand + 1,
                view=view,
                system=RULES,
                target=best_action(fold_rate, strong, 10, bet),
            )
            prediction = decide(question, scale)
            action = prediction["answer"] or "check"
            response = (
                ("fold" if draw < fold_rate else "call") if action == "bet" else None
            )
            reference = best_action(
                view["estimated_opponent_fold_percent"] / 100, strong, 10, bet
            )
            history.append(dict(your_action=action, opponent_response=response))
            yield dict(
                **question,
                **prediction,
                fold_rate=fold_rate,
                strong=strong,
                response=response,
                loss_bb=loss(action, fold_rate, strong, 10, bet),
                calculator_loss_bb=loss(reference, fold_rate, strong, 10, bet),
            )


def main():
    adapters = json.load(urlopen(URL + "/lora-adapters"))
    expected = Path(
        "runs/inkling-tuning/001-river/selected-adapter/poker.gguf"
    ).resolve()
    if (
        len(adapters) != 1
        or Path(adapters[0]["path"]).resolve() != expected
        or adapters[0]["id"] != 0
    ):
        raise RuntimeError("Start the Session 001 launcher before evaluating")
    questions = [*price_questions(), *hand_questions(), *replay_questions()]
    rows = []
    for arm, scale in [("original", 0), ("step140", 1)]:
        for question in questions:
            rows.append(dict(arm=arm, **question, **decide(question, scale)))
        rows.extend(dict(arm=arm, **result) for result in games(scale))
    path = OUTPUT / "predictions.json"
    temp = path.with_suffix(".partial")
    temp.write_text(json.dumps(rows, indent=2) + "\n")
    temp.replace(path)


if __name__ == "__main__":
    main()
