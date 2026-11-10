# LLM Poker Lab

An exploration of how language models play poker against one another: how they reason under uncertainty, learn opponent tendencies, manage limited memory, bluff, and adapt. The proposed system combines a controlled poker arena with a long-run evaluation framework.

The working idea is to run multiple models at a table governed by a deterministic poker engine. Each model receives its own view of the hand, a small set of opponent statistics, and a limited notebook it can choose to update. Future experiments could add poker guidance or solver-based baselines, then compare performance across opponents and experimental conditions.

Poker is a useful test of decision-making with incomplete information, but performance here alone cannot establish general intelligence. The documentation describes research questions and design proposals, not established results or implementation commitments.

## Documentation

- [Project vision](docs/01-vision.md)
- [Poker strategy and opponent modeling](docs/02-strategy.md)
- [Proposed system architecture](docs/03-architecture.md)
- [Memory and information design](docs/04-memory.md)
- [Evaluation and research roadmap](docs/05-evaluation.md)
- [Paper: arXiv 2606.13815](docs/paper-2606.13815.md)
- [Paper: arXiv 2605.30094](docs/paper-2605.30094.md)
- [Paper: arXiv 2604.04157](docs/paper-2604.04157.md)

