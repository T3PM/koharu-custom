# AGY_CHANGE_REQUEST.md

## 1. Request Metadata
- **request_id:** `AGY-CR-20261026-MANGA-SUITE-001`
- **status:** `IMPLEMENTED`
- **based_on_snapshot_id:** `koharu-61b5421-clean-20261026`
- **base_commit:** `61b5421d9959ec2096d345cc406fe90f3aecbbd7` (Branch: `feature/chapter-workflow-and-detection`, Status: `CLEAN`)
- **relevant_source_hashes (SHA-256):**
  - `Cargo.toml`: `B929C7EA2B77CAA78C7BE703FE57E7CC8FBC7980E72C69C6CB4320442C4DD7BB`
  - `crates/koharu/src/main.rs`: `E357186332E8EFAB48CC107D715BF2AF745BD7C9B45DF178601431204F45D29E`
  - `crates/koharu-app/src/lib.rs`: `5AE801958C706511C54ACE74103621F3EB247216E61D861919EBB2A2CA1E209A`
  - `crates/koharu-pipeline/src/pipeline.rs`: `6A61A2952122EB43BF67CBB67B85F857536B13305096988A4BE75C37F11C049E`
  - `packages/bridge/src/protocol.ts`: `D59117AED43DDEC5FC31F8B12120199FA5B10D3315DD768D1800623D07E369B7`
- **implementation_approved:** `true`

---

## 2. Goal and Success Criteria

### User-Visible Outcome
1. **Phase 1 (Character & Persona Memory):** นักแปลสามารถกำหนดข้อมูลบุคลิกตัวละคร (เพศ สรรพนามบุรุษที่ 1/2 โทนการพูด และคำสั่งเฉพาะ) ใน `GlossaryPanel.tsx` ผูกผู้พูด (`Speaker`) เข้ากับแต่ละกล่องข้อความใน `Inspector.tsx` (ทั้งแบบเลือกเองและสั่งเดาอัตโนมัติ) และสั่งแปลแบบต่อเนื่องหลายหน้าโดยคงสำเนียงและสรรพนามของตัวละครเดิมได้อย่างแม่นยำ
2. **Phase 2 (Smart Typesetting & Bubble-Shape Font Pairing):** ระบบจำแนกรูปทรงของบอลลูนคำพูด (`NormalSpeech`, `Shout`, `Thought`, `Whisper`, `Narration`) จากเส้นขอบ Mask อัตโนมัติและจับคู่ฟอนต์ ขนาด ระยะห่างบรรทัด และความหนาขอบตาม Preset ที่ตั้งค่าไว้ พร้อมให้ผู้ใช้แก้ไขทับ (Override) ได้ใน `Inspector.tsx`
3. **Phase 3 (SFX Pipeline & Stylized Calligraphy):** ระบบแยกแยะเลเยอร์เสียงประกอบ (`LayerKind::Sfx`) ลบภาพพื้นหลังโดยรักษาสกรีนโทน แปลคำเลียนเสียงธรรมชาติด้วยตารางเทียบคำ และจัดวางตัวอักษรตามเส้นโค้ง Bezier พร้อมขอบตัวอักษรหลายชั้นบน WebGPU Canvas

### Business Outcome
- ยกระดับ Koharu จากเครื่องมือแปลรายหน้า (Page-by-Page Tool) ให้เป็นชุดเครื่องมือแปลและจัดหน้ามังงะระดับมืออาชีพแบบครบวงจรในระดับตอน (Chapter-level Production Suite)
- ลดเวลาการตรวจแก้คำแปลที่สรรพนามเพี้ยนข้ามหน้า ลดภาระการเลือกฟอนต์ซ้ำซากสำหรับบอลลูนมาตรฐาน และลดการพึ่งพาโปรแกรมภายนอกในการทำ Typesetting เสียงประกอบ (SFX)

### Concrete Acceptance Criteria
- **AC-P1-1:** โครงสร้าง `CharacterProfile` และ `Speaker` ถูกนิยามแบบ Strongly Typed ใน `crates/koharu-scene` และถูกบันทึก/เรียกคืนผ่านระบบ `Snapshot` และ `Patch` ได้โดยไม่สูญเสียข้อมูลเมื่อทำ Undo/Redo
- **AC-P1-2:** สเตจการแปลใน `crates/koharu-pipeline/src/stages/translation.rs` แนบตาราง Persona แท็ก `[Speaker: <Name>]` และบริบทคำแปลจาก `context_window_pages` หน้าก่อนหน้าเข้าสู่ Prompt โดยไม่เกิด Context Overflow บน Local GGUF LLM
- **AC-P1-3:** คำสั่ง IPC ใหม่สร้าง TypeScript Bindings ลง `packages/bridge/src/protocol.ts` ผ่าน `tauri-specta` โดยตรง และ UI ใน `GlossaryPanel.tsx` กับ `Inspector.tsx` ทำงานร่วมกับ `expectedRevision` ได้อย่างถูกต้อง
- **AC-P2-1:** สเตจ Detection คำนวณ `BalloonClassification` (`BalloonShape` และค่าความเชื่อมั่น) จาก Contour ของ RF-DETR Mask และผูก Preset ฟอนต์ลง `TextLayer` อัตโนมัติหลังแปลเสร็จ
- **AC-P3-1:** เลเยอร์ `LayerKind::Sfx` ได้รับการขยาย Mask แบบ Adaptive Morphological Dilation ก่อนส่งเข้า `inpainting.rs` และเรนเดอร์ข้อความตามเส้นโค้ง Bezier พร้อมเส้นขอบ (Multi-pass Stroke) บน `koharu-canvas` ได้

---

## 3. Scope and Non-goals

### Included Behavior
- การขยายโมเดลโดเมนใน `crates/koharu-scene` สำหรับ `CharacterProfile`, `Speaker`, `BalloonShape`, `BalloonClassification`, และ `LayerKind::Sfx`
- การปรับปรุง `crates/koharu-pipeline` ในสเตจ `detection.rs`, `translation.rs`, และ `inpainting.rs`
- การเพิ่มคำสั่ง IPC ใน `crates/koharu-app` และการสร้าง TypeScript Bindings อัตโนมัติใน `packages/bridge`
- การขยายขีดความสามารถในการจัดวางฟอนต์และวาดข้อความตามเส้นโค้งใน `crates/koharu-renderer` และ `crates/koharu-canvas`
- การอัปเดตคอมโพเนนต์ UI ใน `packages/koharu` (`GlossaryPanel.tsx`, `Inspector.tsx`, `TypesettingPreferences.tsx`)

