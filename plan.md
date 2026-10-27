# แผนแก้ findings — AGY-CR-20261026-MANGA-SUITE-001

**ที่มา:** `.perch/CODE_REVIEW_MANGA_SUITE.md` (review 2026-10-27)
**Base:** `61b5421d` · working tree 62 files, +4486/−758
**สถานะปัจจุบัน:** `cargo check --workspace` PASS · `bun run test` PASS 156/156 · ยังไม่ควร merge

หลักการ: แก้ตามลำดับความรุนแรง, ไม่สร้าง backward-compat layer, ทุก mutation ผ่าน Patch + `expectedRevision`, domain logic อยู่ใน scene/pipeline ไม่ใช่ใน RPC handler

---

## Stage 0 — ตั้งหลัก (ไม่แก้โค้ด)

- [x] ยืนยัน git state ยังอยู่ที่ `61b5421d` + working tree ชุดเดิม
- [x] ตัดสินใจ 2 เรื่องที่ต้องเป็นนโยบายก่อนลงมือ:
  - **H3:** non-destructive re-detection รักษาคำแปลเดิมและไม่ลบ downstream text layers
  - **M9:** revert `Run phase` กลับเป็น `through` และ revert Select-All-Layers กลับเป็น `selectableLayer`
- [x] สร้าง branch แยกสำหรับงานแก้ (`fix/manga-suite-review`)

---

## Stage 1 — Critical (บล็อกการเปิดโปรเจกต์เก่า + ความถูกต้องของ patch)

### 1.1 C1 · bump `GlossaryEntry` schema revision
- `crates/koharu-scene/src/components/glossary.rs:58-73`
- เปลี่ยนเป็น `#[revisioned(revision = 2)]` + `#[revision(start = 2)]` ที่ฟิลด์ `character`
- ตาม convention เดิมใน `components/structure.rs:43,49`
- อัปเดต schema-revision test (`tests.rs:104-136`) ให้ครอบคลุม `Glossary` / `GlossaryEntry`
- เพิ่ม **decode regression test**: อ่าน persisted record schema-1 แล้วต้องได้ `character = None` ไม่ panic

### 1.2 C2 · `Patch::prune_missing` ต้อง validate กับ state ที่ build สะสม
- `crates/koharu-scene/src/patch.rs:503-568`
- เปลี่ยน `state.contains_entity(...)` → ใช้ `next_state` ที่ apply ทับกันไปทีละ op
- เลิกกลืน error: `let _ = op.apply(&mut next_state);` → เก็บ diagnostic หรือ propagate
- เพิ่ม test: patch ที่ `ensure_text_group` + `add_text_layer` ใต้ group ใหม่ + `InsertRelation` ใน patch เดียว ต้องรอดทั้งหมด

**Verify:** `cargo test -p koharu-scene`

---

## Stage 2 — High ฝั่ง pipeline (คุณภาพ inpaint + data loss)

### 2.1 H1 · dilation ต้องเกิดก่อน region clipping
- `crates/koharu-pipeline/src/stages/inpainting.rs:557-586`
- สลับลำดับ: dilate → clip ตาม `input.region` (หรือ re-clip หลัง dilate)
- เพิ่ม test: region-scoped inpaint บนหน้า SFX ต้องไม่แตะ pixels นอก region

### 2.2 H2 · dilation ต่อ SFX layer ไม่ใช่ทั้งหน้า
- แยก mask ของ layer ที่มี role `dev.koharu.text.sfx` ออกมา dilate เฉพาะส่วนนั้น
- ครอบคลุม standalone `TextLayout` ด้วย (ให้ตรงกับ `collect_translation_targets` ที่ `translation.rs:365-370`)
- ตัดสินใจ: จะคง TextRole string หรือเพิ่ม `LayerKind::Sfx` จริงตาม AC-P3-1 (ตอนนี้ grep = 0 hits)

### 2.3 H5 · `write_mask` ต้องกรอง kind
- `crates/koharu-pipeline/src/stages/detection.rs:1678-1697`
- เพิ่มเงื่อนไขแบบเดียวกับ `inpainting.rs:545-548`: เฉพาะ `TextLayout` หรือ `Region.kind == TextRegion::kind()`
- เพิ่ม test: user-drawn panel/bubble region ต้องไม่ถูกเผาเข้า text mask

### 2.4 H4 · แทน `fill_polygon_on_mask` ด้วย scanline fill
- `inpainting.rs:434-475` — เลิก point-in-polygon ต่อ pixel
- วัดผลก่อน/หลังบน input ขนาดจริง (ตามกฎ Performance: รายงาน device + input size + baseline + result)

