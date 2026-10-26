# AGY_CHANGE_REQUEST.md

## 1. Request Metadata
- **request_id:** `AGY-CR-20261026-001`
- **status:** `IMPLEMENTED`
- **based_on_snapshot_id:** `koharu-549857f-dirty-20261026`
- **base_commit:** `549857f533aa40d9d7010f33fa468a106efedcbb` (Branch: `feature/chapter-workflow-and-detection`)
- **relevant_source_hashes (SHA-256):**
  - `Cargo.toml`: `B929C7EA2B77CAA78C7BE703FE57E7CC8FBC7980E72C69C6CB4320442C4DD7BB`
  - `crates/koharu/src/main.rs`: `E357186332E8EFAB48CC107D715BF2AF745BD7C9B45DF178601431204F45D29E`
  - `crates/koharu-app/src/lib.rs`: `5AE801958C706511C54ACE74103621F3EB247216E61D861919EBB2A2CA1E209A`
  - `packages/bridge/src/protocol.ts`: `3220C4CCDD44FD595347BBDE9C8CF833038FA6DD28DE19D96068E5E82B863676`
- **implementation_approved:** `true`

---

## 2. Goal and Success Criteria

### User-Visible Outcome
1. ผู้ใช้สามารถนำเข้า ซิงค์ และจัดการคำศัพท์ในพจนานุกรม (`GlossaryPanel.tsx`) โดยมีผลต่อการแปลในระดับหน้าและระดับตอน (Chapter) อย่างถูกต้อง
2. ผู้ใช้สามารถสั่งประมวลผลมังงะทั้งตอน (Batch Chapter Processing) พร้อมสั่งหยุดชั่วคราวและทำต่อ (Pause/Resume) ได้โดยที่การ์ดจอที่มี VRAM 8GB ไม่เกิดอาการค้างหรือ Out of Memory
3. เมื่อผู้ใช้แก้ไขข้อความหรือขยับกล่องข้อความ (`Text Box`) ใน `Inspector.tsx` ภาพพื้นหลังที่ผ่านการระบายลบคำ (`RasterLayer` จากการ Inpaint) จะไม่ถูกล้างหรือต้องประมวลผล Inpaint ใหม่โดยไม่จำเป็น

### Business Outcome
- ยกระดับ `Koharu` จากเครื่องมือแปลทีละหน้า (Single-page workflow) ไปสู่เครื่องมือแปลระดับตอน (Production Chapter Workflow) ที่มีความสม่ำเสมอของคำศัพท์สูงขึ้น ลดเวลาการทำงานซ้ำของนักแปล และรองรับฮาร์ดแวร์ระดับกลาง (8GB VRAM GPUs) ได้อย่างมีเสถียรภาพ

### Concrete Acceptance Criteria
1. การทำงานของ `apply_glossary_import` และ `add_glossary_entry` ใน `crates/koharu-scene/src/components/glossary.rs` และ `crates/koharu-app/src/commands/glossary/` ต้องตรวจสอบ `expectedRevision`, `Language Tag`, และโครงสร้างคำศัพท์อย่างครบถ้วน
2. `crates/koharu-pipeline` ต้องรองรับการรันงานระดับ Chapter พร้อมความสามารถในการ Pause/Resume ข้ามหน้า และปลดโหลดโมเดลที่ไม่ได้ใช้งานออกจาก VRAM ตามกฎ `VRAM and Allocation Discipline`
3. การแก้ไข `Text Layer` หรือ `Text Box` ใน `crates/koharu-scene` ต้องไม่ทำลายข้อมูล `RasterLayer` ของภาพ Inpainted ที่แคชไว้ เว้นแต่ผู้ใช้จะสั่ง `commit_inpaint` หรือรัน Stage Inpainting ใหม่โดยตรง
4. การเปลี่ยนแปลงสัญญา IPC ทั้งหมดต้องถูกสร้างใหม่ลง `packages/bridge/src/protocol.ts` ผ่าน `tauri-specta` เท่านั้น (ห้ามแก้ไขไฟล์ `protocol.ts` ด้วยมือ) และไม่มีการสร้าง Backward-compatibility layer ใดๆ
5. ชุดทดสอบ `cargo check`, `cargo test --workspace`, และ `bun test` ต้องผ่านทั้งหมดโดยไม่มีข้อผิดพลาด

---

## 3. Scope and Non-goals

