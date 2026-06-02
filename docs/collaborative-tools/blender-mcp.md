# Blender MCP

這份文件整理了本 repo 目前 `blender-mcp` 的安裝檢查結果、正確設定方式，以及在 GAL 工作流內的建議使用順序。

## 檢查結論

- `uvx` 已安裝，且可從系統 `PATH` 找到
- `blender-mcp` 套件可透過 `uvx` 正常解析與載入
- Blender addon 目前已在 `localhost:9876` 正常監聽
- 透過 `blender-mcp` 對 Blender 發送 `get_scene_info` 已成功
- 目前 `~/.gal/config/mcp.local.json` 已可用 `env` 顯式設定 `BLENDER_HOST` 與 `BLENDER_PORT`

本次檢查代表兩件事都成立：

- `blender-mcp` 套件可啟動
- Blender 端 addon 已啟用並可接受 MCP 指令

## 本次實測

我實際驗證了以下幾件事：

```powershell
Get-Command uvx
uvx --from blender-mcp python -c "import blender_mcp; print(blender_mcp.__version__)"
Test-NetConnection -ComputerName localhost -Port 9876
uvx --from blender-mcp python -c "from blender_mcp.server import get_blender_connection; import json; result = get_blender_connection().send_command('get_scene_info'); print(json.dumps(result, ensure_ascii=False))"
```

結果：

- `uvx` 可用
- `blender_mcp` 可成功 import
- 匯入版本為 `0.1.0`
- `localhost:9876` TCP 連線成功
- `get_scene_info` 成功回傳目前場景資訊

這表示你目前不是安裝問題，也不是 Blender addon 沒開。
目前真正需要注意的是：如果 Blender MCP 與 FreeCAD MCP 同時使用，必須明確規劃 port，避免預設值衝突。

## 正確的 MCP 設定

