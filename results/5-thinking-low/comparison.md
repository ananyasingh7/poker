Arms (bb/100; chips and luck-adjusted with 95% intervals over complete seeds; costs are chips given up, lower is better)

| Arm | Seeds | Chips | Luck-adjusted | River cost | River decisions /100 hands | Turn cost (approx) | VPIP | $/hand | Fallbacks /100 hands | Truncated | Providers |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think low · none | 10 | +98 [+16, +179] | +161 [+47, +274] | 435 | 35 | 154 | 39% | 0.00032 | 1.0 | 6 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · stats | 10 | +105 [-3, +213] | +168 [+40, +296] | 360 | 32 | 126 | 36% | 0.00053 | 1.5 | 9 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · stats+hint | 8 | +178 [-44, +400] | +267 [+10, +523] | 246 | 31 | 103 | 34% | 0.00056 | 2.0 | 9 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · stats+menu | 9 | +180 [+33, +327] | +263 [+124, +403] | 340 | 33 | 127 | 36% | 0.00055 | 2.3 | 13 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · stats+hint+menu | 9 | +159 [-42, +359] | +250 [+1, +498] | 249 | 30 | 114 | 32% | 0.00056 | 3.0 | 16 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · notes | 5 | +94 [-186, +374] | +223 [-44, +489] | 313 | 28 | 118 | 32% | 0.00112 | 1.4 | 1 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · notes-every5 | 8 | +112 [-8, +232] | +157 [+4, +310] | 303 | 29 | 107 | 34% | 0.00077 | 1.0 | 4 | DeepInfra/InferenceNet/Baidu |
| deepseek-v4.1-flash-1017 · think low · oracle | 10 | +242 [+68, +416] | +297 [+102, +492] | 161 | 28 | 96 | 31% | 0.00058 | 12.7 | 76 | DeepInfra/InferenceNet/Baidu |

Costs are per 100 hands played, so a model that reaches the river less often shows a lower river cost: read them beside the river decisions and VPIP columns.

Help effects: each arm minus the same model's no-help arm, paired on shared seeds

| Arm | minus | Seeds | Chips | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think low · stats | deepseek-v4.1-flash-1017 · think low · none | 10 | +8 [-43, +58] | +8 [-43, +58] | -75 [-141, -9] | -28 [-52, -4] |
| deepseek-v4.1-flash-1017 · think low · stats+hint | deepseek-v4.1-flash-1017 · think low · none | 8 | +60 [-96, +215] | +70 [-89, +229] | -200 [-305, -95] | -38 [-75, -1] |
| deepseek-v4.1-flash-1017 · think low · stats+menu | deepseek-v4.1-flash-1017 · think low · none | 9 | +62 [-48, +172] | +75 [-33, +184] | -107 [-194, -20] | -22 [-54, +9] |
| deepseek-v4.1-flash-1017 · think low · stats+hint+menu | deepseek-v4.1-flash-1017 · think low · none | 9 | +63 [-76, +203] | +84 [-63, +231] | -183 [-292, -74] | -34 [-80, +12] |
| deepseek-v4.1-flash-1017 · think low · notes | deepseek-v4.1-flash-1017 · think low · none | 5 | +2 [-148, +152] | +5 [-140, +151] | -106 [-155, -57] | -29 [-95, +37] |
| deepseek-v4.1-flash-1017 · think low · notes-every5 | deepseek-v4.1-flash-1017 · think low · none | 8 | -6 [-72, +60] | -39 [-155, +77] | -144 [-186, -102] | -35 [-62, -9] |
| deepseek-v4.1-flash-1017 · think low · oracle | deepseek-v4.1-flash-1017 · think low · none | 10 | +144 [+27, +262] | +137 [-31, +304] | -274 [-435, -113] | -58 [-104, -11] |
