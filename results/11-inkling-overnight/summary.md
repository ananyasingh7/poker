# Inkling overnight results

Status: **complete**. Updated 2026-11-21T11:02:01.178713+00:00.

Completed 96/96 controlled histories and 24/24 paired own-choice games.

Values below are avoidable bb lost per decision. Lower loss is better. Positive savings means the change helped.

## Does it use new evidence?

Fresh reads saved **1.003 [0.729, 1.281] bb per decision** against frozen reads after a switch. The interval is a 95% bootstrap interval over independent histories.

The predeclared learning test passed in this controlled setup.

| Switch | Savings from fresh reads |
| --- | ---: |
| caller_to_folder | 1.076 [0.653, 1.549] |
| folder_to_caller | 0.931 [0.611, 1.271] |

## Memory, calculator or fine-tuning?

| Change | Chips saved per decision, with 95% interval |
| --- | ---: |
| fixed_calculator | 0.903 [0.660, 1.144] |
| exact_recent_counts | 0.125 [0.052, 0.219] |
| raw_history_memory | -0.083 [-0.177, 0.021] |
| counts_and_calculator | 1.028 [0.792, 1.262] |

These comparisons hold the other step fixed. The calculator uses the same model read; the memory comparison gives the same calculator a rate from exact recent counts.

Both the fixed calculator and exact-count memory helped. Use them together, then test on fresh histories before training.

**Fine-tuning has not been tested here.** Residual mistakes in questions with supplied probabilities or supplied payoffs identify training candidates; they do not show that training would fix them.

## Results by setup

| Setup | Average loss per decision |
| --- | ---: |
| calculator_counts | 0.000 [0.000, 0.000] |
| calculator_frozen | 1.531 [1.250, 1.844] |
| calculator_notes | 0.125 [0.052, 0.219] |
| calculator_raw_history | 0.208 [0.135, 0.292] |
| calculator_true | 0.000 [0.000, 0.000] |
| counts_read | 0.957 [0.740, 1.177] |
| direct_notes | 1.531 [1.226, 1.842] |
| fresh_read | 1.028 [0.792, 1.262] |
| frozen_read | 1.476 [1.203, 1.748] |
| perfect_read | 0.896 [0.698, 1.099] |

## Basic skills

| Check | Results |
| --- | --- |
| reader_counts | 80 questions; mean error 0.027 percentage points; 0 failed answers |
| reader_history | 80 questions; mean error 1.719 percentage points; 0 failed answers |
| reader_counts_with_rate | 80 questions; mean error 0.013 percentage points; 0 failed answers |
| bet_numeric_label_False | 160 questions; correct 81.250%; mean loss 2.827 bb; 0 failed answers |
| bet_sentence_label_False | 160 questions; correct 85.000%; mean loss 1.805 bb; 0 failed answers |
| bet_payoffs_label_False | 160 questions; correct 98.125%; mean loss 0.250 bb; 0 failed answers |
| bet_numeric_label_True | 160 questions; correct 88.750%; mean loss 1.853 bb; 0 failed answers |
| bet_sentence_label_True | 160 questions; correct 84.375%; mean loss 1.898 bb; 0 failed answers |
| bet_payoffs_label_True | 160 questions; correct 96.875%; mean loss 0.294 bb; 0 failed answers |

## Own-choice games

Separating the steps saved 0.081 [-0.161, 0.362] bb per decision after a change. The two setups saw the same cards and random response draws, but their own bets determined what evidence they collected.

5 note writes failed and kept their previous note. Failed actions check; failed reads retain their previous estimate (50% initially). No model failures are silently dropped.

## Limits

Intervals resample whole independent histories. Only fresh versus frozen is confirmatory; all intervention comparisons are descriptive. This is a river drill with supplied hand strength in the main experiment. No fine-tuning was performed.

Exact scripted probabilities are used only for grading and explicitly labelled perfect-information controls. Probe results never enter notebooks. Main and own-choice histories are held out from training. The quiz groups keep all wording and hand-label variants in the same split.
