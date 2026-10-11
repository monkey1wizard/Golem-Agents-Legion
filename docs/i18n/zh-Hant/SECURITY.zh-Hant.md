---
source: SECURITY.md
lang: zh-Hant
source_commit: 8e7f937227df0bb76b5d62fc1d390a92e5c826c9
translated_at: 2026-09-21
type: Policy
title: 安全性政策
description: 說明 GAL 支援的版本、私密通報漏洞的流程，以及產品的安全性模型與信任邊界。
tags:
  - security
  - disclosure
  - trust-boundaries
status: stable
---

# 安全性政策 (Security Policy)

## 支援版本

只有最新 release 版本會收到安全性修補。較舊版本不另外維護安全性修補分支。

## 私密漏洞回報

請一律使用 GitHub Private Security Advisories 私密通報漏洞，切勿建立公開 issue。專案未另設安全性回報專用的電子郵件。

維護者會盡力協調漏洞揭露與修補時程，但不承諾固定天數的服務等級協定 (SLA)。收到通報後，維護者會在 advisory 內確認影響範圍、修補方式與適當的公開時間。

## Security Model

### Assets

GAL 保護的主要資產包含 `gal init` 產生的 `AGENTS.md`、投影後的 skills、commands 與 golem 代理程式介面、`~/.gal/config/config.json` 的本機設定、`~/.gal/local/` 的使用者內容，以及儲存庫自有的 `.dev/` 狀態。

### Trust Roles

本機 operator 是受信任角色。Release supply chain 由 SHA-256 `checksums.txt` 與 keyless cosign 驗證。受派送的 executor、executor write-back、匯入的計畫與 prompt，皆視為不受信任的輸入。Host MCP 設定屬於使用者所有，`gal` 不會擅自寫入。Codex 核准與沙盒設定由使用者管理，GAL 不會修改。GAL 以 `--dangerously-bypass-approvals-and-sandbox` 啟動受派送的 Codex 子行程，因此子行程與其他執行器一樣在 Codex 沙盒外執行。原因是 Codex 的 Windows 沙盒目前會在執行任何指令之前就失敗，詳見 [setup.zh-Hant.md](./setup.zh-Hant.md#已知問題-codex-windows-沙盒失效)。操作遭拒絕絕不代表已授權，未觀察到的政策狀態維持 Unknown。

### Trust Boundaries

- 受追蹤合約與產生的轉接器 (generated adapters) 之間以 GAL ownership marker 為界。若既有檔案未帶有標記，套用流程即告中止。
- GAL 自有狀態與外部狀態以 `_galProjection` 命名空間劃分邊界。`plugins.lock.json` 的其餘頂層欄位不由 GAL 管理。
- 控制節點 (Control node) 與 executor 之間以完整內嵌的 contract bytes 為界。Executor 不需要讀取僅存在於控制節點的路徑。
- `~/.gal/config/config.json` 是本機邊界，其中的 `executorRouting`、`sshTarget` 與 `remoteWorkdir` 不得分享或簽入儲存庫。

### Existing Invariants

轉接器算繪會驗證必要章節並採 fail-closed（預設阻擋）。寫入與清理由 ownership marker 保護，只有具備明確 GAL 所有權證據的路徑才可移除。計畫範圍的清理 (Plan-scoped cleanup) 必須全數成功，否則會留下失敗收據 (fail receipt)。共用的 `secret_re` 同時支援 git filters 與 `gal doctor` 的機密資訊掃描。安裝程式 (Installer) 若在 SHA-256 或 cosign 驗證失敗時會立即中止。Codex 的設定與復原程序請參閱[安裝與初始化](./setup.zh-Hant.md#codex-管道執行設定)。詳細規範請參閱 [architecture.zh-Hant.md](./architecture.zh-Hant.md)、[projection.zh-Hant.md](./projection.zh-Hant.md)、[configuration.zh-Hant.md](./configuration.zh-Hant.md) 與 [workflows.zh-Hant.md](./workflows.zh-Hant.md)。

### Attack Surface

主要攻擊面包含 executor input 與 write-back、匯入的計畫與 prompt、`config.json` 的 SSH 通道 (SSH lane)、MCP host 載入時解析的 `${ENV_VAR}`，以及 release 下載路徑。`ready-to-finalize` 僅代表合作式協定證明 (protocol evidence)，並非用來防禦擁有相同寫入權限攻擊者的密碼學來源證明。

### Local Review Tooling

本機安全性審查工具可提早回報常見弱點，但並非 GAL 的強制相依項目或合併關卡 (merge gate)。審查結果若涉及本文件定義的信任邊界，應以該邊界所對應的不變式規則為準。寫死的機密由 `secret_re` 關卡負責把關。轉接器所有權標記 (Adapter ownership markers)、executor write-back、`config.json` 的 SSH 通道，以及匯入的計畫與 prompt，皆屬必須另行檢查的 GAL 專屬風險。正式釋出版本若發現漏洞，一律經由上方的私密回報管道通報。
