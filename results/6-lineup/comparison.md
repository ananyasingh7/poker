Arms (bb/100; chips and luck-adjusted with 95% intervals over complete seeds; costs are chips given up, lower is better)

| Arm | Seeds | Chips | Luck-adjusted | River cost | River decisions /100 hands | Turn cost (approx) | VPIP | $/hand | Fallbacks /100 hands | Truncated | Providers |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think low · none | 20 | +175 [+87, +264] | +207 [+116, +298] | 362 | 34 | 140 | 36% | 0.00032 | 1.3 | 16 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · stats+hint | 18 | +292 [+154, +430] | +325 [+184, +465] | 224 | 31 | 114 | 32% | 0.00055 | 2.1 | 22 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · stats+sharp | 10 | +486 [+312, +659] | +476 [+296, +657] | 146 | 29 | 93 | 30% | 0.00049 | 1.2 | 7 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think off · none | 10 | +15 [-63, +93] | +19 [-54, +91] | 574 | 36 | 148 | 36% | 0.00011 | 0.0 | 0 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think off · stats+hint | 10 | -8 [-122, +106] | +24 [-57, +104] | 559 | 32 | 135 | 32% | 0.00023 | 0.0 | 0 | DeepInfra/InferenceNet/Baidu |
| glm-5.3-flash · think low · none | 20 | +180 [+79, +280] | +191 [+92, +289] | 434 | 34 | 190 | 40% | 0.00006 | 0.7 | 0 | Wafer/Darkbloom |
| glm-5.3-flash · think low · stats+hint | 20 | +334 [+164, +503] | +373 [+201, +546] | 342 | 35 | 239 | 46% | 0.00015 | 0.7 | 0 | Wafer/Darkbloom |
| gpt-oss-20b · think low · none | 18 | -744 [-984, -504] | -725 [-911, -539] | 1196 | 73 | 475 | 87% | 0.00007 | 14.9 | 6 | Darkbloom/AkashML/DeepInfra |
| gpt-oss-20b · think low · stats+hint | 20 | -221 [-456, +14] | -31 [-147, +85] | 45 | 4 | 67 | 43% | 0.00006 | 11.3 | 8 | Darkbloom/AkashML/DeepInfra |
| gemma-4-26b-a4b-it · think off · none | 10 | -10 [-64, +44] | -10 [-64, +44] | 659 | 40 | 227 | 47% | 0.00007 | 0.3 | 0 | Darkbloom/AkashML |
| gemma-4-26b-a4b-it · think off · stats+hint | 5 | -47 [-216, +122] | -47 [-216, +122] | 914 | 45 | 245 | 56% | 0.00014 | 14.4 | 0 | Darkbloom/AkashML |

Costs are per 100 hands played, so a model that reaches the river less often shows a lower river cost: read them beside the river decisions and VPIP columns.

Help effects: each arm minus the same model's no-help arm, paired on shared seeds

| Arm | minus | Seeds | Chips | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think low · stats+hint | deepseek-v4.1-flash-1017 · think low · none | 18 | +99 [+5, +193] | +97 [+1, +193] | -135 [-213, -57] | -19 [-40, +2] |
| deepseek-v4.1-flash-1017 · think low · stats+sharp | deepseek-v4.1-flash-1017 · think low · none | 10 | +232 [+31, +434] | +223 [+18, +429] | -142 [-265, -19] | -34 [-67, -1] |
| deepseek-v4.1-flash-1017 · think off · stats+hint | deepseek-v4.1-flash-1017 · think off · none | 10 | -22 [-122, +78] | +5 [-68, +77] | -15 [-76, +46] | -13 [-58, +32] |
| glm-5.3-flash · think low · stats+hint | glm-5.3-flash · think low · none | 20 | +154 [+22, +286] | +183 [+32, +334] | -92 [-190, +6] | +49 [+7, +90] |
| gpt-oss-20b · think low · stats+hint | gpt-oss-20b · think low · none | 18 | +537 [+178, +896] | +697 [+499, +894] | -1146 [-1359, -934] | -417 [-499, -336] |
| gemma-4-26b-a4b-it · think off · stats+hint | gemma-4-26b-a4b-it · think off · none | 5 | -63 [-151, +25] | -63 [-151, +25] | +259 [-211, +730] | +25 [-305, +354] |

Model differences: each arm minus the first model in the same condition, paired on shared seeds

| Arm | minus | Seeds | Chips | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think off · none | deepseek-v4.1-flash-1017 · think low · none | 10 | -238 [-434, -43] | -234 [-423, -45] | +286 [+92, +479] | +21 [-51, +93] |
| deepseek-v4.1-flash-1017 · think off · stats+hint | deepseek-v4.1-flash-1017 · think low · stats+hint | 10 | -391 [-612, -171] | -348 [-567, -128] | +353 [+161, +546] | +12 [-37, +60] |
| glm-5.3-flash · think low · none | deepseek-v4.1-flash-1017 · think low · none | 20 | +4 [-85, +94] | -16 [-109, +76] | +72 [-40, +184] | +50 [+17, +82] |
| glm-5.3-flash · think low · stats+hint | deepseek-v4.1-flash-1017 · think low · stats+hint | 18 | +115 [+5, +224] | +131 [+26, +236] | +107 [+18, +196] | +124 [+87, +161] |
| gpt-oss-20b · think low · none | deepseek-v4.1-flash-1017 · think low · none | 18 | -901 [-1167, -635] | -917 [-1117, -717] | +856 [+691, +1022] | +335 [+264, +406] |
| gpt-oss-20b · think low · stats+hint | deepseek-v4.1-flash-1017 · think low · stats+hint | 18 | -518 [-782, -255] | -369 [-545, -193] | -176 [-256, -97] | -46 [-89, -3] |
| gemma-4-26b-a4b-it · think off · none | deepseek-v4.1-flash-1017 · think low · none | 10 | -263 [-412, -115] | -263 [-412, -115] | +371 [+241, +501] | +101 [-18, +220] |
| gemma-4-26b-a4b-it · think off · stats+hint | deepseek-v4.1-flash-1017 · think low · stats+hint | 5 | -478 [-852, -105] | -456 [-858, -54] | +682 [-88, +1451] | +115 [-49, +279] |
