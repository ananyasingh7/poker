import gzip
import hashlib
import json
import unittest
from collections import Counter

from inkling_overnight import best_action
from inkling_session003_revision import FIXED, OUT
from prepare_inkling_tune import classify
from score_inkling_session003 import always_bet, read


class RevisionTests(unittest.TestCase):
    def test_balance_labels_and_fixed_evaluation(self):
        manifest = json.loads((OUT / "manifest.json").read_text())
        self.assertEqual(
            manifest["fixed_dev_sha256"],
            hashlib.sha256(
                gzip.decompress((FIXED / "dev.jsonl.gz").read_bytes())
            ).hexdigest(),
        )
        rows = read(OUT / "train.jsonl.gz")
        self.assertEqual(len(rows), 1280)
        counts = Counter()
        for r in rows:
            _, user, answer = r["messages"]
            view = json.loads(user["content"])
            label = json.loads(answer["content"])["action"]
            _, wins = classify(view)
            if "bet_bb" in view:
                rate = view.get(
                    "fold_percent", view.get("estimated_opponent_fold_percent")
                )
                self.assertEqual(
                    label, best_action(rate / 100, wins, view["pot_bb"], view["bet_bb"])
                )
                counts["price" if "if_called" in view else "cards", label] += 1
            else:
                self.assertEqual(label, "win" if wins else "lose")
        self.assertEqual(
            counts,
            {
                (fmt, action): 256
                for fmt in ("price", "cards")
                for action in ("bet", "check")
            },
        )
        benchmark = always_bet(read(FIXED / "dev.jsonl.gz"))
        self.assertEqual(
            (benchmark["correct"], benchmark["total"], benchmark["pairs_correct"]),
            (96, 128, 0),
        )
