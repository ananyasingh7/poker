import gzip
import json
import unittest
from collections import defaultdict

from inkling_session003 import OUT, REFERENCES
from inkling_split_audit import boards, pricing_cases
from prepare_inkling_tune import classify


class ValuePairs(unittest.TestCase):
    def test_fresh_boards_and_rates(self):
        references = list(REFERENCES)
        for name in ["train", "dev"]:
            path = OUT / f"{name}.jsonl.gz"
            new_boards = boards(path)
            new_rates = {k[1] for k in pricing_cases(path)}
            groups = 128 if name == "train" else 32
            self.assertEqual((len(new_boards), len(new_rates)), (groups, groups))
            for ref in references:
                # The historical raw originals are local-only; their overlap was checked
                # at dataset generation. The published references are always available.
                if not ref.exists() and ref.parts[0] == "runs":
                    continue
                self.assertFalse(new_boards & boards(ref))
                self.assertFalse(new_rates & {k[1] for k in pricing_cases(ref)})
            references.append(path)

    def test_matched_hands_and_exact_labels(self):
        for split in ["train", "dev"]:
            groups = defaultdict(list)
            with gzip.open(OUT / f"{split}.jsonl.gz", "rt") as handle:
                for row in map(json.loads, handle):
                    view = json.loads(row["messages"][1]["content"])
                    target = (
                        row.get("target")
                        or json.loads(row["messages"][2]["content"])["action"]
                    )
                    kind, wins = classify(view)
                    if kind == "hand_read":
                        self.assertEqual(target, "win" if wins else "lose")
                        continue
                    rate = view.get(
                        "fold_percent", view.get("estimated_opponent_fold_percent")
                    )
                    profit = rate * view["pot_bb"] - (100 - rate) * view["bet_bb"]
                    self.assertNotEqual(profit, 0)
                    self.assertEqual(target, "bet" if wins or profit > 0 else "check")
                    if "board" in view:
                        groups[tuple(view["board"])].append((wins, target, view))
            for decisions in groups.values():
                self.assertEqual(len(decisions), 4)
                holes = {wins: tuple(view["hole"]) for wins, _, view in decisions}
                self.assertEqual(len(set(holes[True]) & set(holes[False])), 1)
                by_price = defaultdict(list)
                for wins, target, view in decisions:
                    by_price[
                        view["pot_bb"],
                        view["estimated_opponent_fold_percent"],
                        view["bet_bb"],
                    ].append((wins, target))
                self.assertEqual(len(by_price), 2)
                self.assertEqual(
                    sorted(map(sorted, by_price.values())),
                    [
                        [(False, "bet"), (True, "bet")],
                        [(False, "check"), (True, "bet")],
                    ],
                )
