# 計畫: 升級 PDF Skill 至 Chandra OCR

## Goal (目標)

將 GAL `pdf` skill 升級，以先進的視覺語言模型 `datalab-to/chandra` (Chandra OCR) 取代舊有的 `pdfplumber` 與 `pytesseract`，提供高品質的文字與 OCR 擷取能力；同時保留 `pypdf` 與 `qpdf` 負責 PDF 檔案的常規操作（如合併、分割、旋轉等）。

## Requirements (需求)

- [ ] `pdf` skill 文件 (`SKILL.md`) 必須指示 Agent 使用 Chandra OCR 來執行文字、表格與掃描檔擷取，而非使用 `pdfplumber` 和 `pytesseract`。
- [ ] `pdf` skill 文件必須保留 `pypdf`、`reportlab` 以及 `qpdf` 用於 PDF 檔案操作的指令。
- [ ] 提供一個便利的封裝腳本 (`scripts/chandra_extract.py`)，讓 Agent 能夠透過簡單的 CLI 介面呼叫 Chandra OCR，而不需要自行撰寫繁瑣的推論程式碼。
- [ ] `SKILL.md` 中的 Quick Reference 區塊必須更新，明確標示 Chandra 為擷取任務的最佳工具。

## Approach (實作方法)

### 步驟 1: 建立 Chandra OCR 便利腳本

- **Files**: `skills/pdf/scripts/chandra_extract.py`
- **What**: 撰寫一支 Python 腳本，接收 PDF 檔案路徑與輸出路徑作為參數，載入 `chandra-ocr` 模型（使用 Hugging Face 或 vLLM 後端），並將文件萃取為結構化的 Markdown 檔案。
- **Verify**: 腳本能正確解析參數，且包含 `chandra-ocr` 官方文件所記載的基本推論邏輯。

### 步驟 2: 更新 PDF Skill 文件

- **Files**: `skills/pdf/SKILL.md`
- **What**:
  - 移除所有關於 `pdfplumber` 與 `pytesseract` 的內容。
  - 新增 Chandra OCR 專屬段落，並說明如何使用 `chandra_extract.py` 腳本。
  - 保留 `pypdf` 與 `reportlab` 等段落。
  - 更新底部的 Quick Reference 表格。
- **Verify**: 更新後的 `SKILL.md` 閱讀順暢，且能根據任務性質（擷取 vs 操作）正確引導 Agent 使用對應工具。

## Files to Create or Modify (異動檔案)

- `skills/pdf/SKILL.md` — [MODIFY] 更新指引以使用 Chandra OCR 進行擷取，並指向新的便利腳本。
- `skills/pdf/scripts/chandra_extract.py` — [NEW] 提供給 `chandra-ocr` InferenceManager 的 CLI 封裝工具。

## Test Cases (測試案例)

- [ ] 執行 `python skills/pdf/scripts/chandra_extract.py --help` → 驗證腳本可正常執行並顯示參數說明。
- [ ] 模擬 Agent 閱讀 `SKILL.md` → 驗證當 Agent 被要求從 PDF 擷取表格時，會正確選擇 `chandra_extract.py` 而不是尋找 `pdfplumber`。

## Success Criteria (成功準則)

- [ ] `pdfplumber` 與 `pytesseract` 已完全從 `pdf` skill 的建議工具集中移除。
- [ ] Chandra OCR 成為處理文字、表格與掃描檔擷取的唯一建議工具。
- [ ] 檔案操作任務依然正確指向 `pypdf` 或 `qpdf`。

## Risks (風險)

- Chandra OCR 是一個深度學習模型。使用者的本機環境必須安裝 `chandra-ocr`、`torch` 與 `transformers`。若缺乏 GPU 加速，執行速度可能會較慢。我們將透過在 Skill 文件中清楚標示環境需求來緩解此問題，但執行速度本身不在我們的控制範圍內。

## Open Questions (待確認問題)

無。

## Approval (核准狀態)

- Human approval: [pending]
- Architect review: [not requested]
- Additional domain review: [not requested]

## Review Results (審查結果)

### Architecture Review

Pending.

### Business Review

Pending.

### Design Review

Pending.

### Engineering Review

**CLEAR**. 變更範圍定義明確。本計畫合理地將文件擷取（交由新的 Chandra VLM 處理）與文件操作（交由現有的 pypdf 處理）分開。建立封裝腳本可避免 Agent 反覆生成過長且複雜的 VLM 推論程式碼。所有任務與測試皆合理可行。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan (測試計畫)

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | manual | 執行 `python skills/pdf/scripts/chandra_extract.py --help` 驗證相依性與語法 | T-001 |
| TP-002 | manual | 審查 `SKILL.md` 確保舊工具已移除且 `pypdf` 已被保留 | T-002 |

## Tasks (執行工作項目)

- [ ] T-001 — 建立 `skills/pdf/scripts/chandra_extract.py` 做為 `chandra-ocr` 的封裝腳本
- [ ] T-002 — 更新 `skills/pdf/SKILL.md`，移除 `pdfplumber`/`pytesseract`，加入 Chandra，並更新 Quick Reference 表格