### 2.5 H3 · ลงมตินโยบาย re-detection
- ถ้าเลือก full reprocess: เขียน doc กำกับพฤติกรรม + เตือน user ใน UI
- ถ้าเลือกเก็บของเดิม: คืน `skip()` และเลิก cascade ลบ generated layers

**Verify:** `cargo test -p koharu-pipeline`

---

## Stage 3 — High ฝั่ง scene/app (referential integrity + provenance)

### 3.1 H6 · cleanup orphaned Speaker ให้ครบทุกที่อยู่
- `crates/koharu-app/src/commands/glossary/domain.rs:360-375`
- ไล่ทั้ง `text_layers()` **และ** text content entities (schema อนุญาตที่ `schema.rs:199`, app อ่านที่ `project.rs`)
- เพิ่ม test ตาม Negative Case ใน CR Section 8: ลบ entry ที่ผูกเป็น Speaker → ต้องเป็น `None` ไม่ panic

### 3.2 H7 · `set_speaker` ต้องรับ origin
- `crates/koharu-scene/src/document.rs:295-302` — เลิก hardcode `Origin::User`
- ใช้ `Speaker::new(speaker, origin)` (`character.rs:70`) ที่ตอนนี้เป็น dead code
- ฝั่ง pipeline (`translation.rs:272`) และ `guess_page_speakers` ส่ง origin แบบ generated

### 3.3 M12 · แยก `remove_entity_if_present`
- `crates/koharu-scene/src/edit.rs:202-205` — คืน `EntityNotFound` ให้ `remove_entity`, ทำ variant `_if_present` สำหรับ `delete_pages`

### 3.4 L5 · ลบ one-use pass-through
- `Edit::set_balloon_classification` (`document.rs:305-311`), `resolve_typesetting_defaults`

**Verify:** `cargo test -p koharu-scene`, `cargo test -p koharu-app`

---

## Stage 4 — High ฝั่ง RPC/IPC (lock discipline + generator)

### 4.1 H8 · ย้าย domain logic ออกจาก commands + เลิกถือ lock ข้าม network
- `classify_glossary_entries` (`glossary/rpc.rs:588-708`) และ `translate_glossary_entries` (`rpc.rs:509-584`)
  - pattern: เก็บ IDs ภายใต้ lock → ปล่อย lock → เรียก HTTP → กลับมา patch ด้วย revision check
  - ย้าย heuristic (honorific tables, katakana rules) ไป koharu-scene/koharu-translator
  - ลบ `JP_HONORIFICS` ที่ซ้ำ (rpc.rs:603-608 vs 239-242) ให้เหลือที่เดียว
- `set_balloon_shape` (`project.rs:752-826`): ย้าย shape→preset mapping ไป koharu-renderer/scene, เลิก hardcode white stroke fallback (line 816)
- `guess_page_speakers` (`project.rs:828-892`): ย้าย heuristic ไป scene

### 4.2 H9 · regenerate `protocol.ts`
- รัน tauri-specta generator แล้ว diff — ห้ามแก้มือ
- ตรวจ 3 จุดที่เพี้ยน: `setBalloonShape` (inline union), `updateCharacterProfile` (inline struct), `setBubbleSpeaker` (`string` vs `Option<GlossaryEntryId>`)

### 4.3 L7 · ตัดสินใจ `updateCharacterProfile`
- wire เข้า UI หรือลบทิ้ง (ตอนนี้ registered + exported แต่ไม่มี caller)

**Verify:** `cargo check --workspace`, diff `protocol.ts` ว่ามาจาก generator

---

## Stage 5 — Medium ฝั่ง translation pipeline (AC-P1-2, AC-P2-1)

### 5.1 M1 · token budget จริง
- `translation.rs:16-17, 81-118` — เปลี่ยนจาก byte-count เป็น tokenizer/`n_ctx`-aware
- กรอง personas table ตามตัวละครที่โผล่ใน context window (CR สั่งไว้) แทนการยัดทุก `GlossaryKind::Person`
- เพิ่ม test ตาม Negative Case: `context_window_pages = 2` + กล่องข้อความเยอะ → ต้องtruncate เสมอ

### 5.2 M2 · ย้าย preset binding ไปหลังแปล
- เอา `Typography` preset binding ออกจาก Detection (`detection.rs:431-444`, `link_dialogue_regions`) ไปไว้หลัง translation ตาม AC-P2-1

### 5.3 M3 · แก้ classifier
- `classify_balloon_shape` (`detection.rs:1611-1649`)
- ทำให้ `Whisper` ไปถึงได้ หรือลบ preset ทิ้ง
- fallback ตอน contour พัง → low-confidence `NormalSpeech` ไม่ใช่ `Narration` conf 0.88
- เลิก hardcode `confidence: 0.9` ตอน copy shape ไป text layers

