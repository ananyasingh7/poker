# Local river study — complete

Quiz results on development situations. Expected loss is in big blinds per decision.

- Expanded deals, cards_only: 93/144 correct, 0 answer failures, mean loss 8.472 bb.
- Expanded deals, hand_read: 99/144 correct, 0 answer failures.
- Expanded deals, plain: 93/144 correct, 0 answer failures, mean loss 6.644 bb.
- Expanded deals, table: 98/144 correct, 0 answer failures, mean loss 6.178 bb.
- Legacy deals, plain: 93/144 correct, 0 answer failures, mean loss 6.644 bb.
- Legacy deals, table: 103/144 correct, 0 answer failures, mean loss 5.653 bb.
- Legacy format changes: 78/144 paired decisions differed.
- Expanded format changes: 75/144 paired decisions differed.

Completed 288 arm-games. Post-change savings compare fully matched switched games only.
- abrupt: fresh reads saved 1.066 bb/decision across 24 switched games.
- gradual: fresh reads saved 0.068 bb/decision across 24 switched games.
- abrupt caller_to_folder: model mean absolute fold-rate error 28.7 percentage points.
- abrupt folder_to_caller: model mean absolute fold-rate error 23.7 percentage points.
- abrupt stable_caller: model mean absolute fold-rate error 12.0 percentage points.
- abrupt stable_folder: model mean absolute fold-rate error 12.0 percentage points.
- gradual caller_to_folder: model mean absolute fold-rate error 16.2 percentage points.
- gradual folder_to_caller: model mean absolute fold-rate error 14.6 percentage points.
- gradual stable_caller: model mean absolute fold-rate error 10.6 percentage points.
- gradual stable_folder: model mean absolute fold-rate error 11.1 percentage points.

Raw hands, model notes, fold estimates, grading probabilities and replies are checkpointed in state.sqlite3. Training examples and reserved exams are separate JSONL files.
