# WPT reftests

The CSS reftests of web-platform-tests 0936286d874b7d5ddcb68ecf47509995c953d750 (`tools/wpt_fetch.py`) rendered by the engine at 800 x 600 and compared with their references (`tests/wpt/main.rs`). Crashes and hangs count as failures; the pass rate is of the tests that ran.

| Directory | Pass | Fail | Crash | Skip | Pass rate |
|---|---:|---:|---:|---:|---:|
| css-align | 33 | 33 | 0 | 0 | 50.0% |
| css-anchor-position | 96 | 145 | 0 | 0 | 39.8% |
| css-backgrounds | 426 | 150 | 0 | 3 | 74.0% |
| css-box | 23 | 19 | 0 | 0 | 54.8% |
| css-cascade | 28 | 23 | 0 | 0 | 54.9% |
| css-color | 277 | 44 | 0 | 0 | 86.3% |
| css-display | 144 | 77 | 0 | 4 | 65.2% |
| css-flexbox | 792 | 229 | 0 | 8 | 77.6% |
| css-grid | 1257 | 516 | 0 | 6 | 70.9% |
| css-position | 133 | 117 | 0 | 5 | 53.2% |
| css-sizing | 351 | 234 | 0 | 27 | 60.0% |
| css-text | 1037 | 526 | 0 | 0 | 66.3% |
| css-values | 85 | 112 | 0 | 17 | 43.1% |
| selectors | 178 | 50 | 0 | 1 | 78.1% |
| **total** | 4860 | 2275 | 0 | 71 | 68.1% |

| Skipped because | Tests |
|---|---:|
| animation | 1 |
| missing reference | 12 |
| missing stylesheet | 8 |
| nested document | 50 |

| Crashed with | Tests |
|---|---:|
