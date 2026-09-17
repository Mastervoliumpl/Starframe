# Bounded malformed-input campaigns

Issue [#47](https://github.com/Mastervoliumpl/Starframe/issues/47) adds deterministic mutation targets for catalog JSON, activation/capability/report contracts, relative paths and ZIP extraction. The harness calls the production readers and extractor. It uses the existing Rust test target and dependencies; it adds no runtime code or fuzzing dependency.

## Checks and limits

The corpus uses the shared contract fixtures, a synthetic catalog and ZIPs containing inert files. Mutations change bytes, truncate documents, insert malformed JSON/path/ZIP tokens and append data. One in sixteen campaign cases runs an unchanged seed. Accepted catalogs and contracts must survive serialization and revalidation. Accepted paths must remain relative without traversal or stream separators. ZIP extraction must preserve a sibling sentinel and produce files inside its destination with the recorded sizes and hashes.

Each input is capped at 65,536 bytes. A campaign stops at its requested duration (1–900 seconds) or one million cases, whichever comes first. The Python runner gives the test process another 30 seconds before terminating a timed-out process; compilation has a separate 15-minute timeout. ZIP metadata, entry count, expanded-size and runtime readiness limits remain enforced by the production code and its existing boundary regression tests. This harness does not impose a separate resident-memory quota.

The Windows stress test races an ordinary directory against a junction to another synthetic directory while the production directory-pinning function opens it. A successful pin must prevent its name from being moved. The test runs for three seconds, requires successful opens and checks the external sentinel afterward. It exercises concurrent directory replacement; it does not exhaust every NTFS, network-share or device-removal interleaving. It uses only new temporary fixtures.

## Running campaigns

Run the ordinary deterministic corpus smoke check through `npm run check:rust`. The longer campaign and Windows race are ignored by the ordinary test invocation. After the normal frontend build:

```powershell
python scripts/run_fuzz.py --seconds 300 --seed 47
```

The [manual workflow](../../.github/workflows/fuzz.yml) runs the same command on Windows and Linux. It has no push, PR or scheduled trigger, no release credentials and no write permission to the repository. It can be dispatched once the workflow is available on the default branch. Adding this workflow does not establish a hosted campaign result.

Evidence is retained in a unique directory beneath ignored `test-results/fuzz`. `run.json` identifies the starting commit, dirty-tree state, seed and requested duration. `summary.json` records case counts and accepted counts per target. A panic retains `failure.bin` and the target/iteration in `failure.json`; rerun the same seed against the same code to reproduce it. Timeouts or process-level failures can retain only the seed and logs, so preserve the evidence before changing the corpus or implementation. Promote confirmed failures to small deterministic regression cases before fixing them. Raw local logs can contain paths; the hosted workflow uploads only synthetic inputs and JSON reports, for 14 days.

## Interpretation

These are bounded mutation campaigns, not coverage-guided libFuzzer runs. They do not establish code coverage, prove the absence of vulnerabilities or replace review of signing, curation and update authority. They cover the Rust contract readers; C# runtime parsers remain covered by the separate contract tests. Findings and remaining platform gaps must be reviewed again with #30 before public alpha distribution.

The local corpus smoke check and the initial Windows replacement run passed on 9 September 2026. The latter recorded 193,135 attempts and 527 successful pins, with no escaped pin or changed sentinel.

The 300-second campaign with seed 47 completed 946,476 cases without a panic or failed invariant. Counts below show total inputs and accepted inputs; rejection of malformed input is expected.

| Target | Cases | Accepted |
| --- | ---: | ---: |
| Activation | 605,527 | 5,016 |
| Capabilities | 66,111 | 722 |
| Catalog | 10,861 | 761 |
| Relative paths | 54,801 | 6,729 |
| Session report | 154,389 | 2,102 |
| ZIP extraction | 54,787 | 845 |

The accompanying Windows race passed 202,640 attempts and 507 successful pins. No failing input was found to promote into a regression fixture. The campaign ran against the new harness before commit, on base `8b19e7a`; its run record correctly marks the working tree dirty. Rust formatting, all-target Clippy and the full regression suite are checked separately. These local results do not claim a hosted Windows/Linux campaign; the manual workflow still needs execution after it becomes available on the default branch.