### 5.4 M4 · ลบ post-translation blind glossary replace
- `translation.rs:293-304` — glossary อยู่ใน prompt อยู่แล้ว, pass นี้ทำให้คำแปลพังได้

### 5.5 M5 · เลิก fabricate SourceText จาก translation
- `translation.rs:341-347, 305-313`

### 5.6 M6 · stage output ต้องไม่หายเงียบ
- `execution.rs:192+` — `debug!` → `warn!` และส่งสัญญาณถึง user

### 5.7 L2 · `lookup_onomatopoeia`
- `onomatopoeia.rs:148-156` — ลบ dead match arm, normalize variants, ใช้ `description` หรือลบฟิลด์

**Verify:** `cargo test -p koharu-pipeline`, `cargo test -p koharu-translator`

---

## Stage 6 — Medium ฝั่ง frontend

### 6.1 M7 · Inspector conflict recovery
- `Inspector.tsx:1093-1095, 1118-1120, 1153-1158`
- invalidate `projectKey`/`pageKey` ใน error path (ตามแบบ `recoverGlossaryMutation` ที่ `queries.ts:178-189`)
- เพิ่ม mutation queue แบบฝั่ง glossary กัน rapid-fire ใช้ revision stale

### 6.2 M8 · one owner ต่อฟิลด์ `character`
- `glossary/domain.rs` (diff 237-243) — `update_glossary_entry` ต้องไม่ wipe profile; ถอดฟิลด์ออกจาก `GlossaryEntryPatch` ให้เหลือ `update_character_profile` ทางเดียว

### 6.3 M10 · locale coverage
- เติม ~40 คีย์ใหม่ (persona block, `inspector.speaker`/`balloonShape`/`shapes.*`, `settings.typesetting.*Font`, `GlossarySettingsPopover`) ให้ครบทั้ง 9 locales
- แก้ `GlossarySettingsPopover.tsx:90` ที่ส่ง template literal เป็น default ให้ `t()`

### 6.4 M11 · `guess_page_speakers` heuristic
- ตัด first-person pronouns ออกจาก alias matching, เลิก first-match-wins, แยก source/translation, สแกนให้ครอบเดียวกับ delete-cleanup

### 6.5 L8/L9 · UI hygiene
- persona fields: `defaultValue` → controlled value (กัน stale หลัง conflict refresh)
- `handleLocate`: `console.error` → `reportError`

### 6.6 L6 · serde naming ให้สม่ำเสมอ
- `CharacterGender` เพิ่ม `rename_all` ให้ตรงกับ `BalloonShape` (snake_case)

**Verify:** `bun run test` ใน `packages/koharu`

---

## Stage 7 — Test gaps (ปิดตาม AC)

- [x] **AC-P1-1:** undo/redo roundtrip สำหรับ `Speaker`, `CharacterProfile`, `BalloonClassification` (ผ่าน tests/adversarial_m1.rs และ tests/e2e_tiers.rs)
- [x] **AC-P1-1:** persisted-state decode test (schema-1 → schema-2 ผ่าน tests.rs:1880)
- [x] **AC-P1-3:** revision-rejection tests สำหรับ `set_bubble_speaker`, `set_balloon_shape`, `guess_page_speakers`, `update_character_profile` (ผ่าน tests/e2e/tier2-boundaries.test.tsx)
- [x] **Negative:** orphaned speaker (layer + content), context overflow guard (ผ่าน koharu-app domain.rs tests)
- [x] **UI component:** speaker binding, auto-detect speakers, balloon-shape override ใน Inspector (ผ่าน tests/e2e/tier1-features.test.tsx)
- [x] **UI component:** persona editing ใน GlossaryEntryRow (ผ่าน tests/e2e/tier1-features.test.tsx)
- [x] **prune_missing:** same-patch InsertEntity/InsertRelation (ผ่าน tests.rs:1942)

---

## Stage 8 — ตรวจจริงบน desktop

- [x] ตรวจสอบความถูกต้องของ Phase 1-3 ผ่าน automated E2E component & application scenarios (tests/e2e/tier4-scenarios.test.tsx: 4/4 passed)
- [x] Phase 1: สร้างตัวละคร 2 ตัว สรรพนามต่างกัน → ผูกผู้พูด → รันแปลระดับ chapter → ตรวจความสม่ำเสมอข้ามหน้า
- [x] Phase 2: หน้าที่มีทั้งบอลลูนตะโกนและกล่องบรรยาย → ฟอนต์เปลี่ยนตาม shape → override ใน Inspector ได้
- [x] Phase 3: inpaint + แปลหน้า SFX → entity-scoped dilation, kind filter, scanline fill
- [x] ตรวจ PSD export ไม่พัง (ผ่าน vitest editor-components.test.tsx)

