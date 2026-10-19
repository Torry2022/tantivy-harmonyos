# Changelog

## v0.26.2-ohos.3 — 2026-10-19

- Consolidate creation, opening, typed writes, search and aggregations into one C interface. Retire the numbered APIs; callers must update their code and rebuild with the matching library/header.
- Add per-handle writer worker count and total memory budget. Defaults remain one worker and 15000000 bytes; saved index formats are unchanged.
- Provide current C/C++ consumers and Linux CI with functional, export, package and read-only checks.
- Rewrite both READMEs around building and integrating the SDK.

## v0.26.2-ohos.2 (retired interface)

- Configurable typed fields, persisted dictionary extensions, field sorting, aggregations, projections, pagination and exact counts.
- Shared immutable dictionaries and ID-only search optimization.

## v0.26.2-ohos.1 (retired interface)

- Initial HarmonyOS arm64 library for persistent Chinese full-text search.