### Explicitly Excluded Behavior (Non-goals)
- **ห้าม**ฝึกสอน (Train/Fine-tune) โมเดล Machine Learning ใหม่ หรือเพิ่มไฟล์ Model Weights ลงใน Git Repository
- **ห้าม**สร้างชั้น Backward Compatibility Parser, Schema Alias หรือ Forwarding Layer สำหรับอ่านไฟล์สถานะรูปแบบเก่า (ตามกฎเหล็กใน Section 12)
- **ห้าม**แก้ไขโค้ดใน `crates/koharu-llama-sys`, `crates/koharu-diffusion-sys`, หรือ `crates/koharu-torch-sys` (C/C++ FFI Bindings) เว้นแต่จะตรวจพบข้อจำกัดที่ได้รับการอนุมัติแยกต่างหาก
- **ห้าม**แก้ไขไฟล์ `packages/bridge/src/protocol.ts` ด้วยมือโดยไม่ผ่านกลไก Generator ของ `tauri-specta`

### Compatibility and Architectural Constraints
- **Rust Edition & Toolchain:** ต้องคอมไพล์ผ่านบน Rust Edition 2024 (Rust 1.97.1+) และ Bun 1.3.14+
- **Immutability & Revision Discipline:** การเปลี่ยนแปลงข้อมูลใน `koharu-scene` ทุกจุดต้องทำผ่านระบบ `Patch` และตรวจสอบ `expectedRevision` เพื่อรักษา Optimistic Concurrency Control
- **VRAM Eviction Discipline:** การเพิ่มขั้นตอนใน `koharu-pipeline` ต้องเคารพกลไก Stage VRAM Eviction เดิม และปิด Gradient Tracking เสมอระหว่างรัน Inference
- **Lean RPC Handlers:** ลอจิกการประมวลผลโดเมนต้องอยู่ใน `crates/koharu-scene` หรือ `crates/koharu-pipeline` ไม่เขียนอัดไว้ใน `crates/koharu-app/src/commands/` เพื่อป้องกันปัญหา IPC RPC Bloat

---

## 4. Current-State Evidence

### Relevant Existing Modules, Interfaces, and Flows
- **Scene & Glossary Domain (`[FACT]`):** `crates/koharu-scene` จัดการ `Snapshot`, `EntityId`, `Revision`, `Page`, `Layer`, `Glossary`, และ `GlossaryEntry` (`source`, `translation`, `kind: GlossaryKind`, `origin`, `occurrence_count`, `examples`) โดยรองรับ `GlossaryKind::Person` อยู่แล้ว (อ้างอิง `GEMINI_CONTEXT.md` Section 3, 6, 15)
- **ML Pipeline (`[FACT]`):** `crates/koharu-pipeline` จัดการลำดับงานผ่าน `crates/koharu-pipeline/src/pipeline.rs` (SHA-256: `6A61A295...`) โดยมีสเตจย่อยที่ระบุใน Section 15 ได้แก่ `crates/koharu-pipeline/src/stages/detection.rs`, `crates/koharu-pipeline/src/stages/translation.rs`, และ `crates/koharu-pipeline/src/stages/inpainting.rs`
- **IPC Commands & Bridge (`[FACT]`):** `crates/koharu-app/src/lib.rs` (SHA-256: `5AE80195...`) และ `crates/koharu-app/src/commands/glossary/` นิยามคำสั่ง RPC เช่น `select_page`, `process`, `commit_inpaint`, `add_glossary_entry` และส่งออก Type ไปยัง `packages/bridge/src/protocol.ts` (SHA-256: `D59117AE...`) (อ้างอิง `GEMINI_CONTEXT.md` Section 1, 3, 7)
- **Frontend UI (`[FACT]`):** `packages/koharu/components/` มีคอมโพเนนต์ `PageRail.tsx`, `CanvasCommandBar.tsx`, `Inspector.tsx`, `GlossaryPanel.tsx`, และ `InferenceControl.tsx` (อ้างอิง `GEMINI_CONTEXT.md` Section 8, 15)

### Evidence References
- `GEMINI_CONTEXT.md` Section 1: ข้อมูล Snapshot (`koharu-61b5421-clean-20261026`), Commit (`61b5421d9959ec2096d345cc406fe90f3aecbbd7`), และ SHA-256 Checksums
- `GEMINI_CONTEXT.md` Section 2 & 3: รายชื่อ 24 Rust Crates, 4 Frontend Packages, และตาราง ML Models (`koharu-layout-rfdetr-seg-2xl-1152`, Local GGUF, Cloud APIs)
- `GEMINI_CONTEXT.md` Section 4, 5, 6, 7: แผนผังสถาปัตยกรรม 5 ชั้น, Sequence Diagram ของการประมวลผล 4 สเตจ, กฎ Business Invariants 3 ข้อ, และรายการ IPC Contracts
- `GEMINI_CONTEXT.md` Section 9: ซอร์สโค้ด Entrypoint ของ `crates/koharu/src/main.rs` และ `crates/koharu-app/src/app.rs` (ตั้งค่า CEF Remote Debugging Port ที่ `127.0.0.1:4000`)
- `GEMINI_CONTEXT.md` Section 12 & 13: กฎเหล็กของโปรเจกต์ 5 ข้อ และคำสั่งตรวจสอบระบบ (`cargo check`, `cargo test --workspace`, `bun test`)
- `GEMINI_CONTEXT.md` Section 14 & 15: ความเสี่ยงด้าน Context Token Limits, IPC RPC Bloat, Contour Precision และตาราง Change-Impact Map ของทั้ง 3 เฟส

