import unittest

from inkling_split_audit import pricing_key, require_novel
from inkling_transfer_audit import hand_questions, price_questions
from prepare_inkling_tune import classify, prompt_key
from score_inkling_tune import score
from session001.size_transfer import make_pairs, score_pairs


class TuningTests(unittest.TestCase):
    def test_scaled_duplicate(self):
        small = dict(pot_bb=10, bet_bb=2.5, fold_percent=30, if_called="you lose")
        large = dict(small, pot_bb=40, bet_bb=10)
        with self.assertRaisesRegex(ValueError, "overlap"):
            require_novel({pricing_key(small)}, {pricing_key(large)})
        require_novel({pricing_key(small)}, {pricing_key(dict(small, bet_bb=10))})

    def test_hand_labels_and_shortcut_counterexamples(self):
        expected = ["lose", "lose", "win", "win", "win", "win", "lose", "win"]
        for question, label in zip(hand_questions(), expected, strict=True):
            self.assertEqual(question["target"], label)
            view = question["view"]
            self.assertEqual(classify(view), ("hand_read", label == "win"))
        view = dict(view, hole=[view["board"][0], view["hole"][1]])
        with self.assertRaises(ValueError):
            classify(view)

    def test_duplicate_and_incomplete_predictions(self):
        row = dict(group=1, format="plain", answer="bet")
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            score([], [row, row])
        with self.assertRaises(ValueError):
            score_pairs([])

    def test_thresholds_and_pair_denominator(self):
        rates = list(price_questions())
        self.assertEqual([q["target"] for q in rates[:2]], ["bet", "check"])
        templates = [
            dict(group=0, format=fmt, messages=[{"role": "system", "content": "test"}])
            for fmt in ("plain", "cards_only")
        ]
        rows = [dict(row, answer=row["target"]) for row in make_pairs(templates)]
        self.assertEqual(score_pairs(rows)["cards_only"]["pairs_correct"], 12)
        self.assertEqual(len({row["fold_pct"] for row in rows}), 4)
        with self.assertRaisesRegex(ValueError, "Incomplete"):
            score_pairs(rows[1:])

    def test_normalized_prompt_key(self):
        before = [dict(content="rules"), dict(content='{"a":1,"b":2}')]
        after = [before[0], dict(content='{ "b":2, "a":1 }')]
        self.assertEqual(prompt_key(before), prompt_key(after))

    def test_frozen_challenge(self):
        from collections import Counter
        from hashlib import sha256
        import gzip
        import json
        from pathlib import Path
        from inkling_split_audit import pricing_cases

        folder = Path(__file__).resolve().parents[1] / "results"
        train = pricing_cases(folder / "14-inkling-finetuning/train.jsonl.gz")
        candidate = folder / "15-inkling-audit/size-challenge.jsonl.gz"
        require_novel(train, pricing_cases(candidate))
        data = gzip.decompress(candidate.read_bytes())
        self.assertEqual(
            sha256(data).hexdigest(),
            "e8030e66f721667a2ab43f77a866fbb18d4449bb7c7c0890c22ecb355e81a5b2",
        )
        strata = Counter()
        sides = Counter()
        for line in data.splitlines():
            row = json.loads(line)
            view = json.loads(row["messages"][1]["content"])
            ratio, percent, wins = pricing_key(view)
            shares_rank = bool(
                {c[0] for c in view["board"]} & {c[0] for c in view["hole"]}
            )
            strata[wins, shares_rank] += 1
            sides[row["group"], wins, row["target"]] += 1
            self.assertEqual(
                row["target"],
                "bet" if wins or percent > (100 - percent) * ratio else "check",
            )
        self.assertEqual(strata[True, True], strata[True, False])
        self.assertEqual(strata[False, True], strata[False, False])
        self.assertEqual(len(sides), 288)
