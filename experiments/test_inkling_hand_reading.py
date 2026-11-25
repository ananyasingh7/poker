import json
import unittest
from collections import defaultdict

from inkling_hand_reading import conditioned_questions, load, summarize
from prepare_inkling_tune import hand_wins


class HandReading(unittest.TestCase):
    def test_training_pairs_flip_one_card_and_do_not_overlap_development(self):
        pairs = defaultdict(list)
        for row in load("train"):
            view = json.loads(row["messages"][1]["content"])
            answer = json.loads(row["messages"][2]["content"])["action"]
            self.assertEqual(answer, "win" if hand_wins(view) else "lose")
            pairs[tuple(sorted(view["board"]))].append((view["hole"], answer))
        self.assertEqual(len(pairs), 640)
        shared_count = 0
        for board, hands in pairs.items():
            (first, target), (second, other) = hands
            self.assertNotEqual(target, other)
            self.assertEqual(len(set(first) & set(second)), 1)
            ranks = {card[0] for card in board}
            shares = [
                any(card[0] in ranks for card in hole) for hole in (first, second)
            ]
            self.assertEqual(*shares)
            shared_count += shares[0]
        self.assertEqual(shared_count, 320)
        dev = load("dev")
        boards = {
            tuple(sorted(json.loads(row["messages"][1]["content"])["board"]))
            for row in dev
        }
        self.assertEqual((len(boards), len(dev)), (32, 192))
        self.assertFalse(boards & pairs.keys())

    def test_second_call_uses_own_wrong_read_but_score_uses_actual_cards(self):
        first = [
            dict(row, answer="win" if row["kind"] == "hand" else row["target"])
            for row in load("dev")
        ]
        second = []
        for question in conditioned_questions(first):
            view = json.loads(question["messages"][1]["content"])
            self.assertEqual(
                set(view), {"if_called", "pot_bb", "bet_bb", "fold_percent"}
            )
            self.assertEqual(view["if_called"], "you win")
            second.append(dict(question, answer="bet"))
        result = summarize(first, second)
        self.assertEqual(result["false_wins"], 32)
        self.assertEqual(result["direct"]["mean_loss_bb"], 0)
        self.assertEqual(result["read_then_act"], result["always_bet"])
        self.assertGreater(result["read_then_act"]["mean_loss_bb"], 0)
        self.assertEqual(result["read_then_act"]["expensive_bluffs"], 32)
        second[0]["answer"] = None
        with self.assertRaisesRegex(ValueError, "Invalid betting action"):
            summarize(first, second)
        first[0]["answer"] = None
        with self.assertRaisesRegex(ValueError, "invalid card read"):
            conditioned_questions(first)
