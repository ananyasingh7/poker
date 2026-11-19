Arms (bb/100; chips and luck-adjusted with 95% intervals over complete seeds; costs are chips given up, lower is better)

| Arm | Seeds | Chips | Luck-adjusted | River cost | River decisions /100 hands | Turn cost (approx) | VPIP | $/hand | Fallbacks /100 hands | Truncated | Providers |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think low · none | 20 | +175 [+87, +264] | +207 [+116, +298] | 364 | 34 | 142 | 36% | 0.00032 | 1.3 | 16 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · stats+hint | 18 | +292 [+154, +430] | +325 [+184, +465] | 225 | 31 | 115 | 32% | 0.00055 | 2.1 | 22 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · stats+sharp | 10 | +486 [+312, +659] | +476 [+296, +657] | 151 | 29 | 93 | 30% | 0.00049 | 1.2 | 7 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think off · none | 10 | +15 [-63, +93] | +19 [-54, +91] | 577 | 36 | 149 | 36% | 0.00011 | 0.0 | 0 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think off · stats+hint | 10 | -8 [-122, +106] | +24 [-57, +104] | 563 | 32 | 134 | 32% | 0.00023 | 0.0 | 0 | DeepInfra/InferenceNet/Baidu |
| glm-5.3-flash · think low · none | 20 | +180 [+79, +280] | +191 [+92, +289] | 433 | 34 | 191 | 40% | 0.00006 | 0.7 | 0 | Wafer/Darkbloom |
| glm-5.3-flash · think low · stats+hint | 20 | +334 [+164, +503] | +373 [+201, +546] | 342 | 35 | 237 | 46% | 0.00015 | 0.7 | 0 | Wafer/Darkbloom |
| gpt-oss-20b · think low · none | 18 | -744 [-984, -504] | -725 [-911, -539] | 1200 | 73 | 478 | 87% | 0.00007 | 14.9 | 6 | Darkbloom/AkashML/DeepInfra |
| gpt-oss-20b · think low · stats+hint | 20 | -221 [-456, +14] | -31 [-147, +85] | 43 | 4 | 66 | 43% | 0.00006 | 11.3 | 8 | Darkbloom/AkashML/DeepInfra |
| gemma-4-26b-a4b-it · think off · none | 10 | -10 [-64, +44] | -10 [-64, +44] | 662 | 40 | 231 | 47% | 0.00007 | 0.3 | 0 | Darkbloom/AkashML |
| gemma-4-26b-a4b-it · think off · stats+hint | 5 | -47 [-216, +122] | -47 [-216, +122] | 933 | 45 | 250 | 56% | 0.00014 | 14.4 | 0 | Darkbloom/AkashML |

Costs are per 100 hands played, so a model that reaches the river less often shows a lower river cost: read them beside the river decisions and VPIP columns.

Learning within a session: last 20 hands minus first 20, per seed (chips up and costs down mean the arm improved; the cards differ by hand number, so compare an arm with the no-help arm below)

| Arm | Seeds | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think low · none | 20 | -46 [-335, +243] | -87 [-379, +204] | -49 [-104, +5] |
| deepseek-v4.1-flash-1017 · think low · stats+hint | 18 | -194 [-605, +217] | -45 [-244, +155] | -55 [-117, +8] |
| deepseek-v4.1-flash-1017 · think low · stats+sharp | 10 | -130 [-802, +541] | -200 [-385, -15] | -36 [-130, +58] |
| deepseek-v4.1-flash-1017 · think off · none | 10 | -98 [-209, +12] | -259 [-894, +375] | -175 [-309, -40] |
| deepseek-v4.1-flash-1017 · think off · stats+hint | 10 | -117 [-326, +93] | -209 [-1009, +592] | -114 [-268, +40] |
| glm-5.3-flash · think low · none | 20 | -154 [-430, +122] | +31 [-251, +313] | -5 [-88, +78] |
| glm-5.3-flash · think low · stats+hint | 20 | -195 [-674, +283] | -122 [-312, +69] | +20 [-72, +113] |
| gpt-oss-20b · think low · none | 18 | -143 [-718, +432] | +49 [-263, +361] | +14 [-147, +174] |
| gpt-oss-20b · think low · stats+hint | 20 | +162 [-151, +474] | -3 [-91, +84] | +24 [-97, +146] |
| gemma-4-26b-a4b-it · think off · none | 10 | -123 [-287, +42] | -313 [-998, +371] | -75 [-355, +204] |
| gemma-4-26b-a4b-it · think off · stats+hint | 5 | -101 [-553, +351] | -246 [-1484, +992] | -180 [-739, +379] |

Learning relative to no help: (late minus early) minus the no-help arm's (late minus early), paired on shared seeds

