import gzip
import hashlib
import json
import unittest
from pathlib import Path

from inkling_session002 import OUT, SEEDS, score
from inkling_split_audit import boards, pricing_cases, require_novel
from prepare_inkling_tune import classify
from score_inkling_tune import score as historical_score
from summarize_inkling_session002 import load_predictions


class FrozenSession002(unittest.TestCase):
    def test_report_replays_from_committed_answers(self):
        report = json.loads((OUT / "results.json").read_text())
        questions = load_predictions("dev")
        controls = load_predictions("retention-questions")

        def control_actions(name):
            return {
                (r["group"], r["format"]): r["answer"] for r in load_predictions(name)
            }

        self.assertEqual(
            control_actions("retention-step140"),
            control_actions("retention-parent-local"),
        )
        losses = {}
        for seed in SEEDS:
            arm = report["replicates"][str(seed)]
            answers = load_predictions(f"candidate-{seed}")
            self.assertEqual(
                [
                    {k: row[k] for k in question}
                    for row, question in zip(answers, questions, strict=True)
                ],
                questions,
            )
            self.assertEqual(score(answers), arm["metrics"])
            history = historical_score(controls, load_predictions(f"retention-{seed}"))
            self.assertEqual(history["overall_betting"], arm["retention"])
            losses[seed] = arm["metrics"]["combined"]["mean_loss_bb"]
        median_loss = sorted(losses.values())[1]
        self.assertEqual(
            report["representative"],
            next(seed for seed in SEEDS if losses[seed] == median_loss),
        )

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
