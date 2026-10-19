# Current interface validation

Checked on 2026-10-18 with the unified interface. Historical results in `history/` apply only to their recorded artifacts.

## Host checks

- Linux x86_64, Rust 1.90.0: `python3 tools/check.py` passed 22 Rust tests and 5 package-check tests, built all six external C/C++ consumers with warnings as errors, and verified 17 exported functions against the current header.
- C99/C++11 basic examples, sorting/aggregation example, writer create/reopen/readonly and combined acceptance create/reopen/readonly all passed. Read-only runs preserved the complete index file sets, lengths, SHA-256 hashes and modification times.
- Windows GNU: 21 of 22 Rust tests passed. The remaining test failed during a committed write with OS error 5 (access denied opening a `.term` file). No retry or test skip was added. The Windows library and all six C/C++ consumers built; the C99, C++11 and advanced examples ran successfully.
- Complete C examples extracted from both READMEs compiled with `-std=c99 -Wall -Wextra -Werror -fsyntax-only`. README relative links and JSON blocks were checked.

## HarmonyOS checks

Built with Rust 1.90.0, Tantivy 0.26.2, jieba-rs 0.8.1 and OpenHarmony Native SDK API 26 (26.0.0.105). All six C/C++ consumers cross-compiled. The shared library exports the same 17 current functions; no numbered API symbols remain.

On an idle MateBook Pro S, an isolated `com.torry2022.retrievalprobe` application ran create, reopen and readonly in separate processes. All three reported PASS with empty error logs. These cover persisted default/extended dictionaries, mutations, multi-page sorting with missing values, terms/stats/range aggregations and validation errors, optional exact counts, retained result data, finite/overflow scores, two-worker creation and three-worker reopening with distinct memory budgets. Readonly rejects writer options and performs queries without a writer.

The build library SHA-256 is `453584f16e85a687cdb38945004f4c5fb49286f9477c9d05e967acef5cb84c1d`. DevEco applies `llvm-strip --strip-all` during HAP packaging; the extracted HAP library exactly matched that stripped build, SHA-256 `534cc3bc55617ecf4550299992456149b76435fb192e24cf02fa9f309d694586`.

The device shell could read probe reports but could not enumerate private index directories, so whole-directory hash/mtime immutability was verified on the Linux host, not asserted for the device. The existing chat process remained running with unchanged CPU counters throughout device checks. No chat App or model was changed.

The independent application can be prepared with `prepare_api_smoke.ps1`; keep signing configuration local and use a new runId for each trial. Logs, hashes and host reports for this check remain locally under `build/`. No new performance claim or minimum supported OS version is inferred from these functional checks.

## Release package integration — 2026-10-19

The release candidate ZIP was extracted into a separate directory. All six C/C++ consumers were cross-compiled using only its source, public header and shared library, with warnings treated as errors. The SDK toolchain's unused `--gcc-toolchain` argument warning was disabled for this consumer build; source warnings remained errors.

An independent application built from the extracted sample ran create, reopen and readonly in separate processes on an idle MateBook Pro S (HarmonyOS 7.0.0.111, API 26). All three phases passed, including writer options, paging, projections, sorting, aggregations and score validation; error logs were empty. Local signing configuration was supplied separately and is excluded from the SDK. No chat App was installed, restarted or changed.

The extracted library has the same `453584f16e85a687cdb38945004f4c5fb49286f9477c9d05e967acef5cb84c1d` SHA-256 as the previously validated build. Its 17 exported functions match the packaged header, which is identical to the packaged source header. The HAP's library matches the SDK library after DevEco's stripping step. Whole-index read-only immutability remains covered by the Linux host checks above; it was not asserted for the private device sandbox.

The archive checker verified the file checksum inventory, required source/examples/licenses, matching headers and AArch64 library hash. These checks cover the current package and tested configuration, not older HarmonyOS releases or a minimum supported system version.