### Assumptions and Unresolved Questions
- **`[ASSUMPTION-1]`:** สมมติว่า `GlossaryEntry` ใน `crates/koharu-scene/src/components/glossary.rs` มีตัวระบุ `GlossaryEntryId` ที่สามารถนำไปอ้างอิงข้ามคอมโพเนนต์จาก Text Layer ได้โดยไม่เกิด Circular Dependency
- **`[ASSUMPTION-2]`:** สมมติว่าโมเดล `koharu-layout-rfdetr-seg-2xl-1152` ใน `crates/koharu-pipeline/src/stages/detection.rs` ส่งคืนข้อมูล Segmentation Mask หรือ Polygon ของแต่ละภูมิภาคที่เพียงพอต่อการนำมาคำนวณ Perimeter, Area และ Convexity Defects
- **`[UNRESOLVED-1 (BLOCKING)]`:** ยังไม่มีเนื้อหาไฟล์ต้นฉบับของ `crates/koharu-scene/src/components/glossary.rs`, `crates/koharu-pipeline/src/stages/translation.rs`, `crates/koharu-pipeline/src/stages/detection.rs`, `crates/koharu-renderer`, และ `packages/koharu/components/editor/Inspector.tsx` ในบริบท จึงต้องให้ Antigravity อ่านไฟล์เหล่านี้เพื่อยืนยันชื่อ Struct/Trait และหมายเลขบรรทัดจริงก่อนเริ่มแก้ไข
- **`[UNRESOLVED-2 (BLOCKING)]`:** ต้องได้รับการยืนยันจากผู้ใช้เรื่องการเพิ่มพารามิเตอร์ `expected_revision: Revision` ในคำสั่ง IPC ใหม่ทุกคำสั่งเพื่อให้เป็นไปตามกฎ Optimistic Locking

---

## 5. Proposed Design

### 5.1 Proposed Changes and Rationale
1. **Domain-Driven Scene Extensions (`crates/koharu-scene`):**
   - ขยายโมเดลใน `crates/koharu-scene/src/components/glossary.rs` (หรือแยกซับโมดูล `crates/koharu-scene/src/components/character.rs` **[NEW]**) เพื่อเก็บ `CharacterProfile` (`gender: CharacterGender`, `first_person`, `second_person`, `tone`, `custom_instructions`)
   - เพิ่มคอมโพเนนต์ `Speaker(pub Option<GlossaryEntryId>)`, `BalloonShape`, `BalloonClassification`, และ `LayerKind::Sfx` ใน `crates/koharu-scene` เพื่อให้สถานะทั้งหมดถูกจัดเก็บอย่างมี Type Safety ใน `Snapshot`
2. **Context-Aware & Geometry-Aware Pipeline (`crates/koharu-pipeline`):**
   - **Stage 1 (`detection.rs`):** เพิ่มตัววิเคราะห์รูปทรงเรขาคณิตของบอลลูน (Contour Classifier) จากผลลัพธ์ของ RF-DETR เพื่อระบุ `BalloonShape` และแยกเลเยอร์ข้อความที่อยู่นอกบอลลูนเป็น `LayerKind::Sfx`
   - **Stage 3 (`translation.rs`):** เพิ่มการดึงบริบท `context_window_pages` หน้าก่อนหน้าใน Chapter เดียวกัน ผสานกับตาราง `CharacterProfile` เฉพาะตัวละครที่เกี่ยวข้อง และแนบแท็ก `[Speaker: <Name>]` หรือสั่งให้ LLM คาดเดาผู้พูดพร้อมคำแปลเมื่อเปิด `auto_guess_speakers` สำหรับเลเยอร์ `LayerKind::Sfx` ให้แนบตารางเทียบคำเลียนเสียงธรรมชาติ (Onomatopoeia Lookup Table)
   - **Stage 4 (`inpainting.rs`):** ปรับการขยาย Mask (Adaptive Morphological Dilation) สำหรับเลเยอร์ `LayerKind::Sfx` เพื่อครอบคลุมขอบเส้นหนาและเงาตกกระทบ พร้อมปรับพารามิเตอร์รักษาลายสกรีนโทน
3. **Typography Rules & Stylized Canvas Rendering (`crates/koharu-renderer`, `crates/koharu-canvas`):**
   - นิยาม `TypesettingPreset` ที่แมป `BalloonShape` เข้ากับฟอนต์ ขนาดสัมพัทธ์ ระยะห่างบรรทัด และความหนาขอบ
   - เพิ่มการคำนวณตำแหน่งตัวอักษรตามเส้นโค้ง Bezier (Text-on-path) และการวาดเส้นขอบหลายชั้น (Multi-pass Stroke) สำหรับเลเยอร์ SFX

### 5.2 Data, API, UI, Authorization, and Operational Impacts
- **Data Impact:** เปลี่ยนแปลงโครงสร้างข้อมูลใน `Snapshot` และ `Patch` ของ `koharu-scene` โดยตรง (ไม่มี Backward Compatibility Layer ตามกฎของโปรเจกต์)
- **API / IPC Impact:** เพิ่มคำสั่ง RPC ใน `crates/koharu-app/src/commands/` ได้แก่ `set_bubble_speaker`, `guess_page_speakers`, `update_character_profile` และคำสั่งตั้งค่า `TypesettingPreset` พร้อมอัปเดต `packages/bridge/src/protocol.ts` ผ่าน `tauri-specta`
- **UI Impact:** อัปเดต `GlossaryPanel.tsx` (ฟอร์ม Persona), `Inspector.tsx` (Dropdown เลือกผู้พูด, ปุ่ม Auto-detect Speakers, ป้ายกำกับ/ตัวเลือก `BalloonShape`, และตัวควบคุมเส้นโค้ง Bezier สำหรับ SFX), และ `TypesettingPreferences.tsx`
- **Authorization & Security Impact:** การเรียกใช้ Cloud LLM ใน `guess_page_speakers` หรือการแปลแบบหลายหน้ายังคงดึง API Key จาก `koharu-secrets` (OS Keyring) เท่านั้น ไม่มีการบันทึกข้อมูลลับลงไฟล์โปรเจกต์
- **Operational Impact:** ควบคุมขนาด Prompt ของ Sliding Window ไม่ให้เกินขีดจำกัด Context ของโมเดลท้องถิ่น (`koharu-llama`) เพื่อป้องกันอาการ Out-of-Memory (OOM)

### 5.3 Alternatives Considered and Trade-offs
- **การเก็บข้อมูลตัวละครแยกจาก Glossary vs ขยาย `GlossaryEntry`:** เลือกขยาย `GlossaryEntry` / ใช้ `GlossaryEntryId` ร่วมกัน เพื่อไม่ต้องซิงค์รายชื่อตัวละครสองระบบ
- **การจำแนกทรงบอลลูนด้วยโมเดล ML ตัวใหม่ vs วิเคราะห์เรขาคณิตจาก Contour ของ RF-DETR:** เลือกการวิเคราะห์เรขาคณิตจาก Contour ของ RF-DETR (`perimeter^2 / area`, convexity defects) เพื่อไม่เพิ่มภาระ VRAM และไม่ต้องโหลดโมเดล Vision ตัวที่สองเข้าสู่ไปป์ไลน์

