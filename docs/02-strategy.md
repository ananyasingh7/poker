# Strategy: baseline, exploitation, and deception

## Baseline and exploitation

One approach is to use a game-theory-optimal (GTO) strategy as a baseline, look for opponent deviations, and adjust where the expected gain warrants it. A player who folds too often can potentially be bluffed more; a player who calls too loosely can potentially be value-bet more. In one example, a supposedly strong hand may be a call against a balanced range but a fold against an opponent whose all-in range is exceptionally strong. The answer depends on the assumptions and the exact game state.

A GTO-informed baseline can be followed by cautious exploitation. Candidate tools include expert-authored poker skills, preflop charts, equity or solver tools, and potentially fine-tuning on high-quality decisions. Guidance could also address board texture, blockers, bluff selection, and bluff-catching.

A central distinction is between **outcome** and **decision quality**: a positive-EV bluff can lose, and a successful bluff need not have been the best decision. Bluffing well means choosing profitable actions and frequencies given the situation, not achieving a perfect success rate.

## Applying opponent reads

- **Underbetting for value:** If an opponent checks strong hands where a balanced strategy would often bet, their betting range may be stronger or weaker depending on what else they bet. Examine the complete range and action before adjusting marginal calls; the observed checking tendency alone does not determine the correct response.
- **Overfolding:** If an opponent folds too often to a particular bet or raise, increasing bluffs at that decision point can be profitable. For a simplified bluff risking 2 units to win 1, with no chance to win when called and no later action, the break-even fold rate is 2 / (2 + 1), or about 67%. If the opponent folds 80%, the bluff can be profitable even though it sometimes runs into a strong hand. Actual four-bet decisions also depend on calls, raises, blockers, remaining stacks, and subsequent play.
- **Overcalling:** If an opponent calls too loosely, thinner value bets can gain EV; bluffing more into that tendency can instead lose money.
- **Exceptionally strong all-in range:** If a call costs 2 units to win a pot of 3 units, it needs more than 40% equity to show immediate profit, ignoring rake. A hand such as ace-king may qualify against a sufficiently wide range but not against a very tight range of aces and kings. Exact equity and the decision depend on the cards, ranges, and pot accounting.

## Judging reads and decisions

An agent's best response depends on its estimate of an opponent's range and tendencies. It may correctly exploit one opponent while incorrectly adjusting to another; a more conservative agent may miss the first opportunity but avoid the second error. A confident explanation does not establish that an assumption is correct. Record both the observations and the uncertainty behind each read, then test the resulting decisions against known opponent policies and over sufficiently large samples. Individual outcomes cannot establish which approach has greater long-run EV.

## Our assessment

A solver or chart is tied to assumptions: game format, stack sizes, available bet sizes, rake, and ranges. Skill text or restricted actions do not automatically reproduce a solver's strategy or establish an unexploitable baseline. In multiplayer poker, equilibrium and exploitability are more complicated than the familiar two-player zero-sum setting; avoid claiming a universal GTO solution or guaranteed protection from loss.

The proposed exploitation gate is a useful hypothesis, but it needs an actual opponent model and a defensible way to estimate uncertainty and value. A threshold alone does not calculate EV. Hidden cards and selective showdowns create biased evidence, and five revealed hands are rarely a sound basis for a precise conclusion. Record opportunities as well as events, keep uncertainty visible, and test decisions against controlled opponents with known behavior.

An intentional-leak agent is interesting because it tests whether opponents adapt and whether their adaptation can be anticipated. It also costs chips, may never be noticed, and may be exploitable by more than the intended target. Compare it to an otherwise identical agent that does not make the deliberate leak. Measure the full cost and benefit over the entire session, not only the later payoff.

Separate legal-action validation from strategic restrictions. An engine must reject illegal actions; a skill system that excludes legal options changes the agent's strategy and may prevent a profitable exploit. Treat that restriction as an experimental condition, not a rules requirement.
