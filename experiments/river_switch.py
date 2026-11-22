"""Local Qwen adaptation pilot. Run with: python3 experiments/river_switch.py NEW_RUN_DIR"""
import itertools
import json
import os
from pathlib import Path
import random
import sys
from urllib.request import ProxyHandler, build_opener, Request

URL = os.environ.get("POKER_MODEL_URL", "http://127.0.0.1:9931/v1/chat/completions")
MODEL = os.environ.get("POKER_MODEL", "qwen3.8-pi")
JSON_PREFILL = os.environ.get("POKER_JSON_PREFILL", "0") == "1"
ACTION_MAX_TOKENS = int(os.environ.get("POKER_ACTION_MAX_TOKENS", "32"))
NOTE_MAX_TOKENS = int(os.environ.get("POKER_NOTE_MAX_TOKENS", "256"))
TEMPLATE_KWARGS = json.loads(os.environ.get("POKER_TEMPLATE_KWARGS", '{"enable_thinking":false}'))
KEEP_PRIOR_ON_NOTE_FAILURE = os.environ.get("POKER_KEEP_PRIOR_ON_NOTE_FAILURE", "0") == "1"
REPLAY_ROOT = os.environ.get("POKER_REPLAY_TRACE_ROOT")
replayed = {}
if REPLAY_ROOT:
    for path in sorted(Path(REPLAY_ROOT).rglob('*trace*.jsonl'), key=lambda p: p.stat().st_mtime):
        for line in path.read_text().splitlines():
            row = json.loads(line)
            if row['kind'] == 'inference':
                key = json.dumps(row['request'], sort_keys=True)
                replayed.setdefault(key, []).append(row['reply'])
# Two deliberately extreme, unannounced policies. Opponent never raises.
GAMES = [(92101, "call", "fold"), (92102, "fold", "call")]
CHECKPOINT_HANDS = 8
POT = 10
GROUP = 4
# All boards unpaired, with at most two cards per suit. Opponent always holds
# top pair with an off-board kicker. Hero has either a set or unimproved air.
TRAIN = [("K9742", "Q5", "99"), ("Q9632", "J4", "66"),
         ("J8532", "T4", "88"), ("A9743", "Q2", "77")]
PROBE = [("K8642", "Q3", "88"), ("Q9742", "J5", "99")]
RULES = ("Heads-up hold'em river drill. You are Seat 0. Seat 1 is the same opponent throughout "
         "the session. Each deal starts on the river with 10 bb in the pot, 5 bb contributed "
         "by each player. Seat 1 has checked. In this drill Seat 1 is dealt exactly one pair "
         "of the board's highest rank. You may check or bet 10 bb. Seat 1 may call or fold. "
         "Maximize expected chips. Your private scouting notes contain earlier observations.")
http = build_opener(ProxyHandler({}))


def ask(system, observation, field, options=None, numeric=False):
    spec = {"type": "number", "minimum": 0, "maximum": 100} if numeric else {"type": "string"}
    if options:
        spec["enum"] = options
    schema = {"type": "object", "properties": {field: spec}, "required": [field],
              "additionalProperties": False}
    body = {"model": MODEL, "temperature": 0, "max_tokens": NOTE_MAX_TOKENS if not options else ACTION_MAX_TOKENS,
            "chat_template_kwargs": TEMPLATE_KWARGS,
            "response_format": {"type": "json_schema", "json_schema":
                                {"name": field, "strict": True, "schema": schema}},
            "messages": [{"role": "system", "content": system},
                         {"role": "user", "content": json.dumps(observation)}]}
    if JSON_PREFILL:
        body["messages"].append({"role": "assistant", "content": "{"})
    key = json.dumps(body, sort_keys=True)
    replay = replayed.get(key)
    was_replayed = bool(replay)
    if was_replayed:
        reply = replay.pop(0)
    else:
        req = Request(URL, json.dumps(body).encode(), {"Content-Type": "application/json"})
        with http.open(req, timeout=180) as response:
            reply = json.load(response)
    log("inference", request=body, reply=reply, replayed=was_replayed)
    choice = reply["choices"][0]
    if field == 'notes' and choice['finish_reason'] == 'length' and KEEP_PRIOR_ON_NOTE_FAILURE:
        prior = observation.get('prior', observation.get('prior_notebook'))
        assert isinstance(prior, str) and reply['model'] == MODEL
        log('notebook_failure', reason='length', policy='keep prior notebook')
        return prior
    assert choice["finish_reason"] == "stop" and reply["model"] == MODEL
    result = json.loads(choice["message"]["content"])[field]
    if numeric:
        assert type(result) in (int, float) and 0 <= result <= 100
    else:
        assert isinstance(result, str) and (not options or result in options)
    return result