### Included Behavior
- การทำความสะอาดและทำให้ฟีเจอร์ Glossary Sync & Import และ Chapter Workflow ในกิ่ง `feature/chapter-workflow-and-detection` เสร็จสมบูรณ์ (`crates/koharu-scene`, `crates/koharu-app`, `packages/koharu`)
- การปรับปรุง Scheduler และการจัดการ VRAM ใน `crates/koharu-pipeline` และ `crates/koharu-app/src/commands/processing.rs` สำหรับ Batch Chapter Processing
- การทำ Mask Caching Optimization ใน `crates/koharu-scene/src/components.rs` และ `crates/koharu-renderer` เพื่อแยกการอัปเดต Text Box ออกจาก Inpainted `RasterLayer`

### Explicitly Excluded Behavior (Non-goals for this CR)
- การแก้ไขโค้ดภายใน C/C++ FFI bindings (`crates/koharu-llama-sys`, `crates/koharu-diffusion-sys`, `crates/koharu-torch-sys`)
- การเปลี่ยนระบบเรนเดอร์หลักจาก `koharu-canvas` (WebGPU/Vello) ไปเป็นอย่างอื่น (การทำ Fallback ไปยัง `crates/koharu-rasterizer` และการแยก Headless CLI ใน `crates/koharu/src/main.rs` ให้แยกเป็น Change Request ถัดไปหากผู้ใช้ยังไม่สั่งรวมในรอบนี้)

### Compatibility and Architectural Constraints
- **Never Add Backward Compatibility:** ห้ามสร้าง Compatibility Parser, Type Aliases หรือ Forwarding Functions เมื่อปรับแก้ API ใน `crates/koharu-scene`, `crates/koharu-pipeline`, หรือ `crates/koharu-app` ให้แก้ไข Consumer ทั้งหมดใน Workspace ทันที
- **Generated Source Boundary:** ห้ามแก้ไข `packages/bridge/src/protocol.ts` ด้วยมือโดยเด็ดขาด
- **Optimistic Concurrency Control:** ทุกคำสั่งที่แก้ไขสถานะใน `crates/koharu-scene` ต้องตรวจสอบ `expectedRevision` และบันทึกเป็น Immutable `Snapshot` / Append-only `Patch` ลง `crates/koharu-storage`
- **VRAM Discipline:** โค้ดส่วน ML ต้องรับ Device Abstraction ที่ขอบเขตภายนอกเพียงครั้งเดียว และปิด Gradient Tracking เสมอขณะ Inference

---

## 4. Current-State Evidence

### Relevant Existing Modules, Interfaces, and Flows
- **Entry & Runtime (`FACT`):** `crates/koharu/src/main.rs` เริ่มการทำงานด้วย `#[tauri_runtime_cef::cef_entry_point]` และเรียก `crates/koharu-app/src/app.rs` ซึ่งเปิดพอร์ต CEF Remote Debugging ที่ `127.0.0.1:4000` ในโหมด Debug
- **Scene & Glossary State (`FACT`):** `crates/koharu-scene` จัดการ `Snapshot`, `EntityId`, `Revision`, `Page`, `Layer` (แยกเป็น Raster Layer และ Text Layer), และ `Glossary` / `GlossaryEntry` (`crates/koharu-scene/src/components/glossary.rs`, `crates/koharu-scene/src/components.rs`)
- **IPC Commands (`FACT`):** `crates/koharu-app/src/commands/` นิยามคำสั่ง `subscribe`, `select_page`, `process(scope: Scope, operation: Operation)`, `commit_inpaint`, `add_glossary_entry`, `export`, และ `export_chapter` โดยกำลังมีการแก้ไขใน `crates/koharu-app/src/commands/glossary/` และ `crates/koharu-app/src/commands/processing.rs`
- **Frontend UI (`FACT`):** `packages/koharu/components/` มี `PageRail.tsx`, `CanvasCommandBar.tsx`, `Inspector.tsx`, `GlossaryPanel.tsx`, `InferenceControl.tsx` และไลบรารี `packages/koharu/lib/glossary.ts`

### Evidence References
- `GEMINI_CONTEXT.md` Section 1 (Snapshot Metadata & Checksums)
- `GEMINI_CONTEXT.md` Section 3 & 4 (Module Map & Architectural Layering)
- `GEMINI_CONTEXT.md` Section 6 & 7 (Data Models, Business Invariants, and Tauri Specta IPC Contracts)
- `GEMINI_CONTEXT.md` Section 12 (Project Rules and Change Constraints)
- `GEMINI_CONTEXT.md` Section 14 & 15 (Current Status, Technical Debt, and Change-Impact Map)

