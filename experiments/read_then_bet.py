"""Controlled local pilot. python3 -B experiments/read_then_bet.py NEW_RUN_DIR"""
import itertools
import json
from pathlib import Path
import random
import sys

import river_switch as model

GAMES = [(93101, .2, .8), (93102, .8, .2), (93103, .2, .2), (93104, .8, .8)]
TRAIN = [("AT852", "Q4", "88"), ("KT632", "J9", "66"),
         ("QT742", "J3", "77"), ("JT952", "Q3", "99")]
PROBES = [("A9632", "Q4", "66"), ("KT852", "Q3", "88")]
RULES = model.RULES.replace("Your private scouting notes contain earlier observations.", "")
READ = ("Estimate the probability that Seat 1 folds to the next pot-sized river bet, "
        "using only the supplied observations. Every listed bet gave Seat 1 a choice of call "
        "or fold. A probability of 80 means 80 percent of such bets get folded. Give your "
        "best estimate of current behavior. Return only the requested JSON object.")
UPDATES = {
    "observations": "Record observed calls and folds by Seat 1, with counts and when they occurred. "
                    "Keep observations only. Write at most 80 words.",
    "advice": "Record observed calls and folds by Seat 1, with counts and when they occurred. "
              "Recommend how to play against Seat 1. Write at most 80 words.",
}


def expected_value(action, fold_probability, strong):
    if action == "check":
        return 5.0 if strong else -5.0
    called = 15.0 if strong else -15.0
    return fold_probability * 5.0 + (1 - fold_probability) * called


def score(action, probability, strong):
    best = max(expected_value(a, probability, strong) for a in ("check", "bet"))
    return best - expected_value(action, probability, strong)


def ask(system, observation, field, options=None):
    key = json.dumps([system, observation, field, options], sort_keys=True)
    if key not in cache:
        cache[key] = model.ask(system, observation, field, options)
    return cache[key]


def predict(evidence):
    return int(ask(READ, evidence, "fold_percent", [str(i) for i in range(0, 101, 5)]))


def choose(cards, strong, evidence):
    observation = {**cards, "your_hand": "three of a kind; beats Seat 1" if strong
                   else "unpaired high card; loses to Seat 1", **evidence}
    return ask(RULES, observation, "action", ["check", "bet"])


def evaluate(probes, probability, notes, estimates):
    scores = {}
    evidence = {f"direct_{arm}": {"private_notebook": text} for arm, text in notes.items()}
    evidence.update({f"read_{arm}": {"estimated_opponent_fold_percent": percent}
                     for arm, percent in estimates.items()})
    evidence["oracle"] = {"estimated_opponent_fold_percent": int(probability * 100)}
    evidence["blank"] = {}
    for arm, context in evidence.items():
        actions = [choose(cards, strong, context) for cards, strong in probes]
        losses = [score(a, probability, strong) for a, (_, strong) in zip(actions, probes, strict=True)]
        scores[arm] = {"actions": actions, "mean_loss_bb": sum(losses) / len(losses)}
    return scores


def run(directory):
    global cache
    cache = {}
    directory.mkdir(parents=True, exist_ok=False)
    design = {"model": model.MODEL, "endpoint": model.URL, "temperature": 0,
              "json_prefill": model.JSON_PREFILL, "thinking": model.TEMPLATE_KWARGS.get("enable_thinking"), "chat_template_kwargs": model.TEMPLATE_KWARGS,
              "action_max_tokens": model.ACTION_MAX_TOKENS, "note_max_tokens": model.NOTE_MAX_TOKENS, "games": GAMES, "phase_hands": 12, "update_every": 6,
              "keep_prior_on_note_failure": model.KEEP_PRIOR_ON_NOTE_FAILURE,
              "training_templates": TRAIN, "probe_templates": PROBES,
              "rules": RULES, "read_prompt": READ, "note_prompts": UPDATES,
              "probability_encoding": "Legacy string buckets at 5-point intervals; may distort non-bucket answers. See numeric replay.",
              "probe_order": ["board 1 air", "board 1 set", "board 2 air", "board 2 set"],
              "primary": "Post-phase-2 probe loss for direct versus read-then-bet, by notebook format.",
              "unit": "One observed tape per condition; descriptive pilot only.",
              "exposure": "Predetermined pot bets, same responses for both notebook writers.",
              "host_baseline": "Recent six bets, (folds+1)/(bets+2), rounded to nearest 5 percent.",
              "controls": "Frozen notes and predictions, blank, true current fold probability.",
              "cache": "Identical requests reuse their first answer at temperature zero."}
    (directory / "design.json").write_text(json.dumps(design, indent=2) + "\n")
    rows = []
    with (directory / "trace.jsonl").open("x") as model.trace:
        for seed, before, after in GAMES:
            rng = random.Random(seed)
            suits = rng.sample("cdhs", 4)
            probes = [(model.deal(t, strong, suits), strong) for t, strong in
                      itertools.product(PROBES, (False, True))]
            notebook = {arm: "" for arm in UPDATES}
            observations = []
            for phase, probability in enumerate((before, after), 1):
                current = []
                for i in range(12):
                    cards = model.deal(TRAIN[i % len(TRAIN)], i % 2 == 0, rng.sample("cdhs", 4))
                    response = "fold" if rng.random() < probability else "call"
                    current.append({"hand": len(observations) + i + 1, **cards,
                                    "your_action": "bet", "opponent_response": response})
                observations.extend(current)
                model.log("tape", seed=seed, phase=phase, hands=current)
                for start in (0, 6):
                    for arm, prompt in UPDATES.items():
                        notebook[arm] = ask(RULES + " Update your private scouting notebook. " + prompt,
                                            {"prior_notebook": notebook[arm], "completed_hands": current[start:start+6]},
                                            "notes")
                estimates = {arm: predict({"private_notebook": text}) for arm, text in notebook.items()}
                estimates["raw"] = predict({"completed_hands": observations})
                recent_folds = sum(h["opponent_response"] == "fold" for h in current[-6:])
                estimates["counts"] = 5 * round(100 * (recent_folds + 1) / 8 / 5)
                if phase == 1:
                    frozen_notes, frozen_estimates = notebook.copy(), estimates.copy()
                scores = evaluate(probes, probability,
                                  {**notebook, **{f"frozen_{k}": v for k, v in frozen_notes.items()}},
                                  {**estimates, **{f"frozen_{k}": v for k, v in frozen_estimates.items()
                                                 if k in UPDATES}})
                row = {"seed": seed, "phase": phase, "true_fold_percent": int(100 * probability),
                       "observed_folds": sum(h["opponent_response"] == "fold" for h in current),
                       "recent_folds": recent_folds, "notes": notebook.copy(), "estimates": estimates,
                       "scores": scores}
                rows.append(row)
                model.log("checkpoint", **row)
                (directory / "results.json").write_text(json.dumps(rows, indent=2) + "\n")
                print(json.dumps(row), flush=True)
    return rows


if __name__ == "__main__":
    run(Path(sys.argv[1]))
