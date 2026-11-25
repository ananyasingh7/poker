"""Train frozen replicates exclusively on Tinker's free model."""

import fcntl
import hashlib
import json

import tinker
from inkling_session003 import OUT, PARENT, RUN, SEEDS
from tinker_cookbook.recipes.chat_sl.train import get_dataset_builder
from tinker_cookbook.supervised.common import compute_mean_nll
from train_inkling_session002 import MAX_LENGTH, MODEL

FREE_MODEL = MODEL + ":peft:65536:free"


def train_seed(seed, recipe, run=RUN, session="003", external_final_only=False):
    folder = run / str(seed)
    source = folder / "train.jsonl"
    if (
        hashlib.sha256(source.read_bytes()).hexdigest()
        != recipe["order_hashes"][str(seed)]
    ):
        raise ValueError("Unexpected training order")
    trainer = folder / "trainer"
    trainer.mkdir(exist_ok=True)
    checkpoints = trainer / "checkpoints.jsonl"
    previous = (
        list(map(json.loads, checkpoints.read_text().splitlines()))
        if checkpoints.exists()
        else []
    )
    last = previous[-1] if previous else None
    if last and last["name"] == "final":
        return
    builder = get_dataset_builder(str(source), MODEL, "tml_v0", MAX_LENGTH, 32)
    dataset, _ = builder()
    if len(dataset) != recipe["batches"] or len(dataset) != 40:
        raise ValueError("Unexpected batch count")
    start = last["batch"] if last else 0
    metrics = trainer / "metrics.jsonl"
    with tinker.ServiceClient(
        display_name=f"inkling-session{session}-free-{seed}"
    ) as service:
        if last:
            client = service.create_training_client_from_state_with_optimizer(
                last["state_path"], base_model=FREE_MODEL
            ).result()
        else:
            client = service.create_training_client_from_state(
                PARENT, base_model=FREE_MODEL
            ).result()
        info = client.get_info()
        if info.model_data.model_name != FREE_MODEL or info.lora_rank != 16:
            raise RuntimeError("Free rank-16 training client required")
        print(seed, "verified", FREE_MODEL, "starting at", start, flush=True)
        records = (
            list(map(json.loads, metrics.read_text().splitlines()))[:start]
            if metrics.exists()
            else []
        )
        if [r["step"] for r in records] != list(range(start)):
            raise ValueError("Missing checkpoint metrics")
        metrics.write_text("".join(json.dumps(r) + "\n" for r in records))
        for step in range(start, len(dataset)):
            batch = dataset.get_batch(step)
            rate = 5e-5 * (1 - step / len(dataset))
            fb = client.forward_backward(batch, "cross_entropy")
            opt = client.optim_step(
                tinker.AdamParams(learning_rate=rate, beta1=0.9, beta2=0.95, eps=1e-8)
            )
            outputs = fb.result().loss_fn_outputs
            opt.result()
            nll = compute_mean_nll(
                [r["logprobs"] for r in outputs],
                [d.loss_fn_inputs["weights"] for d in batch],
            )
            row = {
                "step": step,
                "learning_rate": rate,
                "num_sequences": len(batch),
                "num_tokens": sum(d.model_input.length for d in batch),
                "train_mean_nll": float(nll),
                "model": FREE_MODEL,
            }
            with metrics.open("a") as handle:
                handle.write(json.dumps(row) + "\n")
            print(seed, step + 1, "/", len(dataset), flush=True)
            completed = step + 1
            final = completed == len(dataset)
            if final or (not external_final_only and completed in (10, 20, 30)):
                name = "final" if final else f"{completed:06d}"
                save = (
                    client.save_weights_external
                    if external_final_only
                    else client.save_state
                )
                state = save(name).result()
                checkpoint = {
                    "batch": completed,
                    "name": name,
                    "state_path": state.path,
                    "model": FREE_MODEL,
                }
                with checkpoints.open("a") as handle:
                    handle.write(json.dumps(checkpoint) + "\n")


def train(
    out=OUT,
    run=RUN,
    baseline_dir=OUT,
    session="003",
    *,
    seeds=SEEDS,
    external_final_only=False,
):
    recipe = json.loads((out / "render-check.json").read_text())
    if not (baseline_dir / "step140.jsonl.gz").is_file():
        raise RuntimeError("Missing baseline")
    with (run / "train.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        for seed in seeds:
            train_seed(seed, recipe, run, session, external_final_only)