### Assumptions and Unresolved Questions
- **[ASSUMPTION - Non-blocking]:** สมมติว่ากลไกสร้างไฟล์ `packages/bridge/src/protocol.ts` ทำงานอัตโนมัติเมื่อรันโหมดพัฒนาหรือเทสต์ของ `crates/koharu-app` ผ่าน `tauri-specta`
- **[UNRESOLVED - Blocking]:** ต้องตรวจสอบเนื้อหาไฟล์จริงของ `crates/koharu-scene/src/components/glossary.rs`, `crates/koharu-scene/src/components.rs`, `crates/koharu-app/src/commands/processing.rs`, และ `crates/koharu-pipeline` ที่อยู่ในสถานะ `DIRTY` ก่อนเริ่มแก้ไขจริง เพื่อไม่ให้ทับงานที่ผู้ใช้กำลังเขียนค้างไว้

---

## 5. Proposed Design

### Proposed Changes and Rationale
1. **Domain-Level Glossary Validation & Sync (`crates/koharu-scene` & `packages/koharu`):**
   - ย้ายตรรกะการตรวจสอบความถูกต้องของการนำเข้าคำศัพท์ (`apply_glossary_import`) และการจับคู่คำศัพท์ (`GlossaryEntry`) ให้จบใน `crates/koharu-scene/src/components/glossary.rs` เพื่อให้ `crates/koharu-app/src/commands/glossary/` ทำหน้าที่เป็นเพียงบางๆ (Thin IPC Adapter) ลดปัญหา IPC RPC Bloat
2. **Stage-Aware Batch Scheduler & VRAM Eviction (`crates/koharu-pipeline` & `crates/koharu-app`):**
   - ปรับปรุง `Pipeline::load` และตัวจัดการ `Request` ใน `crates/koharu-pipeline` ให้รองรับการควบคุมสถานะงานระดับ Chapter (Queueing, Pause, Resume, Cancel) และเพิ่มการสั่งปลดโหลดโมเดลออกจาก VRAM เมื่อสลับ Stage เพื่อป้องกัน VRAM Exhaustion บนการ์ดจอ 8GB
3. **Decoupled Inpaint Mask & Raster Caching (`crates/koharu-scene` & `crates/koharu-renderer`):**
   - แยกการอ้างอิงข้อมูล `DetectionAnalysis` และ Mask ของ `RasterLayer` ที่ผ่านการ Inpaint ออกจาก `Text Layer` ใน `crates/koharu-scene/src/components.rs` เพื่อให้การแก้ไข `Translation` หรือการจัดวางข้อความใน `crates/koharu-renderer` ใช้ภาพ Inpainted เดิมจากแคชได้ทันที

### Data, API, UI, Authorization, and Operational Impacts
- **Data Impact:** รักษา Invariant การแยก `SourceText` กับ `Translation` และการบันทึกผ่าน `Patch` ลง `crates/koharu-storage`
- **API Impact:** อัปเดตชนิดข้อมูลใน `crates/koharu-app/src/commands/glossary/` และ `crates/koharu-app/src/commands/processing.rs` แล้วซิงค์ไปยัง `packages/bridge/src/protocol.ts`
- **UI Impact:** อัปเดต `packages/koharu/components/GlossaryPanel.tsx`, `packages/koharu/lib/glossary.ts`, และ `packages/koharu/components/InferenceControl.tsx` ให้แสดงสถานะความคืบหน้าและการนำเข้าคำศัพท์ตาม Contract ใหม่
- **Security/Auth Impact:** ไม่กระทบการจัดเก็บ API Keys ใน `crates/koharu-secrets` และไม่ส่งภาพมังงะออกนอกเครื่องในการรันโมเดล Local

### Distinguish Current Components from Proposed Components
- **Existing Files to Modify:**
  - `crates/koharu-scene/src/components/glossary.rs`
  - `crates/koharu-scene/src/components.rs`
  - `crates/koharu-renderer` (ไฟล์ที่รับผิดชอบการประกอบเลเยอร์ข้อความและภาพพื้นหลัง)
  - `crates/koharu-pipeline` (โมดูล Scheduler และ `Pipeline::load` / `Request`)
  - `crates/koharu-app/src/commands/glossary/`
  - `crates/koharu-app/src/commands/processing.rs`
  - `packages/bridge/src/protocol.ts` (อัปเดตผ่าน Generator เท่านั้น)
  - `packages/koharu/lib/glossary.ts`
  - `packages/koharu/components/GlossaryPanel.tsx`
