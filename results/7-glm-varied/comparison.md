Arms (bb/100; chips and luck-adjusted with 95% intervals over complete seeds; costs are chips given up, lower is better)

| Arm | Seeds | Chips | Luck-adjusted | River cost | River decisions /100 hands | Turn cost (approx) | VPIP | $/hand | Fallbacks /100 hands | Truncated | Providers |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | +22 [-111, +155] | +30 [-64, +125] | 438 | 29 | 243 | 44% | 0.00007 | 0.5 | 0 | Wafer/Darkbloom |
| glm-5.3-flash · think low · none+features · varied (personalities, mix 0.1, random field) bots | 20 | +173 [+11, +334] | +131 [+3, +259] | 369 | 24 | 167 | 40% | 0.00007 | 0.0 | 0 | Wafer/Darkbloom |
| glm-5.3-flash · think low · oracle · varied (personalities, mix 0.1, random field) bots | 20 | +98 [-26, +223] | +84 [-47, +215] | 316 | 24 | 267 | 41% | 0.00011 | 0.3 | 0 | Wafer/Darkbloom |
| glm-5.3-flash · think low · stats+sharp · varied (personalities, mix 0.1, random field) bots | 20 | +98 [-76, +271] | +158 [-21, +336] | 301 | 28 | 253 | 48% | 0.00015 | 0.6 | 0 | Wafer/Darkbloom |
| glm-5.3-flash · think low · notes-every5 · varied (personalities, mix 0.1, random field) bots | 20 | +104 [-35, +243] | +111 [-28, +250] | 489 | 31 | 247 | 48% | 0.00015 | 0.3 | 0 | Wafer/Darkbloom |
| deepseek-v4.1-flash-1017 · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | +148 [+51, +245] | +137 [+42, +231] | 401 | 24 | 181 | 36% | 0.00032 | 1.7 | 20 | DeepInfra/InferenceNet/Baidu |

Costs are per 100 hands played, so a model that reaches the river less often shows a lower river cost: read them beside the river decisions and VPIP columns.

Learning within a session: last 20 hands minus first 20, per seed (chips up and costs down mean the arm improved; the cards differ by hand number, so compare an arm with the no-help arm below)

| Arm | Seeds | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- |
| glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | +58 [-218, +335] | +336 [+85, +586] | -45 [-150, +61] |
| glm-5.3-flash · think low · none+features · varied (personalities, mix 0.1, random field) bots | 20 | -37 [-423, +350] | +24 [-187, +235] | +0 [-150, +151] |
| glm-5.3-flash · think low · oracle · varied (personalities, mix 0.1, random field) bots | 20 | -75 [-501, +351] | -6 [-218, +205] | -102 [-268, +63] |
| glm-5.3-flash · think low · stats+sharp · varied (personalities, mix 0.1, random field) bots | 20 | +32 [-380, +445] | +78 [-183, +339] | +7 [-95, +109] |
| glm-5.3-flash · think low · notes-every5 · varied (personalities, mix 0.1, random field) bots | 20 | +318 [-68, +704] | +128 [-192, +448] | -25 [-119, +70] |
| deepseek-v4.1-flash-1017 · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | -7 [-297, +283] | +331 [+84, +578] | -26 [-156, +103] |

Learning relative to no help: (late minus early) minus the no-help arm's (late minus early), paired on shared seeds

| Arm | minus | Seeds | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- | --- |
| glm-5.3-flash · think low · none+features · varied (personalities, mix 0.1, random field) bots | glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | -95 [-418, +228] | -312 [-532, -92] | +45 [-80, +169] |
| glm-5.3-flash · think low · oracle · varied (personalities, mix 0.1, random field) bots | glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | -133 [-431, +164] | -342 [-580, -105] | -58 [-222, +107] |
| glm-5.3-flash · think low · stats+sharp · varied (personalities, mix 0.1, random field) bots | glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | -26 [-404, +352] | -258 [-512, -3] | +51 [-89, +192] |
| glm-5.3-flash · think low · notes-every5 · varied (personalities, mix 0.1, random field) bots | glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | +259 [-52, +570] | -208 [-503, +88] | +20 [-83, +123] |

Help effects: each arm minus the same model's no-help arm, paired on shared seeds

| Arm | minus | Seeds | Chips | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- | --- | --- |
| glm-5.3-flash · think low · none+features · varied (personalities, mix 0.1, random field) bots | glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | +151 [+0, +301] | +101 [-16, +219] | -69 [-174, +35] | -76 [-137, -16] |
| glm-5.3-flash · think low · oracle · varied (personalities, mix 0.1, random field) bots | glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | +76 [-105, +258] | +54 [-106, +214] | -122 [-249, +5] | +24 [-35, +83] |
| glm-5.3-flash · think low · stats+sharp · varied (personalities, mix 0.1, random field) bots | glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | +76 [-68, +219] | +127 [-6, +261] | -137 [-228, -46] | +10 [-41, +60] |
| glm-5.3-flash · think low · notes-every5 · varied (personalities, mix 0.1, random field) bots | glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | +82 [-90, +254] | +81 [-73, +234] | +51 [-66, +168] | +4 [-59, +67] |

Model differences: each arm minus the first model in the same condition, paired on shared seeds

| Arm | minus | Seeds | Chips | Luck-adjusted | River cost | Turn cost |
| --- | --- | --- | --- | --- | --- | --- |
| deepseek-v4.1-flash-1017 · think low · none · varied (personalities, mix 0.1, random field) bots | glm-5.3-flash · think low · none · varied (personalities, mix 0.1, random field) bots | 20 | +126 [+23, +228] | +106 [+23, +190] | -37 [-159, +85] | -62 [-145, +20] |
