# What we learned about Inkling

**Status: complete.** Updated 2026-11-21T11:02:01.178713+00:00.

Inkling passed our predeclared adaptation test in this controlled river drill.

Finished 96/96 independent histories and 24/24 paired own-choice games.

## Which change should we try next?

**Start with a fixed betting calculator.** Both fixes reduced mistakes in these comparisons. Test their combination in new own-choice games before treating the gain as established in ordinary play.

- Replacing Inkling's betting with a calculator saved **0.903 bb per decision (95% interval 0.660 to 1.144)**, using the same read.
- Replacing its notes with exact recent counts saved **0.125 bb per decision (95% interval 0.052 to 0.219)**, using the same calculator.
- Replacing both steps saved **1.028 bb per decision (95% interval 0.792 to 1.262)** compared with its separate reading-and-betting setup.

These comparisons are descriptive. Positive savings means fewer chips given up through bad choices.

**Fine-tuning remains untested.** We should train only if useful, repeatable mistakes remain after testing the simpler fix. The study saves correctly graded training candidates and keeps evaluation histories separate.

## Did it use new evidence?

Fresh reads saved **1.003 bb per decision (95% interval 0.729 to 1.281)** compared with the old read after an opponent changed.

- When a caller became a folder: 1.076 bb per decision (95% interval 0.653 to 1.549) saved.
- When a folder became a caller: 0.931 bb per decision (95% interval 0.611 to 1.271) saved.

A positive average alone does not pass our test: the interval must sit above zero and both switch directions must improve on average.

## Did it work when choosing its own bets?

Separating reading from betting saved **0.081 bb per decision (95% interval -0.161 to 0.362)** after a change. Each pair faced the same cards and response draws, but its choices determined what it learned.

## How accurate were the reads?

Average distance from the opponent's actual fold percentage; lower is better.

- Inkling reading its notes: 8.13 percentage points.
- Inkling reading the full action history: 8.61 percentage points.
- Exact recent counts with a small smoothing adjustment: 7.21 percentage points.

## Basic skills

- Given the fold chance and hand strength: 88.8% correct across 160 questions.
- Given the calculated winnings and hand strength: 96.9% correct across 160 questions.
- Given the fold chance, reading its own cards: 81.2% correct across 160 questions.

5 note writes failed and kept the previous note. Failed responses remain in the results.

## What this establishes

This measures controlled river decisions, not full six-seat poker or a measured fine-tuning benefit. The main histories supply hand strength and guarantee exposure to opponent responses. The own-choice games check what happens when exposure depends on Inkling's bets. Intervals treat whole histories as independent; repeated test questions are not counted as independent games.

[Detailed report](summary.md) · [Analysis data](analysis.json) · [Registered plan](protocol.json)
