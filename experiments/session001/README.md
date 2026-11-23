# Session 001 evaluation helpers

These are the checkpoint-selection, export validation and local comparison scripts used for the pilot. Run them from the repository root. The local comparison expects the FastSSD runtime and IQ4_NL base described in the spectator documentation.

- `follow_checkpoints.py` evaluates saved Tinker candidates, records the selection and downloads the winning adapter. Hosted sampling incurs usage.
- `verify_conversion.py` checks the local PEFT/GGUF weight updates. Pass the selected-adapter directory. Use the training environment with the pinned llama.cpp checkout's gguf-py on PYTHONPATH.
- `PYTHONPATH=experiments python3 experiments/session001/local-comparison.py runs/inkling-tuning/001-river/selected-adapter/poker.gguf` repeats the matched local comparison and size-switch probe, writing the results beside the adapter. The script uses the saved development dataset and a temporary loopback server.
- `size_transfer.py` builds and scores paired examples on which changing the bet price flips the correct decision.

[Published scores and compressed training examples](../../results/14-inkling-finetuning/) · [Training protocol](../../docs/inkling-first-fine-tune.md)