- **Proposed New Files (`NEW`):**
  - ไม่มีไฟล์ใหม่ที่บังคับสร้าง (แนะนำให้ขยายภายในโมดูลที่มีอยู่เดิมของแต่ละ Crate ก่อน ยกเว้นเมื่อตรวจสอบโค้ดจริงใน `crates/koharu-pipeline` แล้วพบว่าต้องแยกไฟล์ย่อยสำหรับ VRAM Eviction Policy)

---

## 6. Dependency-Ordered Tasks

### Task 1: Inspect Dirty Working Tree and Complete Glossary Domain Logic
- **task_id:** `TASK-001`
- **objective:** ตรวจสอบการแก้ไขที่ค้างอยู่ในกิ่ง `feature/chapter-workflow-and-detection` และทำให้ Domain Logic ของ `GlossaryView` และ `apply_glossary_import` ใน `crates/koharu-scene` เสร็จสมบูรณ์พร้อม Unit Tests
- **depends_on:** `None`
- **existing_files_to_inspect:**
  - `crates/koharu-scene/src/components/glossary.rs`
  - `crates/koharu-app/src/commands/glossary/`
  - `packages/koharu/lib/glossary.ts`
  - `packages/koharu/components/GlossaryPanel.tsx`
- **proposed_files_to_create_or_modify:**
  - `[MODIFY]` `crates/koharu-scene/src/components/glossary.rs`
  - `[MODIFY]` `crates/koharu-app/src/commands/glossary/`
  - `[MODIFY]` `packages/koharu/lib/glossary.ts`
  - `[MODIFY]` `packages/koharu/components/GlossaryPanel.tsx`
  - `[GENERATED]` `packages/bridge/src/protocol.ts`
- **affected_symbols_or_contracts:** `Glossary`, `GlossaryEntry`, `GlossaryView`, `apply_glossary_import`, `add_glossary_entry`
- **implementation_outline:**
  1. ตรวจสอบ `git status` และ `git diff` ในโฟลเดอร์ที่เกี่ยวข้องเพื่อทำความเข้าใจโค้ดที่ค้างอยู่
  2. ทำให้ฟังก์ชัน `apply_glossary_import` ตรวจสอบความเข้ากันได้ของ `Language Tag`, กรองคำซ้ำ และผูกกับการสร้าง Revision `Patch` อย่างถูกต้อง
  3. ปรับให้ IPC handler ใน `crates/koharu-app/src/commands/glossary/` เรียกใช้ฟังก์ชันจาก `crates/koharu-scene` โดยตรงโดยไม่ใส่ Business Logic ซ้ำซ้อน
  4. อัปเดต `packages/koharu/lib/glossary.ts` และ `packages/koharu/components/GlossaryPanel.tsx` ให้ตรงกับ Type ใน `packages/bridge/src/protocol.ts`
- **acceptance_criteria:**
  - การเพิ่มและนำเข้าคำศัพท์ผ่าน `add_glossary_entry` และ `apply_glossary_import` ตรวจสอบ `expectedRevision` และคืนค่า `GlossaryView` ที่ถูกต้อง
- **tests_to_add_or_update:**
  - เพิ่ม Unit Tests ใน `crates/koharu-scene/src/components/glossary.rs` สำหรับการนำเข้าคำศัพท์ การชนกันของคำศัพท์ และการตรวจสอบ Language Tag
  - เพิ่ม/อัปเดตเทสต์ใน `packages/koharu` สำหรับ `packages/koharu/lib/glossary.ts`
- **verification_commands:**
  - `cargo check` (ใน `c:\Users\T3R0\koharu`)
  - `cargo test --workspace` (ใน `c:\Users\T3R0\koharu`)
  - `bun test` (ใน `packages/koharu`)
- **risks_and_stop_conditions:**
  - **Stop Condition:** หยุดทันทีหากพบว่าโครงสร้าง `Patch` ใน `crates/koharu-storage` ขัดแย้งกับการแก้ไขที่ค้างอยู่ใน Working Tree และแจ้งให้ผู้ใช้ตัดสินใจ

