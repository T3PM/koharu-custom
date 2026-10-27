# E2E Test Infra: Koharu Manga Suite Review

## Test Philosophy
- Opaque-box, requirement-driven test verification derived directly from `ORIGINAL_REQUEST.md` and user-facing specifications.
- Independent of internal refactoring implementation details.
- Multi-tier testing methodology: Category-Partition, Boundary Value Analysis (BVA), Pairwise Combinatorial Testing, and Real-World Workload Testing.

## Feature Inventory & Test Mapping
| # | Feature Area | Requirement Reference | Tier 1 (Happy) | Tier 2 (Boundaries) | Tier 3 (Pairwise) | Tier 4 (Workloads) |
|---|--------------|-----------------------|:--------------:|:-------------------:|:-----------------:|:------------------:|
| 1 | Schema Revision & Persistence | R1 (C1) | ≥5 | ≥5 | ✓ | ✓ |
| 2 | Patch Accumulation & Diagnostic Propagation | R1 (C2) | ≥5 | ≥5 | ✓ | ✓ |
| 3 | Inpainting Morphology & Scoping | R2 (H1, H2, H4) | ≥5 | ≥5 | ✓ | ✓ |
| 4 | Mask Filtering & Non-Destructive Detection | R2 (H3, H5) | ≥5 | ≥5 | ✓ | ✓ |
| 5 | Translation Budget & Persona Injection | R2 (M1, M4, M5) | ≥5 | ≥5 | ✓ | ✓ |
| 6 | Balloon Shape & Typesetting Binding | R2 (M2, M3, L2) | ≥5 | ≥5 | ✓ | ✓ |
| 7 | Speaker Provenance & Orphan Cleanup | R3 (H6, H7, M12) | ≥5 | ≥5 | ✓ | ✓ |
| 8 | Concurrency, Locks & Protocol Stability | R3 (H8, H9, L6, L7) | ≥5 | ≥5 | ✓ | ✓ |
| 9 | Inspector Conflict Recovery & Form State | R4 (M7, M8, L8, L9) | ≥5 | ≥5 | ✓ | ✓ |
| 10 | Locale Completeness (9 Locales) | R4 (M10) | ≥5 | ≥5 | ✓ | ✓ |

## Test Architecture & Runners
- **Rust Focused Test Runners**:
  - `cargo test -p koharu-scene`: Schema encode/decode, patch accumulation, document edit contracts.
  - `cargo test -p koharu-pipeline`: Inpainting dilation, mask filtering, scanline rasterization, detection skip, translation token budget.
  - `cargo test -p koharu-app`: Command lock concurrency, glossary speaker cleanup, revision rejection.
  - `cargo test -p koharu-translator`: Onomatopoeia matching, token budget calculators.
- **Frontend & Protocol Runners**:
  - `bun run test` in `packages/koharu`: Inspector conflict recovery, mutation queuing, controlled form state, locale completeness tests.
  - `cargo run -p koharu-app --bin generate`: Protocol generator check comparing against `packages/bridge/src/protocol.ts`.
- **Live Desktop Verification Harness**:
  - CEF remote debugging endpoint at `http://127.0.0.1:4000` via Chrome DevTools Protocol (`chrome-devtools-mcp`).
  - Chapter workflow test: multi-speaker chapter translation persona consistency and dialogue balloon font switching.

## Coverage Goals & Acceptance Thresholds
- **Tier 1 (Feature Coverage)**: ≥50 focused happy-path unit and scenario tests verifying each feature isolation contract.
- **Tier 2 (Boundary & Corner Cases)**: ≥50 boundary tests (empty layers, context overflow, stale revision rejected, missing entity cleanup, invalid polygon bounds).
- **Tier 3 (Cross-Feature Combinations)**: ≥10 integration tests exercising simultaneous interactions (e.g. patch with multiple entity creations + relations, glossary deletion during active page translation).
- **Tier 4 (Real-World Application Scenarios)**:
  - Scenario 1: Multi-character manga chapter with persona pronouns and speaker consistency.
  - Scenario 2: Dense SFX action page with complex scanline mask dilation and background text preservation.
  - Scenario 3: Rapid concurrent user edits in Inspector inducing revision conflicts and automated queue recovery.
  - Scenario 4: Full project roundtrip (legacy v1 project load -> edit -> save -> reload v2).
  - Scenario 5: Full translation pass with font preset bindings and balloon shape classification.
