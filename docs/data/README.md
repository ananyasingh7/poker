# River replay records

The three arrays in `river-replays.json` represent different kinds of evidence.

- `overnight` contains hands 43–58, in order, from the November 23 abrupt-switch game with seed 1410000 and the live model's own read and action. Selected fields come from `runs/inkling-sleep-v1/hand.jsonl`. The initial pot and bet were both 10 bb. Only the recorded opponent response was visible to the player. The hidden probability and expected loss are grader annotations.
- `training` contains the first 12 annotated table-format practice examples in the shuffled [Session 001 training file](../../results/14-inkling-finetuning/train.jsonl.gz). Each includes its one-based line number. An action here is a computed teaching target.
- `transfer` contains every cards-only question in the Session 001 local size-switch probe, joined across the original and tuned response logs by pair and bet size. Each group contains one cheap and one expensive bet. Both replies are actual model outputs. The target comes from the expected chip values. These questions were created separately from the training set.