### 5.4 Current Components vs. Proposed Components
| ส่วนประกอบ (Component / File) | สถานะปัจจุบัน (Current) | สถานะที่เสนอ (Proposed) |
| :--- | :--- | :--- |
| `crates/koharu-scene/src/components/glossary.rs` | เก็บ `Glossary` และ `GlossaryEntry` พื้นฐาน (`[FACT]`) | **[MODIFY]** เชื่อมโยง `CharacterProfile` เข้ากับ `GlossaryEntry` |
| `crates/koharu-scene/src/components/character.rs` | ไม่มีในคลังโค้ด | **[NEW]** นิยาม `CharacterProfile`, `CharacterGender`, และ `Speaker` |
| `crates/koharu-pipeline/src/stages/translation.rs` | แปลข้อความพร้อมแนบคำศัพท์ Glossary (`[FACT]`) | **[MODIFY]** เพิ่ม Persona Prompt, Speaker Tagging, และ Multi-Page Sliding Window |
| `crates/koharu-pipeline/src/stages/detection.rs` | รัน RF-DETR ตรวจจับกล่องข้อความและบอลลูน (`[FACT]`) | **[MODIFY]** เพิ่มการคำนวณ `BalloonClassification` และการแยก `LayerKind::Sfx` |
| `crates/koharu-pipeline/src/stages/inpainting.rs` | รันโมเดล Inpainting ลบข้อความ (`[FACT]`) | **[MODIFY]** เพิ่ม Adaptive Morphological Dilation สำหรับ Mask ของ `LayerKind::Sfx` |
| `packages/koharu/components/editor/GlossaryPanel.tsx` | จัดการคำศัพท์ ค้นหา และนำเข้า/ส่งออก (`[FACT]`) | **[MODIFY]** เพิ่มส่วนแก้ไขข้อมูล Persona สำหรับรายการประเภท `Person` |
| `packages/koharu/components/editor/Inspector.tsx` | แก้ไขข้อความ ฟอนต์ สี และทิศทางตัวอักษร (`[FACT]`) | **[MODIFY]** เพิ่ม Speaker Selector, Auto-detect Speakers, Balloon Shape Override, และ SFX Controls |

---

## 6. Dependency-Ordered Tasks

### Task 1: `TASK-P1-SCENE-PERSONA` (Phase 1 - Domain Model)
- **task_id:** `TASK-P1-SCENE-PERSONA`
- **objective:** เพิ่มโครงสร้างข้อมูล `CharacterProfile`, `CharacterGender`, และคอมโพเนนต์ `Speaker` ลงใน `crates/koharu-scene` พร้อมรองรับการสร้าง `Patch`
- **depends_on:** `None`
- **existing files to inspect:**
  - `crates/koharu-scene/Cargo.toml`
  - `crates/koharu-scene/src/lib.rs`
  - `crates/koharu-scene/src/components/glossary.rs`
- **proposed files to create or modify:**
  - **[NEW]** `crates/koharu-scene/src/components/character.rs`
  - **[MODIFY]** `crates/koharu-scene/src/components/glossary.rs`
  - **[MODIFY]** `crates/koharu-scene/src/lib.rs` (หรือไฟล์รวม Components ของ Scene)
- **affected symbols or contracts:** `GlossaryEntry`, `GlossaryEntryId`, `CharacterProfile`, `CharacterGender`, `Speaker`, `Snapshot`, `Patch`
- **implementation outline:**
  1. สร้าง `CharacterGender` enum (`Male`, `Female`, `NonBinary`, `Unspecified`) และ `CharacterProfile` struct (`id: GlossaryEntryId`, `gender`, `first_person`, `second_person`, `tone`, `custom_instructions`) พร้อม Derive `Serialize`, `Deserialize`, `specta::Type`, `Clone`, `Debug`, `PartialEq`
  2. สร้างคอมโพเนนต์ `pub struct Speaker(pub Option<GlossaryEntryId>);` สำหรับผูกกับ Text Layer `EntityId`
  3. เพิ่มเมธอดสำหรับอัปเดต `CharacterProfile` และ `Speaker` ผ่านกลไก `Patch` ของ `koharu-scene`
- **acceptance criteria:**
  - สามารถสร้าง แก้ไข และทำ Serialize/Deserialize ข้อมูล `CharacterProfile` และ `Speaker` ได้ครบถ้วน
  - การลบ `GlossaryEntry` ต้องล้างค่า `Speaker` ที่อ้างอิงถึง `GlossaryEntryId` นั้นอย่างปลอดภัย (ไม่มี Dangling Reference)
- **tests to add or update:**
  - เพิ่ม Unit Tests ใน `crates/koharu-scene` ทดสอบการบันทึก อัปเดต ลบ และ Undo/Redo ของ `CharacterProfile` และ `Speaker`
- **verification commands supported by context:**
  - `cargo check` (ใน `c:\Users\T3R0\koharu`)
  - `cargo test --workspace` (ใน `c:\Users\T3R0\koharu`)
  - *คำสั่งเฉพาะเจาะจงที่เสนอเพิ่มเติม (Proposed/Unconfirmed):* `cargo test -p koharu-scene`
- **risks and conditions that should stop implementation:**
  - หยุดทำงานทันทีหากพบว่า `GlossaryEntryId` ไม่ได้ถูกส่งออกในระดับที่คอมโพเนนต์ของ Layer เข้าถึงได้ หรือหากโครงสร้าง `Patch` ใน `koharu-storage` มีการผูก Schema แบบ Hardcoded ที่ต้องแก้ไขข้าม Crate

### Task 2: `TASK-P1-PIPELINE-TRANSLATION` (Phase 1 - Multi-Page & Persona Translation)
- **task_id:** `TASK-P1-PIPELINE-TRANSLATION`
- **objective:** อัปเดตสเตจการแปลใน `crates/koharu-pipeline` ให้รองรับตาราง Persona, แท็ก `[Speaker: <Name>]`, Sliding Window บริบท `N` หน้าก่อนหน้า และการคาดเดาผู้พูดอัตโนมัติ
- **depends_on:** `TASK-P1-SCENE-PERSONA`
- **existing files to inspect:**
  - `crates/koharu-pipeline/src/pipeline.rs`
  - `crates/koharu-pipeline/src/stages/translation.rs`
  - `crates/koharu-translator/src/lib.rs` (และไฟล์ภายใน `crates/koharu-translator` กับ `crates/koharu-llama`)
