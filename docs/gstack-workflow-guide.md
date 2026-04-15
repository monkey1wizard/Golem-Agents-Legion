# gstack Workflow Reference

本文現在是 **upstream gstack 的 provider reference**，不是 GAL 使用者的主要操作指南。

如果你在 GAL 內工作，優先使用：

- `/planning`
- `/deep-planning`
- `/plan-to-prompt`
- `/gal whats-next`

只有在 provider routing 判定要走 upstream gstack 時，這份文件中的 skill 名稱才有意義。

## 這份文件的用途

它回答的是：

- upstream gstack 的 planning review lanes 原本如何分工
- 當 GAL 需要把 review lane 映射到 gstack provider 時，應該對應哪個 upstream skill

它不再回答「GAL 使用者下一步應該直接跑什麼指令」。

## Upstream Lane Mapping

| GAL review lane | Upstream gstack skill | 用途 |
| --- | --- | --- |
| Business / Scope review | upstream business review provider | 挑戰 scope、10-star product、商業方向 |
| Design review | upstream design review provider | pre-implementation UX / design audit |
| Engineering review | upstream engineering review provider | architecture、test plan、build readiness |
| Full planning review pipeline | upstream full planning review provider | 串接 CEO → Design → Eng reviews |

## Upstream Discovery Reference

upstream gstack 的 discovery-style 規劃入口有幾個穩定特點：

- 問題導向、question-heavy
- 會用強互動方式挑戰前提
- 把一次 feature 的 design doc 當成流程起點

GAL 不再把這個入口當作自己的 public command surface，但仍可吸收其中少量 discovery 方法到 `/planning`。

## 對 GAL 的實際意義

在 GAL 中，這些 upstream 名稱只應出現在兩個地方：

1. provider routing table
2. provider artifact / review-log contract

它們不應再出現在：

- GAL command catalog
- `/gal status` 或 `/gal whats-next` 的主要 next-step 指引
- README 的主流程教學

## 結論

把這份文件當成 upstream gstack 的語意對照表，而不是 GAL 的主工作流說明。
