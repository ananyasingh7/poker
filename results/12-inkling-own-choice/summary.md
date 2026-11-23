# Inkling own-choice study

Status: complete.

A positive saving means the helper gave up fewer chips per decision.
Intervals use whole games. Partial games are saved but excluded from comparisons.

- core: 32/32 complete matched games.
  - model_minus_calculator: 0.419 bb saved per decision; 95% interval [-0.0166666666666668, 0.8692708333333332].
  - model_minus_counts: 0.482 bb saved per decision; 95% interval [0.20624999999999993, 0.7749999999999999].
  - model_minus_both: 0.866 bb saved per decision; 95% interval [0.40989583333333324, 1.3447916666666666].
  - calculator_minus_both: 0.447 bb saved per decision; 95% interval [0.25781249999999994, 0.6515624999999999].
  - counts_minus_both: 0.384 bb saved per decision; 95% interval [0.003645833333333173, 0.7541666666666667].
  - Updating versus freezing the model read after a change: 1.189 bb saved; interval [0.5987947909472798, 1.8841756680760875] (descriptive).
  - Updating versus freezing the calculator read after a change: 1.659 bb saved; interval [1.006792106198886, 2.30640812932623] (descriptive).
- replication: 32/32 complete matched games.
  - model_minus_calculator: 0.528 bb saved per decision; 95% interval [0.13385416666666655, 0.9432291666666666].
  - model_minus_counts: 0.487 bb saved per decision; 95% interval [0.24635416666666665, 0.7546875].
  - model_minus_both: 1.257 bb saved per decision; 95% interval [0.8531249999999999, 1.7098958333333332].
  - calculator_minus_both: 0.730 bb saved per decision; 95% interval [0.5140625, 0.96875].
  - counts_minus_both: 0.770 bb saved per decision; 95% interval [0.42708333333333326, 1.165625].
  - Updating versus freezing the model read after a change: 1.066 bb saved; interval [0.4264681909367508, 1.797624639826479] (descriptive).
  - Updating versus freezing the calculator read after a change: 1.286 bb saved; interval [0.655760165833712, 1.9449830397340053] (descriptive).
- scarce: 16/16 complete matched games.
  - model_minus_model_probe: 0.022 bb saved per decision; 95% interval [-0.02499999999999998, 0.09999999999999999].
  - calculator_minus_calculator_probe: -0.034 bb saved per decision; 95% interval [-0.221875, 0.1375000000000001].
- gradual: 16/16 complete matched games.
  - model_minus_calculator: 0.260 bb saved per decision; 95% interval [-0.11927083333333334, 0.6261458333333333].
  - model_minus_counts: 0.041 bb saved per decision; 95% interval [-0.011770833333333335, 0.10067708333333336].
  - model_minus_both: 0.595 bb saved per decision; 95% interval [0.14020833333333335, 1.07125].
  - calculator_minus_both: 0.335 bb saved per decision; 95% interval [0.11093750000000001, 0.5569791666666667].
  - counts_minus_both: 0.554 bb saved per decision; 95% interval [0.11369791666666668, 1.0217708333333333].

Across-session memory: 12/12 complete identities; reset minus carry: {'n': 12, 'mean': -0.02083333333333341, 'interval_95': [-1.0291666666666668, 0.7083333333333333]}.
Mistake exam: 512/512 saved questions.
Code-only memory comparison: 400/400 complete seeds.
The format/card-reading exam and kept-memory results are in extra-analysis.json. Only exam development groups enter training-candidates.json.

Predeclared helper-benefit criterion met: False.
This tests local river decisions with supplied hand strength. It does not prove general poker learning.
The both arm uses only code. Probe costs are included. No weights were trained.