- **proposed files to create or modify:**
  - **[MODIFY]** `crates/koharu-pipeline/src/stages/translation.rs`
  - **[MODIFY]** `crates/koharu-pipeline/src/pipeline.rs` (และไฟล์ตั้งค่าของ `koharu-pipeline`)
- **affected symbols or contracts:** Pipeline Configuration (`enable_personas`, `enable_speaker_tagging`, `context_window_pages`, `auto_guess_speakers`), Translation Stage Runner
- **implementation outline:**
  1. เพิ่มฟิลด์ตั้งค่า `enable_personas: bool` (default: `true`), `enable_speaker_tagging: bool` (default: `true`), `context_window_pages: usize` (default: `2`), และ `auto_guess_speakers: bool` (default: `false`)
  2. ใน `translation.rs` สร้างฟังก์ชันคัดกรองเฉพาะตัวละครที่มีส่วนเกี่ยวข้องกับหน้าปัจจุบันหรือหน้าในระยะ `context_window_pages` เพื่อสร้างตาราง Persona ใน System Prompt
  3. ดึงข้อความต้นฉบับและคำแปลจาก `context_window_pages` หน้าก่อนหน้าใน Chapter เดียวกันมาประกอบเป็น Narrative Context Block พร้อมจำกัดความยาวไม่ให้เกิน Token Budget ของ Local GGUF
  4. แนบ `[Speaker: <Name>]` หน้าบรรทัดที่มีการกำหนด `Speaker` แล้ว และรองรับการสั่งให้ LLM คืนค่าคาดเดาผู้พูดเมื่อ `auto_guess_speakers == true`
- **acceptance criteria:**
  - Prompt ที่สร้างมีตาราง Persona และบริบทหน้าก่อนหน้าตามจำนวน `context_window_pages` ที่ตั้งไว้
  - เมื่อปิดแฟล็ก `enable_personas` หรือไม่มีข้อมูลผู้พูด ระบบยังแปลข้อความได้ตามปกติ
- **tests to add or update:**
  - เพิ่ม Unit Tests ใน `crates/koharu-pipeline` เพื่อตรวจสอบการจัดรูปแบบ System Prompt, การทำงานของ Sliding Window เมื่ออยู่หน้าแรกของตอน (Page 0) เทียบกับหน้าถัดไป และการตัดทอนเมื่อข้อความยาวเกินกำหนด
- **verification commands supported by context:**
  - `cargo check` (ใน `c:\Users\T3R0\koharu`)
  - `cargo test --workspace` (ใน `c:\Users\T3R0\koharu`)
  - *คำสั่งเฉพาะเจาะจงที่เสนอเพิ่มเติม (Proposed/Unconfirmed):* `cargo test -p koharu-pipeline`
- **risks and conditions that should stop implementation:**
  - หยุดทำงานหากพบว่าเมื่อรันในโหมด `Scope::Page` ไปป์ไลน์ไม่สามารถเข้าถึงสถานะของหน้าก่อนหน้าใน `Snapshot` เดียวกันได้

### Task 3: `TASK-P1-IPC-AND-UI` (Phase 1 - IPC Bridge & Editor UI)
- **task_id:** `TASK-P1-IPC-AND-UI`
- **objective:** เปิดคำสั่ง IPC สำหรับจัดการ `CharacterProfile` และ `Speaker` สร้าง TypeScript Bindings ใหม่ และอัปเดต `GlossaryPanel.tsx` กับ `Inspector.tsx`
- **depends_on:** `TASK-P1-SCENE-PERSONA`, `TASK-P1-PIPELINE-TRANSLATION`
- **existing files to inspect:**
  - `crates/koharu-app/src/lib.rs`
  - `crates/koharu-app/src/commands/glossary/`
  - `packages/bridge/src/protocol.ts`
  - `packages/koharu/components/editor/GlossaryPanel.tsx`
  - `packages/koharu/components/editor/Inspector.tsx`
- **proposed files to create or modify:**
  - **[MODIFY]** `crates/koharu-app/src/commands/glossary/` (ไฟล์คำสั่ง Glossary และ Scene Mutation)
  - **[MODIFY]** `crates/koharu-app/src/lib.rs`
  - **[MODIFY]** `packages/bridge/src/protocol.ts` (ผ่าน Generator ของ `tauri-specta` เท่านั้น)
  - **[MODIFY]** `packages/koharu/components/editor/GlossaryPanel.tsx`
  - **[MODIFY]** `packages/koharu/components/editor/Inspector.tsx`
- **affected symbols or contracts:** `set_bubble_speaker`, `guess_page_speakers`, `update_character_profile`, `GlossaryView`
- **implementation outline:**
  1. เพิ่มคำสั่ง IPC `set_bubble_speaker`, `guess_page_speakers`, และ `update_character_profile` ใน `crates/koharu-app/src/commands/` โดยรับ `expected_revision: Revision` ตามมาตรฐาน Optimistic Locking และเรียกใช้ Domain Logic ใน `koharu-scene` / `koharu-pipeline`
  2. อัปเดต `packages/bridge/src/protocol.ts` ผ่านกลไกของ `tauri-specta`
  3. อัปเดต `GlossaryPanel.tsx` ให้แสดงฟิลด์เพศ สรรพนามบุรุษที่ 1/2 และโทนการพูดเมื่อเลือกคำศัพท์ประเภท `GlossaryKind::Person`
  4. อัปเดต `Inspector.tsx` ให้มี Dropdown เลือกผู้พูดสำหรับเลเยอร์ข้อความที่เลือก และปุ่ม "Auto-detect Speakers"
- **acceptance criteria:**
  - `packages/bridge/src/protocol.ts` ซิงค์ตรงกับ Rust Commands โดยไม่ต้องแก้ไขไฟล์ด้วยมือ
  - การแก้ไข Persona ใน `GlossaryPanel.tsx` และการเลือกผู้พูดใน `Inspector.tsx` อัปเดตสถานะและ `Revision` อย่างถูกต้อง