### Task 2: Mask Caching Optimization for Inpainted Raster Layers
- **task_id:** `TASK-002`
- **objective:** ป้องกันไม่ให้การแก้ไข `Text Box` หรือการจัดวางข้อความไปล้างผลลัพธ์ `RasterLayer` ที่ผ่านการ Inpaint แล้ว
- **depends_on:** `TASK-001`
- **existing_files_to_inspect:**
  - `crates/koharu-scene/src/components.rs`
  - `crates/koharu-renderer`
  - `crates/koharu-app/src/commands/` (ส่วนที่จัดการ `commit_inpaint` และ `add_text_box`)
- **proposed_files_to_create_or_modify:**
  - `[MODIFY]` `crates/koharu-scene/src/components.rs`
  - `[MODIFY]` `crates/koharu-renderer` (ไฟล์ที่เกี่ยวข้องกับการเรนเดอร์เลเยอร์)
- **affected_symbols_or_contracts:** `DetectionAnalysis`, `RasterLayer`, `Page`, `Layer`, `commit_inpaint`, `add_text_box`
- **implementation_outline:**
  1. ตรวจสอบความสัมพันธ์ระหว่าง `DetectionAnalysis`, `RasterLayer`, และ `Text Layer` ใน `crates/koharu-scene/src/components.rs`
  2. แยกการ Invalidate แคชของ `RasterLayer` ออกจากเหตุการณ์แก้ไขข้อความหรือขอบเขต `Text Box` เพื่อให้ผลจาก `commit_inpaint` และ Stage 4 Inpainting คงอยู่เสมอเมื่อแก้ไขเฉพาะข้อความ
  3. ปรับ `crates/koharu-renderer` ให้ดึง `RasterLayer` ที่แคชไว้มาประกอบกับ `Text Layer` ใหม่โดยไม่สั่งล้าง Mask เดิม
- **acceptance_criteria:**
  - เมื่อเรียก `commit_inpaint` แล้วตามด้วยการแก้ไข `Text Box` หรือข้อความแปล ภาพพื้นหลังที่ Inpaint ไว้ต้องไม่ถูกรีเซ็ตกลับเป็นภาพต้นฉบับ
- **tests_to_add_or_update:**
  - เพิ่ม Unit Test ใน `crates/koharu-scene` เพื่อยืนยันว่าการสร้าง Patch แก้ไข Text Box ไม่เปลี่ยนแปลง Blob reference ของ Inpainted `RasterLayer`
- **verification_commands:**
  - `cargo check` (ใน `c:\Users\T3R0\koharu`)
  - `cargo test --workspace` (ใน `c:\Users\T3R0\koharu`)
- **risks_and_stop_conditions:**
  - **Stop Condition:** หยุดทำงานหากการเปลี่ยนแปลงกระทบโครงสร้างการจัดเก็บ Blob พื้นฐานใน `crates/koharu-storage` ที่ต้องทำ Data Migration

### Task 3: Batch Chapter Processing Scheduler & VRAM Eviction Control
- **task_id:** `TASK-003`
- **objective:** รองรับการประมวลผลระดับ Chapter ใน `crates/koharu-pipeline` และ `crates/koharu-app/src/commands/processing.rs` พร้อมกลไก Pause/Resume และการคืน VRAM ระหว่าง Stage
- **depends_on:** `TASK-001`, `TASK-002`
- **existing_files_to_inspect:**
  - `crates/koharu-pipeline` (ไฟล์ที่มีนิยาม `Pipeline::load`, `Request`, และ Scheduler)
  - `crates/koharu-app/src/commands/processing.rs`
  - `packages/koharu/components/InferenceControl.tsx`
- **proposed_files_to_create_or_modify:**
  - `[MODIFY]` `crates/koharu-pipeline`
  - `[MODIFY]` `crates/koharu-app/src/commands/processing.rs`
  - `[MODIFY]` `packages/koharu/components/InferenceControl.tsx`
  - `[GENERATED]` `packages/bridge/src/protocol.ts`
