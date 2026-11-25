"""Rebalance Session 003 training. Reuse the original development set and baseline."""

import argparse
import fcntl
import hashlib
import json
import random
from collections import defaultdict
from pathlib import Path

from inkling_fresh_diagnosis import save
from inkling_overnight import best_action, canonical
from inkling_session003 import OUT as FIXED
from inkling_session003 import PARENT, SEEDS
from prepare_inkling_tune import classify
from score_inkling_session003 import always_bet, read
from score_inkling_session003 import main as evaluate

OUT = Path("results/19-inkling-session-003-revision")
RUN = Path("runs/inkling-tuning/003-value-revision")


def prepare():
    rows = read(FIXED / "train.jsonl.gz")
    groups = defaultdict(list)
    training = []
    rng = random.Random(193000)
    for row in rows:
        view = json.loads(row["messages"][1]["content"])
        if "bet_bb" not in view:
            training.append(row)
        else:
            rate = view.get("fold_percent", view.get("estimated_opponent_fold_percent"))
            groups[(rate, view["pot_bb"])].append((row, view))
    for index, ((rate, pot), group) in enumerate(sorted(groups.items())):
        sizes = sorted({view["bet_bb"] for _, view in group})
        assert len(sizes) == 2 and len(group) == 8
        winner_size = sizes[index % 2]
        fair = pot * rate / (100 - rate)
        more = round(fair + (sizes[1] - fair) * rng.uniform(0.4, 1.6), 2)
        if more == sizes[1] or more <= fair:
            more = round(sizes[1] + 0.01, 2)
        for row, view in group:
            _, wins = classify(view)
            if wins and view["bet_bb"] != winner_size:
                continue
            training.append(row)
            if not wins and view["bet_bb"] == sizes[1]:
                assert best_action(rate / 100, False, pot, more) == "check"
                system, user, answer = row["messages"]
                training.append(
                    {
                        "messages": [
                            system,
                            {**user, "content": canonical({**view, "bet_bb": more})},
                            answer,
                        ]
                    }
                )
    rng.shuffle(training)
    OUT.mkdir(exist_ok=True)
    save("train", training, OUT)
    data = "".join(canonical(row) + "\n" for row in training).encode()
    manifest = {
        "train_examples": len(training),
        "train_sha256": hashlib.sha256(data).hexdigest(),
        "parent": PARENT,
        "shuffle_seeds": SEEDS,
        "fixed_dev_sha256": json.loads((FIXED / "manifest.json").read_text())[
            "dev_sha256"
        ],
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (OUT / "always-bet.json").write_text(
        json.dumps(always_bet(read(FIXED / "dev.jsonl.gz")), indent=2) + "\n"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["prepare", "train", "score"])
    parser.add_argument(
        "--wait",
        action="store_true",
        help="Wait for final training checkpoints while scoring",
    )
    args = parser.parse_args()
    mode = args.mode
    if mode == "prepare":
        prepare()
    elif mode == "score":
        with (RUN / "score.lock").open("a") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            if not (OUT / "results.json").exists():
                evaluate(OUT, RUN, FIXED, compare_always_bet=True, wait=args.wait)
    else:
        from train_inkling_free import train
        from train_inkling_session002 import main

        main(False, out=OUT, run=RUN, seeds=SEEDS, price_per_million=0)
        train(OUT, RUN, FIXED, "003-revision")