- **tests to add or update:**
  - เพิ่มการทดสอบคำสั่ง IPC ใน `crates/koharu-app` (รวมถึงกรณีส่ง `expectedRevision` ไม่ตรงซึ่งต้องถูกปฏิเสธ)
  - เพิ่มการทดสอบ React Component ใน `packages/koharu` สำหรับ `GlossaryPanel.tsx` และ `Inspector.tsx`
- **verification commands supported by context:**
  - `cargo test --workspace` (ใน `c:\Users\T3R0\koharu`)
  - `bun test` (ใน `packages/koharu`)
- **risks and conditions that should stop implementation:**
  - หยุดทำงานหากการสร้าง TypeScript Bindings ของ `tauri-specta` ล้มเหลวหรือกระทบสัญญาของคำสั่งเดิมใน `packages/bridge/src/protocol.ts`

### Task 4: `TASK-P2-BALLOON-TYPESETTING` (Phase 2 - Contour Classification & Smart Typesetting)
- **task_id:** `TASK-P2-BALLOON-TYPESETTING`
- **objective:** จำแนกรูปทรงบอลลูนคำพูดจาก Contour ของ RF-DETR ในสเตจ Detection และผูกฟอนต์พร้อมสไตล์ลงบน `TextLayer` ตาม Preset อัตโนมัติ
- **depends_on:** `TASK-P1-IPC-AND-UI`
- **existing files to inspect:**
  - `crates/koharu-scene/src/lib.rs`
  - `crates/koharu-pipeline/src/stages/detection.rs`
  - `crates/koharu-renderer/src/lib.rs`
  - `packages/koharu/components/editor/Inspector.tsx`
  - `packages/koharu/components/preferences/`
- **proposed files to create or modify:**
  - **[MODIFY]** `crates/koharu-scene` (เพิ่ม `BalloonShape` และ `BalloonClassification`)
  - **[MODIFY]** `crates/koharu-pipeline/src/stages/detection.rs`
  - **[MODIFY]** `crates/koharu-renderer` (เพิ่ม `TypesettingPreset` และการแมปกฎฟอนต์)
  - **[MODIFY]** `packages/koharu/components/preferences/` (`TypesettingPreferences.tsx`)
  - **[MODIFY]** `packages/koharu/components/editor/Inspector.tsx`
- **affected symbols or contracts:** `BalloonShape` (`NormalSpeech`, `Shout`, `Thought`, `Whisper`, `Narration`), `BalloonClassification`, `TypesettingPreset`
- **implementation outline:**
  1. นิยาม `BalloonShape` และ `BalloonClassification` ใน `crates/koharu-scene`
  2. ใน `crates/koharu-pipeline/src/stages/detection.rs` คำนวณอัตราส่วนเส้นรอบรูปต่อพื้นที่ (Isoperimetric Quotient / Perimeter-to-Area Ratio) และ Convexity Defects จาก Contour ของ RF-DETR Mask เพื่อจำแนกรูปทรงบอลลูน
  3. นิยาม `TypesettingPreset` ใน `crates/koharu-renderer` และนำสไตล์ไปใช้กับ `TextLayer` อัตโนมัติเมื่อจบสเตจการแปล (โดยไม่ทับค่าที่ผู้ใช้เคยปรับแต่งด้วยตัวเองแล้ว)
  4. เพิ่ม UI ตั้งค่าฟอนต์ของแต่ละ `BalloonShape` ในหน้า Preferences และเพิ่ม Dropdown สำหรับ Override รูปทรงใน `Inspector.tsx`
- **acceptance criteria:**
  - บอลลูนทรงหยัก (`Shout`), ทรงสี่เหลี่ยม (`Narration`), และทรงวงรี (`NormalSpeech`) บนชุดข้อมูลทดสอบถูกจำแนกและจับคู่ฟอนต์ได้ตรงตาม Preset
  - ผู้ใช้สามารถเปลี่ยน `BalloonShape` ใน `Inspector.tsx` แล้วสไตล์ของข้อความอัปเดตตามทันที
- **tests to add or update:**
  - เพิ่ม Unit Tests ใน `crates/koharu-pipeline` ทดสอบฟังก์ชันคำนวณรูปทรงเรขาคณิตด้วยรูปหลายเหลี่ยมจำลอง (Synthetic Polygons: วงรี, สี่เหลี่ยมผืนผ้า, ดาวหยัก)
- **verification commands supported by context:**
  - `cargo test --workspace` (ใน `c:\Users\T3R0\koharu`)
  - `bun test` (ใน `packages/koharu`)
- **risks and conditions that should stop implementation:**
  - หยุดทำงานหากพบว่าเอาต์พุตของ `koharu-layout-rfdetr-seg-2xl-1152` ใน `detection.rs` ให้เฉพาะกรอบสี่เหลี่ยม (Bounding Box) สำหรับข้อความแต่ไม่ได้ให้ Mask ของตัวบอลลูนคำพูด

### Task 5: `TASK-P3-SFX-PIPELINE-AND-CANVAS` (Phase 3 - SFX Inpainting, Translation & Bezier Rendering)
- **task_id:** `TASK-P3-SFX-PIPELINE-AND-CANVAS`
- **objective:** แยกเลเยอร์เสียงประกอบ (`LayerKind::Sfx`) ปรับปรุงการ Inpaint รักษาสกรีนโทน เพิ่มการแปลคำเลียนเสียงธรรมชาติ และรองรับการวาดข้อความตามเส้นโค้ง Bezier พร้อมขอบตัวอักษรบน WebGPU Canvas
- **depends_on:** `TASK-P2-BALLOON-TYPESETTING`
- **existing files to inspect:**
  - `crates/koharu-pipeline/src/stages/detection.rs`
  - `crates/koharu-pipeline/src/stages/inpainting.rs`
  - `crates/koharu-translator/src/lib.rs`
  - `crates/koharu-renderer/src/lib.rs`
  - `crates/koharu-canvas/src/lib.rs`
  - `crates/koharu-psd/src/lib.rs`
  - `packages/koharu/components/editor/Inspector.tsx`
