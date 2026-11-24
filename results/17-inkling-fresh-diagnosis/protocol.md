# Fresh river diagnosis after Session 002

This is a development diagnostic. Step 140 remains the table model. Session 002 run 1 remains the representative selected in the preceding experiment. Each of the three Session 002 adapters and step 140 answers the same 256 prompts. The effective sample is 32 groups. Each group contains two river deals, a losing hand tested at two prices and a winning hand tested at one price. Each full decision has a matched standalone hand judgment and a price question with the correct hand strength supplied.

The generator uses seed 1724001 and the Session 002 rules. It reserves the boards and fold rates from Session 001 training, development, and historical exam, Session 002 training and development, and the independent release challenge. This tests fresh cases from the same synthetic setting. All prompts supply the opponent description or fold chance needed for that question.

Local evaluation uses the IQ4_NL model, one loaded adapter at scale 1, greedy JSON decisions, seed 14001 and the existing local launcher. Each question starts a separate conversation. Each run journals answers locally and binds them to the hashes of the questions and adapter. The release challenge is checked for overlap only.

For each incorrect full decision, record how many expected big blinds its action gives up versus the known best action. Classify the error by which standalone answers were wrong: hand only, price only, both, or neither. “Neither” means the model answered both parts correctly in isolation and still missed the full decision. These groups describe observed answers. They cannot establish the model’s internal reasoning. Compare total chip cost, counts, and costly individual hands before choosing a training target. The sample supports development decisions only.

Run `python3 -B experiments/inkling_fresh_diagnosis.py` from the repository root with the four local adapters available. Use `--rescore` to reproduce the scores offline using the committed questions and answers.
