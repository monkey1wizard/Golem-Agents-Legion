---
source: docs/collaborative-tools/godot.md
lang: zh-Hant
source_commit: PENDING
translated_at: 2026-06-02
status: current
---

# Godot 模組

Godot C# 工具鏈設定與官方文件索引的一站式參考。

## 工具鏈總覽

| 工具 | MCP / Runtime | 用途 |
| --- | --- | --- |
| Godot CLI | 內建 | Build、import、export、CI 自動化 |
| `godot-mcp` | `puntogris/godot-mcp` | 啟動 editor、執行專案、抓 debug 輸出 |
| `better-godot-mcp` | `kevinwallace/better-godot-mcp` | 不啟動 editor 直接改 scene / resource |
| `godot4-runtime-mcp` | `AarushShintre/godot4-runtime-mcp` | 檢查 live nodes、signals、logs、runtime state |
| VS Code + Godot Extension | — | 編輯 C# gameplay code、SceneTree 瀏覽 |

## 工具怎麼選

按責任分工使用：

- **build、import、export、CI 自動化** → Godot CLI
- **啟動 editor、執行專案、抓 debug 輸出** → `godot-mcp`
- **不啟動 editor 直接改 scene / resource** → `better-godot-mcp`
- **檢查 live nodes、signals、logs、runtime state** → `godot4-runtime-mcp`

## Godot CLI 範例

```bash
godot --headless --path <project> --build-solutions
godot --headless --path <project> --import
godot --headless --path <project> --export-release <preset> <output>
godot --path <project> -e
godot --path <project>
```

## 重要限制：Runtime 版本分離

Godot 遊戲程式碼和 GAL 外部工具的相容性目標不一樣：

- **Godot runtime code** — 應維持在專案實際支援的版本，通常是 `.NET 8 / C# 12`
- **外部工具和 MCP server** — 可以使用較新的 runtime，因為 Godot 不會載入它們

寫 gameplay code 時，以 `../../plugins/gal-core/conventions/csharp.md` 中的 Godot runtime section 為準。

## 偵測 Godot 專案

初始化 `.dev/project.md` 時，以下特徵是 Godot codebase 的強力證據：

- `project.godot` 存在於 repo root 或 app root
- 旁邊或巢狀有 `*.csproj` 用於 C# gameplay code
- `export_presets.cfg`、`.tscn`、`.tres`、`.res`、`addons/` 出現在同一棵專案樹中

## 官方文件索引

以下是 GAL 應首先信任的官方文件來源。

| 主題 | 來源 |
| --- | --- |
| Godot C# 基礎 | [Godot C# Basics](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_basics.html) |
| C# API 差異 | [Godot C# API Differences](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_differences.html) |
| C# 匯出屬性 | [Godot C# Exports](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_exports.html) |
| C# 全域 class | [Godot C# Global Classes](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_global_classes.html) |
| C# 信號 | [Godot C# Signals](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_signals.html) |
| C# Variant | [Godot C# Variant](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_variant.html) |
| C# 集合 | [Godot C# Collections](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_collections.html) |
| C# 風格指南 | [Godot C# Style Guide](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_style_guide.html) |

### 權威順序

1. 官方工具文件
2. 官方 API 或 command-line 參考
3. MCP server 文件（用於理解 wrapper 能力邊界）
4. 社群教學（僅在官方文件缺失時使用）

## 相關文件

- [readme](../README.zh-Hant.md) — GAL 是什麼
- [美術工作流](graphics-workflow.zh-Hant.md) — AI-first 遊戲美術指南