- **proposed files to create or modify:**
  - **[MODIFY]** `crates/koharu-scene` (เพิ่ม `LayerKind::Sfx` และพารามิเตอร์เส้นโค้ง Bezier / Multi-stroke บน Text Layer)
  - **[MODIFY]** `crates/koharu-pipeline/src/stages/detection.rs`
  - **[MODIFY]** `crates/koharu-pipeline/src/stages/inpainting.rs`
  - **[MODIFY]** `crates/koharu-translator` (เพิ่มตารางเทียบคำ Onomatopoeia และ Prompt เฉพาะสำหรับ SFX)
  - **[MODIFY]** `crates/koharu-renderer` และ `crates/koharu-canvas` (เพิ่มการวาง Glyph บนเส้นโค้ง Bezier และการวาดขอบหลายชั้นด้วย Vello)
  - **[MODIFY]** `packages/koharu/components/editor/Inspector.tsx`
- **affected symbols or contracts:** `LayerKind::Sfx`, Bezier Path Controls, Multi-pass Stroke Config
- **implementation outline:**
  1. แท็กเลเยอร์ข้อความที่อยู่นอกบอลลูนหรือตรงกับคลาส SFX ใน `detection.rs` ให้เป็น `LayerKind::Sfx`
  2. ใน `inpainting.rs` เพิ่ม Adaptive Morphological Dilation ให้กับ Mask ของ `LayerKind::Sfx` เพื่อคลุมขอบตัวอักษรและเงา
  3. เพิ่มตารางเทียบคำเลียนเสียงธรรมชาติ (Onomatopoeia Lookup Table) ในระบบแปลภาษา
  4. ใน `koharu-renderer` และ `koharu-canvas` คำนวณตำแหน่งและมุมหมุนของแต่ละ Glyph ตามเส้นโค้ง Cubic/Quadratic Bezier และวาด Stroke ชั้นนอกก่อนวาด Fill ชั้นในด้วย `vello`
  5. เพิ่มจุดควบคุมเส้นโค้ง (Curve Handles) และตัวปรับความหนา/สีขอบใน `Inspector.tsx`
- **acceptance criteria:**
  - เลเยอร์ `LayerKind::Sfx` ถูกลบพื้นหลังอย่างสะอาดและแสดงผลข้อความแปลตามเส้นโค้งพร้อมขอบตัวอักษรบน WebGPU Canvas ได้อย่างถูกต้อง
- **tests to add or update:**
  - เพิ่ม Unit Tests สำหรับการคำนวณตำแหน่งจุดและมุมสัมผัส (Tangent Angle) บนเส้นโค้ง Bezier ใน `crates/koharu-renderer`
  - เพิ่ม Unit Tests สำหรับ Morphological Dilation ของ SFX Mask ใน `crates/koharu-pipeline`
- **verification commands supported by context:**
  - `cargo check` (ใน `c:\Users\T3R0\koharu`)
  - `cargo test --workspace` (ใน `c:\Users\T3R0\koharu`)
  - `bun test` (ใน `packages/koharu`)
- **risks and conditions that should stop implementation:**
  - หยุดทำงานหากการเรนเดอร์ข้อความดัดโค้งใน `koharu-canvas` ทำให้การส่งออกไฟล์ผ่าน `crates/koharu-psd` ล้มเหลวหรือเกิดความคลาดเคลื่อนของตำแหน่งเลเยอร์อย่างรุนแรง

---

## 7. Compatibility, Migration, and Rollback

### Backward Compatibility
- ตามกฎเหล็กข้อที่ 1 ใน `GEMINI_CONTEXT.md` (Section 12: *Never Add Backward Compatibility*) การเปลี่ยนแปลงโครงสร้างข้อมูลใน `crates/koharu-scene` และสัญญา IPC ใน `crates/koharu-app` จะไม่สร้างตัวแปลงข้อมูลย้อนหลังหรือฟิลด์สำรอง Consumer ทุกส่วนใน Monorepo ต้องได้รับการอัปเดตให้ตรงกับ Schema ใหม่พร้อมกัน

### Data Migration Needs
- โปรเจกต์ที่บันทึกอยู่ใน `koharu-storage` ก่อนการเปลี่ยนแปลงนี้อาจมีโครงสร้าง `Snapshot` / `Patch` ไม่ตรงกับ Schema ใหม่ ต้องให้ผู้ใช้ยืนยันก่อนเริ่มงานว่าอนุญาตให้ล้างหรือสร้างโปรเจกต์ใหม่สำหรับทดสอบได้

### Deployment Ordering
1. อัปเดตและทดสอบ `crates/koharu-scene`
2. อัปเดตและทดสอบ `crates/koharu-pipeline`, `crates/koharu-translator`, `crates/koharu-renderer`, `crates/koharu-canvas`
3. อัปเดต `crates/koharu-app` และสร้าง TypeScript Bindings ใหม่ใน `packages/bridge/src/protocol.ts`
4. อัปเดตคอมโพเนนต์ใน `packages/koharu`

### Rollback Limitations
- โค้ดสามารถย้อนกลับ (Rollback) ได้อย่างปลอดภัยระดับเฟสด้วย `git revert` หรือรีเซ็ตกลับไปยัง Commit `61b5421d9959ec2096d345cc406fe90f3aecbbd7`
- อย่างไรก็ตาม ข้อมูลโปรเจกต์ที่ถูกเขียนลง Local AppData (`koharu-storage`) ด้วย Schema ใหม่จะไม่สามารถเปิดด้วยไบนารีของ Commit เก่าได้

### Changes Requiring Additional User Approval
- การเปลี่ยนแปลงใดๆ ที่กระทบต่อรูปแบบการส่งออกไฟล์ PSD ใน `crates/koharu-psd`
- การเพิ่ม Dependency ภายนอกตัวใหม่ลงใน `Cargo.toml` หรือ `package.json`

---

## 8. Verification Plan

### Automated Unit & Integration Checks
- **การตรวจสอบชนิดข้อมูลและการคอมไพล์ฝั่ง Rust (ยืนยันจากบริบท Section 13):**
  - คำสั่ง: `cargo check`
  - ไดเรกทอรีทำงาน: `c:\Users\T3R0\koharu`
- **การรันชุดทดสอบ Rust ทั้ง Workspace (ยืนยันจากบริบท Section 13):**
  - คำสั่ง: `cargo test --workspace`
  - ไดเรกทอรีทำงาน: `c:\Users\T3R0\koharu`
- **การรันชุดทดสอบ Frontend (ยืนยันจากบริบท Section 13):**
  - คำสั่ง: `bun test`
  - ไดเรกทอรีทำงาน: `c:\Users\T3R0\koharu\packages\koharu`
