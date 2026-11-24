import gzip
import json
import unittest

from inkling_fresh_diagnosis import OUT, RESULT, diagnose
from inkling_split_audit import boards, pricing_cases


class FreshDiagnosisTest(unittest.TestCase):
    def test_groups_and_exclusions(self):
        path = RESULT / "questions.jsonl.gz"
        with gzip.open(path, "rt") as handle:
            rows = list(map(json.loads, handle))
        self.assertEqual(len(rows), 256)
        self.assertEqual(len({row["group"] for row in rows}), 32)
        current = {key[1] for key in pricing_cases(path)}
        self.assertEqual(len(current), 32)
        for reference in [
            OUT / "train.jsonl.gz",
            OUT / "dev.jsonl.gz",
            OUT.parent / "15-inkling-audit/size-challenge.jsonl.gz",
        ]:
            self.assertFalse(current & {key[1] for key in pricing_cases(reference)})
            self.assertFalse(boards(path) & boards(reference))

    def test_matched_errors(self):
        with gzip.open(RESULT / "questions.jsonl.gz", "rt") as handle:
            group = [
                dict(row, answer=row["target"])
                for row in map(json.loads, handle)
                if row["group"] == 0
            ]
        bad_bluff = next(
            r for r in group if r["kind"] == "combined" and r["target"] == "check"
        )
        price = next(
            r for r in group if r["kind"] == "price" and r["target"] == "check"
        )
        read = next(r for r in group if r["kind"] == "hand" and r["target"] == "lose")
        bad_bluff["answer"] = "bet"
        for read_answer, price_answer, expected in [
            ("lose", "check", "combined_only"),
            ("win", "check", "hand_wrong"),
            ("lose", "bet", "price_wrong"),
            ("win", "bet", "both_wrong"),
        ]:
            read["answer"] = read_answer
            price["answer"] = price_answer
            ledger = diagnose(group[::-1])
            errors = [r for r in ledger if r["loss_bb"] > 0]
            self.assertEqual([r["category"] for r in errors], [expected])