- **affected_symbols_or_contracts:** `Pipeline::load`, `Request`, `Scope`, `Operation`, `process`
- **implementation_outline:**
  1. ตรวจสอบการทำงานของ `Pipeline::load` และการส่ง `Request` ใน `crates/koharu-pipeline`
  2. เพิ่มการจัดการวงจรชีวิตของโมเดลในแต่ละ Stage ให้สามารถปลดโหลดโมเดลออกจาก VRAM ก่อนเริ่ม Stage ถัดไปเมื่อรันงานระดับ Chapter
  3. เพิ่มการตรวจสอบสัญญาณ Pause/Cancel ระหว่างหน้าใน `crates/koharu-pipeline` และเชื่อมต่อกับ `crates/koharu-app/src/commands/processing.rs`
  4. อัปเดต消费者ทั้งหมดใน `packages/koharu` หากมีการเปลี่ยน Signature ของคำสั่ง IPC
- **acceptance_criteria:**
  - สามารถสั่งรัน `process` ในระดับ Chapter ได้ สามารถสั่งหยุดชั่วคราว/ทำต่อข้ามหน้าได้ และโมเดลแต่ละ Stage คืนหน่วยความจำ VRAM ตามที่กำหนด
- **tests_to_add_or_update:**
  - เพิ่ม Unit/Integration Tests ใน `crates/koharu-pipeline` สำหรับลำดับคิวงานระดับ Chapter, การสั่ง Pause/Resume, และการสลับสถานะโมเดล
- **verification_commands:**
  - `cargo check` (ใน `c:\Users\T3R0\koharu`)
  - `cargo test --workspace` (ใน `c:\Users\T3R0\koharu`)
  - `bun test` (ใน `packages/koharu`)
- **risks_and_stop_conditions:**
  - **Stop Condition:** หยุดทันทีหากพบว่า C/C++ bindings (`crates/koharu-llama-sys`, `crates/koharu-diffusion-sys`, `crates/koharu-torch-sys`) ไม่คืน VRAM เมื่อ Drop struct ใน Rust และรายงานให้ผู้ใช้ทราบ

---

## 7. Compatibility, Migration, and Rollback

### Backward Compatibility
- ตามกฎเหล็กของโปรเจกต์ (`Never Add Backward Compatibility` ใน `AGENTS.md`) จะไม่มีการสร้าง Compatibility Layer หรือ Deprecated Alias ใดๆ การเปลี่ยนแปลง Type หรือ Function Signature จะถูกอัปเดตทั้งใน Rust crates และ TypeScript packages พร้อมกันในคราวเดียว

### Data Migration Needs
- ไม่ต้องทำ Data Migration สำหรับฐานข้อมูลภายนอก เนื่องจากโปรเจกต์ใช้ `crates/koharu-storage` แบบ Content-addressed chunks และ `Patch` ในเครื่อง

### Deployment Ordering
1. คอมไพล์และตรวจสอบฝั่ง Rust workspace (`crates/koharu-scene`, `crates/koharu-pipeline`, `crates/koharu-renderer`, `crates/koharu-app`)
2. สร้าง TypeScript bindings ใหม่ไปยัง `packages/bridge/src/protocol.ts`
3. อัปเดตและตรวจสอบฝั่ง Frontend (`packages/koharu`)

### Rollback Limitations
- เนื่องจาก Working Tree ปัจจุบันมีสถานะ `DIRTY` บนกิ่ง `feature/chapter-workflow-and-detection` การ Rollback หากเกิดปัญหาไม่ควรใช้ `git reset --hard` โดยไม่สำรองการแก้ไขที่ค้างอยู่ก่อน ควรสร้าง Git Stash หรือสำรองเป็น WIP Commit ก่อนเริ่มงาน

### Changes Requiring Additional User Approval
- การเปลี่ยนแปลงใดๆ ที่ต้องแก้ไข `Cargo.toml` เพื่อเพิ่ม Dependency ใหม่
- การแก้ไขใดๆ ที่ต้องแตะต้อง `crates/koharu-llama-sys`, `crates/koharu-diffusion-sys`, หรือ `crates/koharu-torch-sys`

---

## 8. Verification Plan

### Unit, Integration, and End-to-End Checks
1. **Rust Type & Syntax Check:**
   - คำสั่ง: `cargo check`
   - ไดเรกทอรีทำงาน: `c:\Users\T3R0\koharu`
   - สถานะคำสั่ง: **CONFIRMED** (อ้างอิงจาก `GEMINI_CONTEXT.md` Section 13)
2. **Rust Workspace Unit & Integration Tests:**
   - คำสั่ง: `cargo test --workspace`
   - ไดเรกทอรีทำงาน: `c:\Users\T3R0\koharu`
   - สถานะคำสั่ง: **CONFIRMED** (อ้างอิงจาก `GEMINI_CONTEXT.md` Section 13)