依照 [ahujasid/blender-mcp](https://github.com/ahujasid/blender-mcp) README，VS Code MCP 整合的核心設定應是：

```json
{
  "servers": {
    "blender": {
      "type": "stdio",
      "command": "uvx",
      "args": ["blender-mcp"]
    }
  }
}
```

如果要在 `~/.gal/config/mcp.local.json` 內直接指定 Blender 連線埠，建議寫成：

```json
{
  "servers": {
    "blender": {
      "type": "stdio",
      "command": "uvx",
      "args": ["blender-mcp"],
      "env": {
        "BLENDER_HOST": "localhost",
        "BLENDER_PORT": "9876"
      }
    }
  }
}
```

你原本的設定是：

```json
{
  "command": "uvx",
  "args": ["/c", "uvx", "blender-mcp"]
}
```

這種寫法混用了 `cmd /c` 包裝方式，但 `command` 卻不是 `cmd`，因此啟動時參數會錯位。

## 為什麼原本設定不對

upstream README 提供了兩種常見 Windows 風格：

- VS Code MCP: 直接使用 `uvx blender-mcp`
- Cursor Windows: 使用 `cmd /c uvx blender-mcp`

你的設定是把這兩種寫法混在一起：

- `command` 用了 `uvx`
- `args` 卻放了 `"/c", "uvx", "blender-mcp"`

實際效果會變成類似：

```powershell
uvx /c uvx blender-mcp
```

這不是 upstream 定義的啟動方式。

## MCP 設定與 Port 避免衝突

If you want the verified English startup and execution checklist for FreeCAD, see [collaborative-tools/freecad.md](collaborative-tools/freecad.md).

當 Blender MCP 與 FreeCAD Robust MCP 同時使用時，最容易踩到的問題是預設 port 撞號。

目前這兩套工具的常見預設值是：

- Blender MCP socket 預設 `9876`
- FreeCAD Robust MCP XML-RPC 預設 `9875`
- FreeCAD Robust MCP socket 預設 `9876`

也就是說：

- Blender 預設 port `9876`
- FreeCAD 的 socket 預設也會用 `9876`

如果兩者同時啟動，而且都用預設值，就會衝突。

### 建議配置

如果你同時使用 Blender 與 FreeCAD，建議固定分配：

- Blender: `9876`
- FreeCAD XML-RPC: `9873`
- FreeCAD socket: `9874`

這樣 Blender 與 FreeCAD 不會互搶 socket port。

### `~/.gal/config/mcp.local.json` 範例

以下是可同時支援 Blender 與 FreeCAD 的本地 MCP 設定示例：

```json
{
  "servers": {
    "blender": {
      "type": "stdio",
      "command": "uvx",
      "args": ["blender-mcp"],
      "env": {
        "BLENDER_HOST": "localhost",
        "BLENDER_PORT": "9876"
      }
    },
    "freecad": {
      "command": "freecad-mcp",
      "env": {
        "FREECAD_MODE": "xmlrpc",
        "FREECAD_SOCKET_HOST": "localhost",
        "FREECAD_XMLRPC_PORT": "9873",
        "FREECAD_SOCKET_PORT": "9874"
      }
    }
  }
}
```

### 同步原則

要讓設定真的生效，port 必須在兩邊一致：

1. Blender 內 `BlenderMCP` 面板顯示的 port
2. `~/.gal/config/mcp.local.json` 的 `BLENDER_PORT`

如果你改了 Blender addon 內的 port，例如改成 `9872`，那 `~/.gal/config/mcp.local.json` 也必須同步改成：

```json
"env": {
  "BLENDER_HOST": "localhost",
  "BLENDER_PORT": "9872"
}
```

FreeCAD 也是同樣原則：

1. FreeCAD bridge 實際監聽的 XML-RPC / socket port
2. `~/.gal/config/mcp.local.json` 中 `FREECAD_XMLRPC_PORT` / `FREECAD_SOCKET_PORT`

這兩邊必須一致，MCP client 才會連到正確的服務。

## 完整可用條件

`blender-mcp` 要真正可用，需要同時滿足兩個條件：

1. MCP server 端可啟動
2. Blender addon 端已載入並開始監聽 socket

依照 upstream 架構：

- `addon.py` 會在 Blender 裡建立 socket server
- `blender-mcp` MCP server 會透過預設 `localhost:9876` 連回 Blender

如果只有 `uvx blender-mcp` 可用，但 Blender 端 addon 沒啟動，工具仍然無法真正操作場景。

## Blender 端安裝步驟

依照 upstream README：

1. 從 `ahujasid/blender-mcp` 下載 `addon.py`
2. 開啟 Blender
3. 到 `Edit > Preferences > Add-ons`
4. 按 `Install...` 並選取 `addon.py`
5. 啟用 `Interface: Blender MCP`

完成後，在 Blender 3D 視窗側欄按 `N`，應能看到 `BlenderMCP` 分頁。

## 啟動順序

建議順序如下：

1. 先確認 VS Code 的 MCP 設定已套用
2. 開啟 Blender 並載入 addon
3. 在 Blender 側欄的 `BlenderMCP` 面板按下 `Connect to MCP server`
4. 再回到支援 MCP 的客戶端確認工具是否出現

上游文件也特別提醒：

- 如果有連線問題，先確認 Blender addon server 已啟動
- 不要手動在另一個終端反覆亂跑 `uvx blender-mcp` 當成日常使用方式
- 第一次指令偶爾可能失敗，重試一次即可

## 建議驗證清單

完成設定後，可依序檢查：

1. `uvx` 是否存在
2. `blender-mcp` 是否可 import
3. Blender addon 是否已安裝
4. Blender 側欄是否出現 `BlenderMCP`
5. 是否已按下 `Connect to MCP server`
6. MCP 客戶端是否看到 Blender 工具
7. 執行 `get_scene_info` 是否有回應

## 在 GAL 內的使用建議

如果你是把 Blender 當成 3D 資產工作流的一部分，建議順序是：

```text
ComfyUI → Blender → 預覽渲染 / 匯出
```

其中 Blender MCP 適合做的事包括：

- 讀取目前場景資訊
- 建立與修改基本幾何
- 調整材質與燈光
- 取得 viewport 截圖
- 執行受控的 Blender Python 操作

## 常見故障排除

### 看得到套件，但工具不能用

通常是 Blender addon 沒有啟動，或 Blender 端沒有在監聽預設 port `9876`。

### MCP 設定已經寫了，還是沒有工具

先確認你的 MCP client 有重新載入設定。若這個 repo 是透過安裝腳本同步到工具設定，請重新執行對應的 setup 流程。

### Blender 已開，但仍連不上

優先檢查：

- addon 是否真的啟用
- BlenderMCP 面板是否顯示正在執行
- 是否有其他程式占用同一個 port
- MCP client 是否只啟動了一個 Blender MCP 實例

## 摘要

目前狀態可以下這個結論：

- `blender-mcp` 套件有正確安裝
- Blender addon 已啟動，且 socket 連線正常
- 端到端 `get_scene_info` 驗證已成功
- `~/.gal/config/mcp.local.json` 可以直接設定 `BLENDER_PORT`
- 若 Blender 與 FreeCAD 同時使用，必須避開兩者預設 `9876` 的衝突
- 若 VS Code 端還看不到工具，最可能原因是 MCP client 尚未重新載入更新後的設定
