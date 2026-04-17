# 美術工作流

AI-first 遊戲美術產線的完整指南。涵蓋四條產線路線、MCP 工具堆疊與官方文件索引。

## 核心原則

1. 從資產產線出發，不是從工具出發
2. ComfyUI 是所有產線的預設生成入口
3. 每條產線必須產出具體的輸出包，而非中間物件
4. 3D 產線以 Blender 為中心，除非明確新增其他免費工具

## 四條產線

### 2D 概念與插畫

```text
ComfyUI → GIMP → 最終遊戲素材
```

1. 在 ComfyUI 中生成方向性參考或受控變體
2. 選出偏好的方向
3. 在 GIMP 中清理邊緣、alpha、圖層與色彩平衡
4. 匯出最終遊戲可用圖像

**輸出合約**：最終 PNG 或 WebP、透明或實色背景、變體命名、prompt 與 seed metadata

### Sprite 與像素資產

```text
ComfyUI → Aseprite → 最終 sprite 或 spritesheet
```

1. 在 ComfyUI 中生成概念或輪廓方向
2. 降低視覺複雜度
3. 在 Aseprite 中重建與清理
4. 匯出 sprite、動畫或 spritesheet 包

**輸出合約**：來源 `.aseprite` 或 `.ase`、最終 sprite PNG 或 spritesheet、色板定義（適用時）、frame 尺寸與 sheet 佈局

### UI、Icon 與 HUD

```text
ComfyUI → Figma → Inkscape → 最終匯出
```

1. 在 ComfyUI 中生成視覺方向
2. 在 Figma 中建立元件狀態與佈局
3. 需要 deterministic SVG 清理或匯出時使用 Inkscape
4. 匯出指定目標尺寸用於遊戲整合

**輸出合約**：SVG 與 PNG 匯出、狀態變體（default / hover / selected / disabled / warning）、穩定元件命名、佈局或間距備注

### 3D 資產

```text
ComfyUI → Blender → 預覽渲染與匯出
```

1. 在 ComfyUI 中生成 reference sheet 或材質方向
2. 在 Blender 中建模、快速迭代、場景組裝、baking、預覽渲染與匯出
3. 產出預覽渲染與匯出

**輸出合約**：概念或 reference sheet、來源 scene 檔案、預覽渲染、texture 輸出或 reference package、匯出目標（FBX / GLB / OBJ 或 engine-ready mesh package）

## MCP 工具堆疊

| 產線 | 工具 | MCP / Runtime | 備注 |
| --- | --- | --- | --- |
| 生成 | ComfyUI | `joenorton/comfyui-mcp-server` | 預設本地生成伺服器 |
| 2D 清理 | GIMP | `maorcc/gimp-mcp` | GIMP 3.0 API bridge |
| 像素資產 | Aseprite | `willibrandon/pixel-mcp` | Sprite、動畫與 spritesheet 工作流 |
| UI 佈局 | Figma | `grab/cursor-talk-to-figma-mcp` | UI 與元件工作流 |
| 向量清理 | Inkscape | `grumpydevorg/inkscape-mcps` | CLI + DOM 操作 |
| 3D 一般 | Blender | `ahujasid/blender-mcp` | 預設 3D 工作流 |

### 決策樹

```text
需要生成資產方向或受控變體？ → ComfyUI
需要光柵清理或合成？         → GIMP MCP
需要像素生產或 spritesheet？  → pixel-mcp
需要 UI 佈局或 HUD 合成？    → Figma MCP
需要 deterministic SVG 清理？ → Inkscape MCP
需要一般 3D 建模和預覽渲染？  → Blender MCP
```

## 跨工具交接規則

- 交接的是檔案，不是抽象意圖。每條產線應留下具體的 artifact 給下一個工具
- 當第二個工具會繼續編輯時，偏好 lossless 中間輸出
- 保持命名 deterministic，讓後續匯出或打包步驟能識別所選資產
- 在可行時保存風格參考、seeds、prompt 變體和核可方向備注

## 官方文件索引

| 主題 | 來源 |
| --- | --- |
| Blender 命令列 | [Blender Manual - Command Line Arguments](https://docs.blender.org/manual/en/latest/advanced/command_line/arguments.html) |
| Blender Python API | [Blender Python API](https://docs.blender.org/api/current/) |
| GIMP 腳本與外掛 | [GIMP Documentation](https://docs.gimp.org/) |
| Inkscape CLI | [Inkscape Man Page](https://inkscape.org/doc/inkscape-man.html) |
| Figma 開發者文件 | [Figma Developers](https://www.figma.com/developers) |
| Aseprite 文件 | [Aseprite Docs](https://www.aseprite.org/docs/) |
| ComfyUI 專案 | [ComfyUI](https://github.com/comfyanonymous/ComfyUI) |

### 權威順序

1. 官方工具文件
2. 官方 API 或 command-line 參考
3. MCP server 文件（理解 wrapper 能力邊界）
4. 社群教學（僅在官方文件缺失時使用）

### ComfyUI 知識邊界

ComfyUI 是 workflow 引擎，不是單一藝術方法論。依賴 ComfyUI 用於 workflow orchestration、parameter exposure、batch variation、image refinement、seed 與 style 一致性。不要把 ComfyUI 輸出預設為 production-ready——它們是下游產線工具的輸入。

## 延後的工具

- **Krita** — 延後，因為 ComfyUI + GIMP 已涵蓋初期 2D 需求
- **Pixelle** — 延後，除非雲端模式或無 GPU 操作成為必要