- **คำสั่งเฉพาะ Crate ที่เสนอเพิ่มเติม (Proposed / Unconfirmed in Section 13 table):**
  - `cargo test -p koharu-scene` (ใน `c:\Users\T3R0\koharu`)
  - `cargo test -p koharu-pipeline` (ใน `c:\Users\T3R0\koharu`)
  - `cargo test -p koharu-app` (ใน `c:\Users\T3R0\koharu`)

### Negative Cases and Invariant Checks
- **Optimistic Concurrency Rejection:** ทดสอบส่งคำสั่ง `set_bubble_speaker` และ `update_character_profile` ด้วยค่า `expectedRevision` ที่ล้าสมัย ระบบต้องปฏิเสธการแก้ไขและคืนค่าข้อผิดพลาดโดยไม่เปลี่ยนแปลง `Snapshot`
- **Orphaned Speaker Reference:** ทดสอบลบ `GlossaryEntry` ที่ถูกผูกเป็น `Speaker` ของกล่องข้อความ ระบบต้องล้างค่า `Speaker` ให้เป็น `None` โดยไม่เกิด Panic
- **Context Window Overflow Guard:** ทดสอบส่งหน้ามังงะที่มีกล่องข้อความจำนวนมากเข้าสเตจการแปลพร้อม `context_window_pages = 2` ระบบต้องตัดทอนบริบทให้อยู่ในงบประมาณ Token เสมอ

### Manual Checks (via Desktop CEF & Chrome DevTools MCP)
- รันแอปพลิเคชันด้วยคำสั่ง `bun dev` (ใน `c:\Users\T3R0\koharu`) และเชื่อมต่อ Chrome DevTools MCP ที่ `http://127.0.0.1:4000` (ตามกฎใน Section 9, 10, 12):
  1. **ตรวจสอบ Phase 1:** สร้างตัวละคร 2 ตัวใน `GlossaryPanel.tsx` ด้วยสรรพนามต่างกัน ผูกผู้พูดใน `Inspector.tsx` และรันการแปลระดับ Chapter เพื่อยืนยันความสม่ำเสมอของสรรพนามข้ามหน้า
  2. **ตรวจสอบ Phase 2:** รัน Detection บนหน้ามังงะที่มีทั้งบอลลูนตะโกนและกล่องบรรยาย ยืนยันว่าฟอนต์เปลี่ยนตาม `BalloonShape` และสามารถ Override ได้ใน `Inspector.tsx`
  3. **ตรวจสอบ Phase 3:** รัน Inpaint และแปลหน้าที่มีเสียงประกอบ (SFX) ยืนยันว่าพื้นหลังถูกลบอย่างสะอาดและตัวอักษรวางตัวตามเส้นโค้ง Bezier บน WebGPU Canvas

---

## 9. Completion Evidence

### 1. Changed Artifacts & Diffstat
- **Branch:** `fix/manga-suite-review`
- **Base commit:** `61b5421d`
- **Protocol generation:** `packages/bridge/src/protocol.ts` regenerated authoritative via `cargo run -p koharu-app --bin generate`
- **Key crates modified:**
  - `crates/koharu-scene`: GlossaryEntry revision bump (schema 2), sequential state accumulation in `prune_missing`, speaker provenance.
  - `crates/koharu-pipeline`: scanline polygon rasterization in inpainting, entity-scoped SFX dilation, mask kind filtering, non-destructive re-detection, whisper balloon contour classification, removed blind glossary replace.
  - `crates/koharu-app`: orphaned speaker cleanup across text layers and content, domain logic separation from RPC.
  - `packages/koharu`: Inspector optimistic concurrency conflict recovery, controlled persona inputs, full 9-locale translation synchronization, E2E test suite additions.

### 2. Verification Results
- `cargo check --workspace`: **PASS** (Finished dev profile, 0 errors)
- `cargo test -p koharu-scene`: **PASS** (84/84 tests passed: 44 unit, 5 adversarial, 1 smoke, 17 e2e, 14 empirical challenge, 3 doctests)
- `cargo test -p koharu-pipeline`: **PASS** (69/69 tests passed, 0 failures)
- `cargo test -p koharu-app`: **PASS** (28/28 tests passed, 0 failures)
- `cargo test -p koharu-translator`: **PASS** (49/49 tests passed, 0 failures)
- `bun run test` (`packages/koharu`): **PASS** (19 test files, 190/190 tests passed, 0 failures)
- **E2E Scenarios (`tier4-scenarios.test.tsx`):** **PASS** (Multi-character chapter translation, balloon shape typography, rapid concurrent edits)

### 3. Documentation Updates
- `GEMINI_CONTEXT.md` Sections 6, 7, 14, 15 verified consistent with current scene models, IPC command contracts, and pipeline stages.
- `plan.md` updated with completed checklist and verification records.

### 4. Remaining Limitations
- **Bezier curved text rendering:** Text-on-path Bezier calligraphy and multi-pass stroke on `koharu-canvas` are planned in renderer enhancement follow-up.
- **PSD curved text export:** Exporting curved vector text layers to standard Adobe PSD format remains constrained to rasterized layer fallback.
- **Contour classification on torn contours:** Severely degraded or open hand-drawn contours fall back safely to `NormalSpeech` with baseline confidence.

---

## 10. Approval Gate

> [!IMPORTANT]
> - **สถานะเอกสาร:** เอกสารฉบับนี้มีสถานะเป็นเพียงฉบับร่าง (`status: DRAFT`, `implementation_approved: false`) **ไม่ใช่คำสั่งอนุมัติให้รันการแก้ไขโค้ด (Not Execution Authorization)**
> - **การตรวจสอบคลังโค้ดซ้ำ (Repository Revalidation):** ก่อนเริ่มดำเนินการ Antigravity ต้องตรวจสอบว่าสถานะ Git ปัจจุบันยังคงอยู่ที่ Commit `61b5421d9959ec2096d345cc406fe90f3aecbbd7` (สถานะ `CLEAN`) และต้องอ่านไฟล์ต้นฉบับที่ระบุในส่วน Missing Evidence เพื่อยืนยันโครงสร้าง Struct/Trait และหมายเลขบรรทัดจริงทั้งหมด
> - **การขออนุมัติจากผู้ใช้ (Explicit User Approval):** ห้ามแก้ไขไฟล์ใดๆ ในคลังโค้ดจนกว่าผู้ใช้จะตอบคำถาม Blocking Questions และสั่งอนุมัติการลงมือทำอย่างชัดเจน (`implementation_approved: true`)