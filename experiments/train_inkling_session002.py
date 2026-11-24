"""Materialize three training orders and count rendered tokens. --train submits paid work."""

import argparse
import gzip
import hashlib
import json
import random
import subprocess
import sys
from pathlib import Path

from tinker_cookbook.recipes.chat_sl.train import get_dataset_builder
from inkling_session002 import OUT, PARENT, SEEDS

MODEL = "thinkingmachines/Inkling-2-Mini"
RUN = Path("runs/inkling-tuning/002-pricing")
MAX_LENGTH = 320


def main(train):
    manifest = json.loads((OUT / "manifest.json").read_text())
    source = gzip.decompress((OUT / "train.jsonl.gz").read_bytes())
    if hashlib.sha256(source).hexdigest() != manifest["train_sha256"]:
        raise ValueError("Training hash differs from frozen manifest")
    paths = []
    order_hashes = {}
    for seed in SEEDS:
        folder = RUN / str(seed)
        folder.mkdir(parents=True, exist_ok=True)
        lines = source.splitlines(keepends=True)
        random.Random(seed).shuffle(lines)
        data = b"".join(lines)
        path = folder / "train.jsonl"
        path.write_bytes(data)
        paths.append(path)
        order_hashes[str(seed)] = hashlib.sha256(data).hexdigest()
    # Measure uncapped example lengths before applying the recipe cap.
    builder = get_dataset_builder(str(paths[0]), MODEL, "tml_v0", None, 32)
    dataset, _ = builder()
    sizes = [
        item.model_input.length
        for batch in range(len(dataset))
        for item in dataset.get_batch(batch)
    ]
    if len(sizes) != manifest["train_examples"] or max(sizes) >= MAX_LENGTH:
        raise ValueError("Recipe would truncate or skip training examples")
    report = dict(
        order_hashes=order_hashes,
        batches=len(dataset),
        positions_per_run=sum(sizes),
        max_positions=max(sizes),
        price_assumption_usd_per_million=0.75,
        estimated_three_runs_usd=sum(sizes) * len(paths) * 0.75 / 1e6,
    )
    (OUT / "render-check.json").write_text(json.dumps(report, indent=2) + "\n")
    print(report, flush=True)
    if not train:
        return
    if not all(
        (OUT / (name + ".jsonl.gz")).is_file() for name in ("original", "step140")
    ):
        raise RuntimeError("Complete both matched baselines first")
    for path in paths:
        folder = path.parent / "trainer"
        if (folder / "checkpoints.jsonl").exists():
            checkpoints = list(
                map(json.loads, (folder / "checkpoints.jsonl").read_text().splitlines())
            )
            if any(c["name"] == "final" for c in checkpoints):
                continue
        if folder.exists():
            raise RuntimeError(f"Inspect incomplete run before retry: {folder}")
        subprocess.run(
            [
                sys.executable,
                "-m",
                "tinker_cookbook.recipes.chat_sl.train",
                f"model_name={MODEL}",
                "renderer_name=tml_v0",
                f"load_checkpoint_path={PARENT}",
                f"dataset={path}",
                f"log_path={folder}",
                "num_epochs=1",
                "batch_size=32",
                "lora_rank=16",
                "learning_rate=0.00005",
                f"max_length={MAX_LENGTH}",
                "save_every=24",
                f"session_name=inkling-session002-{path.parent.name}",
            ],
            check=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--train", action="store_true")
    main(parser.parse_args().train)
