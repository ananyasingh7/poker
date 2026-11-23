"""Make labeled river examples, or score a local model on a saved split."""

import argparse
import json
import random
from collections import Counter
from pathlib import Path
from urllib.request import ProxyHandler, Request, build_opener

from inkling_overnight import RULES, cards, payoff

SYSTEM = (
    "Heads-up river. Your opponent checked. You can check or bet the listed amount. "
    "If you bet, your opponent either calls or folds. The fold chance is supplied. "
    "Choose the action with the highest expected chips. Reply with JSON containing "
    "action, either check or bet."
)
SPLITS = {"train": 160, "dev": 12, "test": 40}
RATIOS = [0.25, 0.5, 1, 2]


def generate(folder):
    rng = random.Random(1300001)
    folder.mkdir(parents=True, exist_ok=False)
    # Each pot/size combination belongs to a single split. A group includes both
    # sides of the bluff threshold and a value hand at the lower fold chance.
    pots = list(range(4, 216))
    rng.shuffle(pots)
    for split, count in SPLITS.items():
        rows = []
        for pot in pots[:count]:
            for ratio in RATIOS:
                bet = pot * ratio
                threshold = 100 * bet / (pot + bet)
                margin = rng.choice([2, 5, 10, 15])
                for strong, offset in [(False, -margin), (False, margin), (True, -margin)]:
                    pct = round(threshold + offset, 1)
                    view = {"pot_bb": pot, "bet_bb": bet, "fold_percent": pct,
                            "if_called": "you win" if strong else "you lose"}
                    values = {a: payoff(a, pct / 100, strong, pot, bet) for a in ("check", "bet")}
                    best = max(values, key=values.get)
                    messages = [{"role": "system", "content": SYSTEM},
                                {"role": "user", "content": json.dumps(view)}]
                    rows.append((messages, best, values))
        pots = pots[count:]
        rng.shuffle(rows)
        with (folder / f"{split}.jsonl").open("x") as output:
            for messages, best, values in rows:
                if split == "train":
                    row = {"messages": messages + [{"role": "assistant", "content": json.dumps({"action": best})}]}
                else:
                    row = {"messages": messages, "target": best, "values_bb": values}
                output.write(json.dumps(row) + "\n")
        print(split, len(rows))


def score(path, port, model, table_format):
    http = build_opener(ProxyHandler({}))
    counts = Counter()
    losses = []
    for line in path.read_text().splitlines():
        row = json.loads(line)
        view = json.loads(row["messages"][1]["content"])
        category = ("value bet" if view["if_called"] == "you win" else
                    "good bluff" if row["target"] == "bet" else "bad bluff")
        messages = row["messages"]
        if table_format:
            strong = view["if_called"] == "you win"
            deal = cards(random.Random(f"{view['pot_bb']}/{view['bet_bb']}/{strong}"), strong)
            observation = {**deal, "pot_bb": view["pot_bb"], "bet_bb": view["bet_bb"],
                           "estimated_opponent_fold_percent": view["fold_percent"],
                           "your_hand": "three of a kind; beats Seat 1" if strong else "unpaired high card; loses to Seat 1"}
            messages = [{"role": "system", "content": RULES},
                        {"role": "user", "content": json.dumps(observation, sort_keys=True, separators=(",", ":"))}]
        body = {"model": model, "temperature": 0, "max_tokens": 64,
                "chat_template_kwargs": {"reasoning_effort": "none"},
                "messages": messages + [{"role": "assistant", "content": "{"}],
                "response_format": {"type": "json_schema", "json_schema": {"name": "action", "strict": True,
                    "schema": {"type": "object", "properties": {"action": {"enum": ["check", "bet"], "type": "string"}},
                               "required": ["action"], "additionalProperties": False}}}}
        request = Request(f"http://127.0.0.1:{port}/v1/chat/completions", json.dumps(body).encode(),
                          {"Content-Type": "application/json"})
        with http.open(request, timeout=180) as response:
            reply = json.load(response)
        if reply["model"] != model:
            raise ValueError(f"Unexpected model: {reply['model']}")
        choice = reply["choices"][0]
        try:
            action = json.loads(choice["message"]["content"])["action"]
            if choice["finish_reason"] != "stop" or action not in ("check", "bet"):
                raise ValueError("Invalid action")
        except (KeyError, TypeError, ValueError):
            counts["fallback_checks"] += 1
            action = "check"
        counts[category + " opportunities"] += 1
        counts[category + " bets"] += int(action == "bet")
        counts["correct"] += int(action == row["target"])
        losses.append(row["values_bb"][row["target"]] - row["values_bb"][action])
    print(json.dumps({"model": model, "split": path.stem, "format": "table" if table_format else "simple", "questions": len(losses),
                      "mean_bb_lost_per_decision": sum(losses) / len(losses), **counts}, indent=2))


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    commands = cli.add_subparsers(dest="command", required=True)
    commands.add_parser("generate").add_argument("folder", type=Path)
    evaluate = commands.add_parser("score")
    evaluate.add_argument("split", type=Path)
    evaluate.add_argument("--port", type=int, default=9932)
    evaluate.add_argument("--model", default="inkling-2-mini")
    evaluate.add_argument("--table-format", action="store_true", help="Ask the same decisions with cards and the overnight prompt")
    args = cli.parse_args()
    if args.command == "generate":
        generate(args.folder)
    else:
        score(args.split, args.port, args.model, args.table_format)
