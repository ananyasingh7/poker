import gzip
import hashlib
import json
import unittest
from pathlib import Path

from inkling_session002 import OUT, score
from inkling_split_audit import boards, pricing_cases, require_novel
from prepare_inkling_tune import classify


class FrozenSession002(unittest.TestCase):
    def test_splits(self):
        manifest = json.loads((OUT / "manifest.json").read_text())
        paths = [
            OUT / "train.jsonl.gz",
            OUT / "dev.jsonl.gz",
            Path("results/15-inkling-audit/size-challenge.jsonl.gz"),
        ]
        for name, path in zip(("train", "dev"), paths):
            self.assertEqual(
                hashlib.sha256(gzip.decompress(path.read_bytes())).hexdigest(),
                manifest[name + "_sha256"],
            )
        for index, path in enumerate(paths):
            for other in paths[index + 1 :]:
                left, right = pricing_cases(path), pricing_cases(other)
                require_novel(left, right)
                self.assertFalse({key[1] for key in left} & {key[1] for key in right})
                self.assertFalse(boards(path) & boards(other))

    def test_labels_and_denominators(self):
        for split in ("train", "dev"):
            for line in gzip.open(OUT / (split + ".jsonl.gz"), "rt"):
                row = json.loads(line)
                view = json.loads(row["messages"][1]["content"])
                kind, wins = classify(view)
                if kind == "hand_read":
                    expected = "win" if wins else "lose"
                else:
                    rate = view.get(
                        "fold_percent", view.get("estimated_opponent_fold_percent")
                    )
                    gain = rate * view["pot_bb"] - (100 - rate) * view["bet_bb"]
                    self.assertNotEqual(gain, 0)
                    expected = "bet" if wins or gain > 0 else "check"
                target = (
                    row.get("target")
                    or json.loads(row["messages"][-1]["content"])["action"]
                )
                self.assertEqual(expected, target)
        dev = [json.loads(line) for line in gzip.open(OUT / "dev.jsonl.gz", "rt")]
        oracle = score([dict(row, answer=row["target"]) for row in dev])
        self.assertEqual(oracle["hand"]["correct"], 32)
        for kind in ("price", "combined"):
            self.assertEqual(oracle[kind]["pairs_correct"], 16)
            self.assertEqual(oracle[kind]["mean_loss_bb"], 0)