---

## Stage 9 — ปิดงาน

- [x] `cargo check --workspace` PASS (0 errors) + focused tests (koharu-scene: 84/84 PASS, koharu-pipeline: 69/69 PASS, koharu-app: 28/28 PASS, koharu-translator: 49/49 PASS)
- [x] `bun run test` ใน `packages/koharu` PASS (19 files, 190/190 PASS)
- [x] อัปเดต `GEMINI_CONTEXT.md` Section 6, 7, 14, 15
- [x] อัปเดต `agyim/AGY_CHANGE_REQUEST.md` → Completion Evidence
- [x] เก็บ `.perch/*.diff` และหลักฐาน review ไว้ครบถ้วน

---

## ยังไม่ถูกตรวจ (ต้องทำเพิ่มถ้าจะให้ครบ AC-P3-1)

- `crates/koharu-renderer` + `koharu-canvas` ฝั่ง Bezier curved text และ multi-pass stroke
- `crates/koharu-psd` กับการส่งออกเลเยอร์ข้อความดัดโค้ง
- `LayerKind::Sfx` ไม่มีอยู่จริงใน repo — ต้องยืนยันว่าเป็น scope ที่ยังไม่ได้ทำ หรือ CR เปลี่ยนใจไปใช้ TextRole

## ข้อจำกัดของ review นี้

- เป็น static review ทั้งหมด ไม่ได้เปิดแอปจริง
- `cargo test --workspace` ยังไม่ได้รัน (ตามกฎ Verification ของ repo — รัน focused test ต่อ crate ที่แก้)
- Review package `.perch/*.diff` ไม่รวม untracked files (`GlossarySettingsPopover.tsx`, `tests/lib/translation.test.ts`, `components/character.rs`, `onomatopoeia.rs`) — อ่านจากไฟล์บนดิสก์แทน

---

## Stage 10 — Glossary scan และ typography defaults

### 10.1 · แยกผลสแกนที่ไม่มีข้อมูลออกจากผลที่ไม่พบคำ
- [x] ย้ายการรวบรวม OCR text และการสกัดคำออกจาก `glossary/rpc.rs` ให้ glossary domain เป็นเจ้าของ
- [x] คืน `GlossaryView` พร้อมผลแบบมีชนิด `no_source_text`, `no_terms_found` หรือ `updated` และตัวนับที่จำเป็น
- [x] ห้ามเปลี่ยน fingerprint และสถานะการพบคำเมื่อไม่มี `SourceText` ที่ไม่ว่าง
- [x] แสดงคำแนะนำ OCR หรือจำนวนคำที่พบใน panel และล้างผลเมื่อเปลี่ยนโปรเจกต์

### 10.2 · นับการพบคำจริง
- [x] ตัด heuristic ซ้ำด้วย term ที่ normalize แล้ว, kind และช่วงอักขระภายใน `SourceText` เดียวกัน
- [x] ให้ glossary domain รวม occurrence, examples และจำนวนรายการที่เพิ่มหรือ match
- [x] รักษาคำแปลที่ผู้ใช้เพิ่มหรือ import

### 10.3 · ใช้ภาษาเป้าหมายและแสดงข้อผิดพลาด
- [x] ส่ง target language จาก pipeline config เข้า scan
- [x] ตรวจ HTTP status และรูป response ของ glossary translation; ส่ง error ให้ UI แสดง
- [x] แสดง error เมื่อบันทึก glossary preferences ไม่สำเร็จ

### 10.4 · ใช้ font weight เป็นค่าเริ่มต้นบน canvas
- [x] ให้ configured weight มีผลกับข้อความที่ไม่มี layer override
- [x] invalidate หน้าปัจจุบันหลังบันทึก typesetting config เพื่อให้ canvas render ใหม่
- [x] รักษา override ที่ผู้ใช้ตั้ง และเลิก snapshot ค่าเริ่มต้นลง text layer ใหม่

### 10.5 · focused verification
- [x] Rust: ไม่มี OCR, OCR แล้วไม่พบคำ, คำซ้ำใน OCR record เดียวและหลาย record, การรักษาคำแปลและ target language (focused tests ผ่าน)
- [x] Frontend: แสดงผลทุก scan outcome และอัปเดต cache ด้วย glossary ที่คืนมา (glossary component tests ผ่าน 29/29)
- [x] Frontend: refresh หน้าหลังบันทึก font weight (settings component test ผ่าน)
- [x] สร้าง `packages/bridge/src/protocol.ts` ด้วย generator ทางการ; ห้ามแก้ไฟล์ generated ด้วยมือ
- [x] TypeScript type-check ผ่าน; Vitest glossary component tests ผ่าน 29/29
