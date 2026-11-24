"""Export completed Session 002 replicates and test each in the local serving stack."""

import argparse
import gzip
import json
import os
import subprocess
import time
from contextlib import contextmanager
from pathlib import Path
from tempfile import TemporaryDirectory
from urllib.error import URLError
from urllib.request import urlopen

from inkling_session002 import OUT, SEEDS, evaluate, write_split
from inkling_transfer_audit import decide
from score_inkling_tune import score as historical_score

RUN = Path("runs/inkling-tuning/002-pricing")
ENV = Path("runs/inkling-training-env/bin")
SSD = Path("/Volumes/FastSSD/inkling-pi-test")
BASE = SSD / "models/Inkling-2-Mini-IQ4_NL.gguf"
CPP = SSD / "runtime/llama.cpp-29b52d398b294509746ec0a3f28948a3107c0213"
URL = "http://127.0.0.1:9933"


def final_checkpoint(folder, wait):
    path = folder / "trainer/checkpoints.jsonl"
    while True:
        if path.exists():
            for line in path.read_text().splitlines():
                checkpoint = json.loads(line)
                if checkpoint["name"] == "final":
                    return checkpoint
        if not wait:
            raise ValueError(f"Training incomplete in {folder}")
        time.sleep(5)


def export(folder, checkpoint):
    destination = folder / "export"
    env = {**os.environ, "PYTHONPATH": str(CPP / "gguf-py")}
    if not destination.exists():
        # Publish only an entire export. An interrupted download or conversion can be retried.
        with TemporaryDirectory(dir=folder) as temporary:
            staging = Path(temporary) / "export"
            subprocess.run(
                [
                    ENV / "tinker",
                    "checkpoint",
                    "download",
                    checkpoint["state_path"],
                    "--output",
                    staging,
                ],
                check=True,
            )
            (weights,) = staging.glob("*/adapter_model.safetensors")
            subprocess.run(
                [
                    ENV / "python",
                    "experiments/convert_inkling_poker_adapter.py",
                    weights.parent,
                    BASE,
                    staging / "poker.gguf",
                ],
                check=True,
                env=env,
            )
            subprocess.run(
                [
                    ENV / "python",
                    "experiments/session001/verify_conversion.py",
                    staging,
                ],
                check=True,
                env=env,
            )
            staging.rename(destination)
    else:
        subprocess.run(
            [
                ENV / "python",
                "experiments/session001/verify_conversion.py",
                destination,
            ],
            check=True,
            env=env,
        )
    adapter = destination / "poker.gguf"
    final = folder / "final.gguf"
    if not final.exists():
        os.link(adapter, final)
    if not os.path.samefile(adapter, final):
        raise RuntimeError(f"Conflicting local adapter: {final}")
    return final


def retention(seed):
    # Exactly the rows of Session 001's matched, fixed local comparison.
    with gzip.open(OUT / "retention-questions.jsonl.gz", "rt") as handle:
        rows = [json.loads(line) for line in handle]
    predictions = []
    for row in rows:
        system, user = row["messages"]
        prediction = decide(
            {
                "kind": "hand" if row["format"] == "hand_read" else "combined",
                "system": system["content"],
                "view": json.loads(user["content"]),
            },
            1,
        )
        predictions.append(dict(group=row["group"], format=row["format"], **prediction))
    write_split(f"retention-{seed}", predictions)
    return historical_score(rows, predictions)


@contextmanager
def local_server(folder, adapter):
    with (folder / "local.log").open("a") as log:
        server = subprocess.Popen(
            ["scripts/serve-inkling-001.sh"],
            stdout=log,
            stderr=log,
            env={**os.environ, "INKLING_ADAPTER": str(adapter.resolve())},
        )
        try:
            for _ in range(180):
                if server.poll() is not None:
                    raise RuntimeError(f"Server exited; see {folder}/local.log")
                try:
                    if (
                        json.load(urlopen(URL + "/health", timeout=3)).get("status")
                        == "ok"
                    ):
                        break
                except (URLError, TimeoutError):
                    pass
                time.sleep(1)
            else:
                raise TimeoutError("Local server warmup")
            registered = json.load(urlopen(URL + "/lora-adapters", timeout=5))
            if (
                len(registered) != 1
                or Path(registered[0]["path"]).resolve() != adapter.resolve()
                or registered[0]["id"] != 0
            ):
                raise RuntimeError("Different local adapter is serving requests")
            yield
        finally:
            server.terminate()
            try:
                server.wait(timeout=20)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()


def main(wait, parent_controls, seeds=SEEDS):
    if parent_controls:
        adapter = Path("runs/inkling-tuning/001-river/selected-adapter/poker.gguf")
        with local_server(RUN, adapter):
            retention("parent-local")
        return

    for seed in seeds:
        folder = RUN / str(seed)
        adapter = export(folder, final_checkpoint(folder, wait))
        with local_server(folder, adapter):
            evaluate(seed)
            scores = retention(seed)
            (OUT / f"retention-{seed}.json").write_text(
                json.dumps(scores, indent=2) + "\n"
            )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wait", action="store_true")
    parser.add_argument(
        "--parent-controls",
        action="store_true",
        help="Replay step 140 on the same local runtime",
    )
    parser.add_argument(
        "--seed", type=int, choices=SEEDS, help="Evaluate a single completed replicate"
    )
    args = parser.parse_args()
    main(args.wait, args.parent_controls, (args.seed,) if args.seed else SEEDS)