3. **Frontend Component & Library Tests:**
   - คำสั่ง: `bun test`
   - ไดเรกทอรีทำงาน: `c:\Users\T3R0\koharu\packages\koharu`
   - สถานะคำสั่ง: **CONFIRMED** (อ้างอิงจาก `GEMINI_CONTEXT.md` Section 13)
4. **Targeted Crate Tests (Proposed / Unconfirmed Flag Syntax):**
   - คำสั่ง: `[PROPOSED_UNCONFIRMED] cargo test -p koharu-scene -p koharu-pipeline`
   - ไดเรกทอรีทำงาน: `c:\Users\T3R0\koharu`

### Negative Cases and Invariant Checks
- ทดสอบส่ง `expectedRevision` ที่ไม่ตรงกับ `Revision` ปัจจุบันของ `Snapshot` เข้าไปยัง `add_glossary_entry` และ `commit_inpaint` เพื่อยืนยันว่าคำสั่งถูกปฏิเสธตามกฎ Optimistic Concurrency Control
- ทดสอบนำเข้า Glossary ที่มี `Language Tag` ไม่ถูกต้องเพื่อยืนยันว่า `apply_glossary_import` คืนค่า Error อย่างปลอดภัยโดยไม่ทำลาย `Snapshot` เดิม

### Manual / UI Checks via CEF Remote Debugging
- รันแอปพลิเคชันในโหมดพัฒนาด้วยคำสั่ง `bun dev` (ใน `c:\Users\T3R0\koharu`) และเชื่อมต่อตรวจสอบ Webview UI ผ่าน Chrome DevTools MCP ที่ `http://127.0.0.1:4000` ตามกฎ `Desktop UI Verification` ใน `GEMINI_CONTEXT.md` Section 12

---

## 9. Completion Evidence

เมื่อ Antigravity ดำเนินการตามแผนงานนี้เสร็จสิ้น ต้องส่งมอบหลักฐานต่อไปนี้:
1. **Expected Changed Artifacts:**
   - รายการไฟล์ทั้งหมดที่ถูกแก้ไขใน `crates/koharu-scene`, `crates/koharu-pipeline`, `crates/koharu-renderer`, `crates/koharu-app`, `packages/bridge/src/protocol.ts`, และ `packages/koharu` พร้อม SHA-256 Checksum ใหม่
2. **Required Verification Results:**
   - ผลลัพธ์การรัน `cargo check`, `cargo test --workspace`, และ `bun test` ที่ผ่านครบทุกเทสต์
   - ผลการตรวจสอบ UI ผ่านพอร์ต `http://127.0.0.1:4000` สำหรับ `GlossaryPanel.tsx` และ `InferenceControl.tsx`
3. **Documentation Updates:**
   - อัปเดตเอกสารภายในหรือ `GEMINI_CONTEXT.md` รอบถัดไปให้สะท้อนสถานะ Working Tree ที่ Clean แล้ว
4. **Remaining Limitations to Report:**
   - รายงานปริมาณการใช้ VRAM สูงสุดที่วัดได้จริงในแต่ละ Stage บนเครื่องทดสอบ และรายการฟีเจอร์ที่แยกไว้ทำในเฟสถัดไป (เช่น Headless CLI Execution และ WebGPU Fallback ไปยัง `crates/koharu-rasterizer`)

---

## 10. Approval Gate

> [!IMPORTANT]
> - **เอกสารฉบับนี้มีสถานะเป็นร่าง (`DRAFT`) เท่านั้น ไม่ใช่การอนุมัติให้รันคำสั่งแก้ไขโค้ดโดยอัตโนมัติ (`implementation_approved: false`)**
> - **Antigravity (`agy`) ต้องทำการตรวจสอบสถานะปัจจุบันของคลังโค้ด (Revalidate Repository State) โดยเฉพาะไฟล์ที่อยู่ในสถานะ `DIRTY` บนกิ่ง `feature/chapter-workflow-and-detection` เทียบกับ Snapshot `koharu-549857f-dirty-20261026` ก่อนเริ่มงาน**
> - **ต้องได้รับการอนุมัติอย่างชัดแจ้งจากผู้ใช้ (Explicit User Approval) และได้รับคำตอบสำหรับ Blocking Questions ก่อนจึงจะสามารถเริ่มดำเนินการแก้ไขโค้ดตาม Task 1–3 ได้**