| Arm | minus | Seeds | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think low · stats+hint | deepseek-v4.1-flash-1017 · think low · none | 18 | -88 [-382, +206] | +71 [-137, +280] | +8 [-72, +88] |
| deepseek-v4.1-flash-1017 · think low · stats+sharp | deepseek-v4.1-flash-1017 · think low · none | 10 | +142 [-144, +428] | -40 [-460, +379] | +10 [-93, +113] |
| deepseek-v4.1-flash-1017 · think off · stats+hint | deepseek-v4.1-flash-1017 · think off · none | 10 | -18 [-223, +187] | +51 [-210, +311] | +61 [-79, +200] |
| glm-5.3-flash · think low · stats+hint | glm-5.3-flash · think low · none | 20 | -42 [-486, +403] | -153 [-481, +175] | +25 [-76, +127] |
| gpt-oss-20b · think low · stats+hint | gpt-oss-20b · think low · none | 18 | +278 [-247, +803] | -53 [-396, +291] | +21 [-130, +173] |
| gemma-4-26b-a4b-it · think off · stats+hint | gemma-4-26b-a4b-it · think off · none | 5 | +52 [-125, +229] | +628 [-189, +1445] | -169 [-778, +440] |

Help effects: each arm minus the same model's no-help arm, paired on shared seeds

| Arm | minus | Seeds | Chips | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think low · stats+hint | deepseek-v4.1-flash-1017 · think low · none | 18 | +99 [+5, +193] | +97 [+1, +193] | -136 [-213, -59] | -20 [-41, +2] |
| deepseek-v4.1-flash-1017 · think low · stats+sharp | deepseek-v4.1-flash-1017 · think low · none | 10 | +232 [+31, +434] | +223 [+18, +429] | -142 [-265, -18] | -38 [-72, -4] |
| deepseek-v4.1-flash-1017 · think off · stats+hint | deepseek-v4.1-flash-1017 · think off · none | 10 | -22 [-122, +78] | +5 [-68, +77] | -14 [-77, +49] | -14 [-55, +27] |
| glm-5.3-flash · think low · stats+hint | glm-5.3-flash · think low · none | 20 | +154 [+22, +286] | +183 [+32, +334] | -91 [-191, +9] | +46 [+5, +87] |
| gpt-oss-20b · think low · stats+hint | gpt-oss-20b · think low · none | 18 | +537 [+178, +896] | +697 [+499, +894] | -1153 [-1369, -937] | -421 [-501, -341] |
| gemma-4-26b-a4b-it · think off · stats+hint | gemma-4-26b-a4b-it · think off · none | 5 | -63 [-151, +25] | -63 [-151, +25] | +267 [-209, +742] | +34 [-285, +354] |

Model differences: each arm minus the first model in the same condition, paired on shared seeds

| Arm | minus | Seeds | Chips | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think off · none | deepseek-v4.1-flash-1017 · think low · none | 10 | -238 [-434, -43] | -234 [-423, -45] | +284 [+88, +481] | +18 [-52, +87] |
| deepseek-v4.1-flash-1017 · think off · stats+hint | deepseek-v4.1-flash-1017 · think low · stats+hint | 10 | -391 [-612, -171] | -348 [-567, -128] | +356 [+156, +555] | +11 [-36, +58] |
| glm-5.3-flash · think low · none | deepseek-v4.1-flash-1017 · think low · none | 20 | +4 [-85, +94] | -16 [-109, +76] | +69 [-41, +179] | +48 [+15, +82] |
| glm-5.3-flash · think low · stats+hint | deepseek-v4.1-flash-1017 · think low · stats+hint | 18 | +115 [+5, +224] | +131 [+26, +236] | +105 [+17, +193] | +121 [+85, +157] |
| gpt-oss-20b · think low · none | deepseek-v4.1-flash-1017 · think low · none | 18 | -901 [-1167, -635] | -917 [-1117, -717] | +857 [+690, +1024] | +336 [+266, +405] |
| gpt-oss-20b · think low · stats+hint | deepseek-v4.1-flash-1017 · think low · stats+hint | 18 | -518 [-782, -255] | -369 [-545, -193] | -180 [-262, -99] | -48 [-91, -5] |
| gemma-4-26b-a4b-it · think off · none | deepseek-v4.1-flash-1017 · think low · none | 10 | -263 [-412, -115] | -263 [-412, -115] | +370 [+247, +492] | +100 [-17, +217] |
| gemma-4-26b-a4b-it · think off · stats+hint | deepseek-v4.1-flash-1017 · think low · stats+hint | 5 | -478 [-852, -105] | -456 [-858, -54] | +687 [-90, +1464] | +122 [-45, +289] |