def log(kind, **data):
    trace.write(json.dumps({"kind": kind, **data}) + "\n")
    trace.flush()


def deal(template, strong, suits):
    board, air, pair = template
    hole = pair if strong else air
    return {"board": [rank + suits[i % 4] for i, rank in enumerate(board)],
            "hole": [hole[0] + suits[2 if hole[0] != board[2] else 0],
                     hole[1] + suits[3]]}


def action(cards, notes):
    return ask(RULES, {**cards, "private_notebook": notes}, "action", ["check", "bet"])


def net(action_taken, policy, strong):
    if action_taken == "bet" and policy == "fold":
        return POT / 2
    stake = POT / 2 + (POT if action_taken == "bet" else 0)
    return stake if strong else -stake


def play_phase(rng, policy, notes):
    total = replies = 0
    history = []
    for i in range(CHECKPOINT_HANDS):
        # Each block includes value hands so a player who never bluffs can still
        # observe the opponent responding. Shuffle suit labels independently.
        strong = i % 2 == 0
        cards = deal(TRAIN[i % len(TRAIN)], strong, rng.sample("cdhs", 4))
        decision = action(cards, notes)
        profit = net(decision, policy, strong)
        observed = policy if decision == "bet" else "showdown"
        history.append({**cards, "your_action": decision, "opponent_response": observed,
                        "your_net_bb": profit})
        log("hand", seed=seed, phase=phase, **history[-1])
        total += profit
        replies += decision == "bet"
        if (i + 1) % GROUP == 0:
            notes = ask(RULES + " Update your private scouting notebook for future decisions "
                        "against Seat 1. Use the prior notebook and these completed hands only. "
                        "Write at most 80 words.", {"prior": notes, "hands": history}, "notes")
            history = []
    return notes, replies, total


def evaluate(seed, checkpoint, policy, notebooks, probes):
    scores = {}
    for arm, notes in notebooks.items():
        loss = 0
        decisions = []
        for cards, strong in probes:
            # Identical inputs at temperature zero share their first answer across checkpoints.
            key = (json.dumps(cards), notes)
            if key not in cache:
                cache[key] = action(cards, notes)
            decision = cache[key]
            decisions.append(decision)
            best = max(net(a, policy, strong) for a in ("bet", "check"))
            loss += best - net(decision, policy, strong)
        scores[arm] = {"mean_loss_bb": loss / len(probes), "actions": decisions}
    log("checkpoint", seed=seed, checkpoint=checkpoint, policy=policy,
        scores=scores, notebooks=notebooks)
    return scores


def run(directory):
    global trace, seed, phase, cache
    directory.mkdir(parents=True, exist_ok=False)
    trace = (directory / "trace.jsonl").open("x")
    rows = []
    for seed, before, after in GAMES:
        rng = random.Random(seed)
        probes = [(deal(t, strong, "cdhs"), strong) for t, strong in
                  itertools.product(PROBE, (False, True))]
        cache = {}
        notes = ""
        for phase, policy in enumerate((before, after), 1):
            notes, responses, profit = play_phase(rng, policy, notes)
            if phase == 1:
                frozen = notes
            scores = evaluate(seed, phase, policy, {"updated": notes, "frozen": frozen,
                              "blank": "", "oracle": f"Seat 1 always {policy}s a river bet."}, probes)
            row = dict(seed=seed, phase=phase, policy=policy, visible_responses=responses,
                       live_net_bb=profit, notes=notes, scores=scores)
            rows.append(row)
            print(json.dumps(row), flush=True)
    (directory / "results.json").write_text(json.dumps(rows, indent=2) + "\n")


if __name__ == "__main__":
    run(Path(sys.argv[1]))
