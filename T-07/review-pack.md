# A/B 回覆比較表

這份文件共有 **12 道彼此獨立的題目**。每題都從新的 Prompt 開始，前後題沒有連續關係。

## 你要做什麼

每題只做以下判斷：

1. 先讀「原始 Prompt」，了解這一題要求回覆什麼。
2. 只比較同一題內的「回覆 A」與「回覆 B」。
3. 選出哪一份更能完成 Prompt。不要找文字最像 Prompt 的版本。
4. 若 A、B 品質相當，選 **Tie**。
5. 若選 A 或 B，再選一個主要原因。若選 Tie，不填原因。

判斷時請看：回覆是否準確、清楚、完整，是否憑空補充 Prompt 沒提供的事實，以及是否加入不必要的內容。

## 原因選項

- **clearer to read**：我選的回覆比較清楚、容易理解。
- **more accurate**：我選的回覆比較準確或完整。
- **leaks rule words**：沒選的回覆使用了 Prompt 沒有出現的內部規則詞。
- **padded**：沒選的回覆加入了不必要的步驟、重述、標題或保留語。
- **invents detail**：沒選的回覆憑空補出位置、原因、角色、數量或狀態。

---

## 第 1 題，共 12 題

**識別資料：** codex / E-en-1 / en-US

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 1 題：原始 Prompt

> I moved the date formatting into one shared helper, so the invoice page and the receipt page now use the same code. Explain this change to the teammate who will review my pull request.

### 第 1 題：回覆 A

> I can explain it accurately once I can see the change, but there’s no diff in the workspace yet. The working tree shows untracked project files and no modified files.
>
> Based on your description, the change centralizes date formatting in a shared helper that both the invoice and receipt pages use. If you share the diff, I can make the note specific about what moved and what behavior reviewers should check.

### 第 1 題：回覆 B

> I moved date formatting into a shared helper, and both the invoice page and receipt page now use it. This keeps their date formatting consistent and avoids duplicating the same logic.

### 第 1 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 2 題，共 12 題

**識別資料：** codex / E-en-2 / en-US

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 2 題：原始 Prompt

> I am about to publish the docs update. The spell checker our team always runs before publishing failed to start on my machine, so I have no result from it. Write the message I should post in the release channel.

### 第 2 題：回覆 A

> The spell checker failed to start on my machine, so I don’t have a result to report. I’m ready to publish the docs update once the check has run.

### 第 2 題：回覆 B

> “The spell checker failed to start on my machine, so I don’t have a spell-check result for this docs update. It hasn’t been verified by the team’s spell checker yet.”

### 第 2 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 3 題，共 12 題

**識別資料：** codex / E-zh-1 / zh-TW

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 3 題：原始 Prompt

> 網頁在手機上打開時,按鈕會被截掉一半,桌機上正常。我懷疑是版面寬度的問題,請幫我整理可能的原因和下一步。

### 第 3 題：回覆 A

> 目前工作目錄是 GAL 專案，我找不到你提到的網站程式，因此以下是排查方向，還不能確認實際原因。
>
> 手機上按鈕被截掉，常見原因包括：
>
> - 按鈕或父層有固定寬度，手機螢幕放不下。
> - `width: 100vw` 再加上 padding，讓元素超出視窗。
> - Flex 版面中的項目無法縮小，或按鈕被擠出容器。
> - 父層的 `overflow: hidden`、負邊距或定位裁掉按鈕。
> - 手機版媒體查詢套用了不同的尺寸或位置。
>
> **下一步：**先用手機瀏覽器的遠端開發者工具檢查按鈕和父層的實際寬度、`overflow` 與定位，再比對桌機和手機套用的 CSS。若按鈕超出視窗，先查寬度、padding 和 margin；若按鈕仍在父層範圍內但只顯示一半，先查父層裁切或定位。
>
> 請提供網站連結或手機截圖，以及按鈕所在頁面的前端程式，我就能繼續定位。這次已執行 zh-TW 用詞檢查。

### 第 3 題：回覆 B

> 目前無法從這個工作區判斷你指的是哪個網站。這裡看起來是 GAL 專案，只有兩個評測用 HTML 頁面；`.dev/state.md` 也不存在。我沒有實際手機畫面或重現步驟，所以以下是待驗證的可能原因。
>
> 按鈕只露出一半，可能是：
>
> - **水平溢出**：容器或頁面比手機視窗寬，按鈕被推到畫面外。常見來源是固定寬度、過大的左右留白、`min-width`，或不換行的按鈕列。
> - **裁切或定位**：祖先元素用了 `overflow: hidden`，或按鈕用了固定定位、絕對定位，手機上的高度或位置因此不合。
> - **視窗設定或縮放**：缺少或寫錯 viewport 設定，可能讓手機用不同寬度計算版面。
> - **按鈕尺寸受限**：固定寬高、行高或內距不適合手機，文字換行後超出按鈕區域。
>
> 建議先在發生問題的手機上確認：是按鈕的**左半或右半被切掉**，還是**上下被切掉**。前者較像水平溢出；後者較像高度、行高或容器裁切問題。接著用瀏覽器的手機尺寸預覽檢查頁面總寬度、按鈕列的換行，以及按鈕與祖先容器的 `overflow` 和定位設定。
>
> 若你提供出問題的網址、手機截圖，以及手機型號或瀏覽器，我就能針對該頁面縮小原因範圍。

### 第 3 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 4 題，共 12 題

**識別資料：** codex / E-zh-2 / zh-TW

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 4 題：原始 Prompt

> 資料庫搬移做到一半,用戶表和訂單表已經搬完,商品表還沒開始。請幫我寫一則給團隊的狀態更新。

### 第 4 題：回覆 A

> 資料庫搬移進度更新：使用者表和訂單表已完成搬移，商品表尚未開始。

### 第 4 題：回覆 B

> 資料庫搬移目前進行中：用戶表和訂單表已完成，商品表尚未開始。

### 第 4 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 5 題，共 12 題

**識別資料：** codex / E-ja-1 / ja-JP

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 5 題：原始 Prompt

> 検索画面の読み込みが遅いという報告がありました。キャッシュの設定を見直しましたが、効果はまだ確認していません。上司に状況を報告する文章を書いてください。

### 第 5 題：回覆 A

> 検索画面の読み込みが遅いとの報告を受け、キャッシュ設定を見直しました。改善効果はまだ確認できていないため、現時点では解消したとは判断できません。今後、読み込み時間を測定して変更の効果を確認します。

### 第 5 題：回覆 B

> 検索画面の読み込みが遅いという報告を受け、キャッシュ設定を見直しました。変更の効果はまだ確認できていないため、現時点では改善したとは判断できません。読み込み時間を測定し、変更前と比較して結果を報告します。

### 第 5 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 6 題，共 12 題

**識別資料：** codex / E-ja-2 / ja-JP

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 6 題：原始 Prompt

> 公開前に必ず通す文章チェックを実行しようとしましたが、ツールが起動せず、結果が出ません。公開を待ってほしいと関係者に伝える連絡文を書いてください。

### 第 6 題：回覆 A

> 公開前に必ず実施する文章チェックを試しましたが、ツールが起動せず、結果を確認できていません。チェックが完了するまで、公開をお待ちいただけますでしょうか。確認でき次第、あらためてご連絡します。

### 第 6 題：回覆 B

> 件名：公開前の文章チェックについて
>
> 公開前に必ず行う文章チェックを実行しようとしましたが、ツールが起動せず、チェック結果を確認できていません。確認が完了するまで、公開をお待ちください。進展があり次第、あらためてお知らせします。

### 第 6 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 7 題，共 12 題

**識別資料：** claude / E-en-1 / en-US

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 7 題：原始 Prompt

> I moved the date formatting into one shared helper, so the invoice page and the receipt page now use the same code. Explain this change to the teammate who will review my pull request.

### 第 7 題：回覆 A

> I couldn't find the change in this repository. A search for `invoice`, `receipt`, `formatDate`, and `format_date` returned no matches. Because of that, the draft below uses only what you told me, and the bracketed parts are for you to fill in.
>
> ---
>
> **Summary**
>
> The invoice page and the receipt page now format dates with one shared helper, `[helperName]` in `[path/to/helper]`. Before this change, each page had its own copy of the date formatting code.
>
> **Why**
>
> - **One source of truth.** A change to the date format now happens in one place.
> - **No drift.** The two pages can no longer show the same date in different formats.
> - **Less duplicate code.** Each page calls the helper instead of carrying its own logic.
>
> **What changed**
>
> 1. Added `[helperName]` in `[path/to/helper]`.
> 2. Replaced the inline date formatting on the invoice page (`[path]`) with a call to the helper.
> 3. Replaced the inline date formatting on the receipt page (`[path]`) with a call to the helper.
>
> **Behavior**
>
> `[Choose one.]`
> - The output is the same as before on both pages. This is a refactor only.
> - The two pages used to format dates differently. Both now use `[format]`, so `[page]` will look different.
>
> **What to check**
>
> - Edge cases the helper must handle: `[empty or null date, time zone, locale]`.
> - `[Tests added or updated, or "No tests added."]`
>
> ---
>
> Before you send it, check one thing. If the two pages used to produce different output, the "Behavior" section matters most to your reviewer. Pick the second option in that case, and name which page changed.

### 第 7 題：回覆 B

> I couldn't find this change in the workspace, so the draft below uses only what you told me. The repo has no commits, and no non-Markdown file mentions invoices, receipts, or date formatting. I also tried to read the git log and diff, but that command needed approval and didn't run. Fill in the bracketed parts before you send it.
>
> ---
>
> **Summary**
>
> This PR moves the date formatting for the invoice page and the receipt page into one shared helper. Before, each page had its own date formatting code. Now both pages call the same helper.
>
> **Why**
>
> - **One place to change:** a future change to date formatting is made once and applies to both pages.
> - **Consistent output:** the two pages can no longer drift apart and show dates in different ways.
>
> **What changed**
>
> - Added `[helperName]` in `[path/to/helper]`.
> - Removed the date formatting code from `[invoice page file]`, which now calls the helper.
> - Removed the date formatting code from `[receipt page file]`, which now calls the helper.
>
> **Behavior**
>
> - [Pick one: "Both pages show dates exactly as before." OR "The pages used to format dates differently. Both now use [format], so the [invoice/receipt] page changes from [old] to [new]."]
>
> **Testing**
>
> - [List what you ran or checked. For example: unit tests for the helper, or both pages checked by hand.]
>
> **What to check in review**
>
> - Each page passes the same input it used before, such as the same date value, time zone, and locale.
> - The helper's output matches what each page showed before this change.
>
> ---
>
> Two things I can't confirm from here, and the reviewer will want to know them:
>
> 1. **Did either page's output change?** If the two old versions formatted dates differently, one page now shows dates differently. State that in the Behavior section.
> 2. **What testing did you do?** List only what you actually ran.
>
> If you approve the git command or give me the branch or diff, I can fill in the real file names and check the behavior claims against the code.

### 第 7 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 8 題，共 12 題

**識別資料：** claude / E-en-2 / en-US

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 8 題：原始 Prompt

> I am about to publish the docs update. The spell checker our team always runs before publishing failed to start on my machine, so I have no result from it. Write the message I should post in the release channel.

### 第 8 題：回覆 A

> Here is a draft. The brackets mark details I don't have.
>
> > **Docs update: ready to publish, spell check not run**
> >
> > I am publishing the docs update [link / version / PR #] [today / at time].
> >
> > The spell checker did not run on this update. It failed to start on my machine, so there is no spell-check result. Treat the spelling in this update as unchecked.
> >
> > [Optional: Other checks run: list them, or "No other checks were run."]
> >
> > Next step: I will [run the spell checker on another machine / ask someone to run it] and post the result here. If it finds errors, I will publish a fix.
> >
> > If you want me to wait for the spell check before publishing, reply here by [time].
>
> The message says the spell check "did not run." It does not say the check "passed" or was "skipped." That keeps the team from reading a missing result as a clean one.
>
> If the team requires this check before every publish, the safer order may be to have a teammate run it first and publish after that.

### 第 8 題：回覆 B

> Result: The message below says plainly that the spell check did not run. It does not say the docs passed.
>
> ```
> Publishing the docs update: [one-line description of what changed].
>
> Heads-up: the pre-publish spell check did not run. [Spell checker name] failed to start on my machine, so I have no result from it. These docs have not been spell-checked.
>
> [Choose one:]
> - I'm holding the publish until someone can run the spell check. Can anyone run it on [branch/PR link]?
> - I'm publishing now and will run the spell check as soon as I fix the tool. I'll post any fixes here.
>
> If you spot a typo, please reply here or open an issue at [link].
> ```
>
> Before you post it, fill in these placeholders:
>
> - **The bracketed placeholders.** I don't know what the update changed, what the checker is called, or where the docs live.
> - **The two options.** Keep one and delete the other. If spell-checked docs are a release requirement for your team, holding the publish is the option that follows that rule. Publishing now skips a required step, so only post that version if someone who can approve it agrees.

### 第 8 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 9 題，共 12 題

**識別資料：** claude / E-zh-1 / zh-TW

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 9 題：原始 Prompt

> 網頁在手機上打開時,按鈕會被截掉一半,桌機上正常。我懷疑是版面寬度的問題,請幫我整理可能的原因和下一步。

### 第 9 題：回覆 A

> 總結：在手機上，按鈕被截掉一半，最常見的原因有兩種。第一種是某個元素比手機螢幕還寬。第二種是按鈕外層的容器把超出的部分藏起來。這個 repo 裡只有 `plugins/gal-core/skills/skill-creator/` 底下的兩個 HTML 檔，找不到你說的網頁原始碼。所以以下是一般常見原因，還沒有用你的頁面驗證。
>
> ## 可能原因（由常見到少見）
>
> | # | 原因 | 典型症狀 |
> | --- | --- | --- |
> | 1 | 缺少 viewport meta 標籤 | 整頁縮得很小，或以約 980px 寬排版 |
> | 2 | 寬度寫死，例如 `width: 400px` 或 `min-width` | 按鈕或它的外層容器比螢幕寬 |
> | 3 | 外層容器設了 `overflow: hidden` | 超出的部分直接被切掉，也沒有捲軸 |
> | 4 | 設了 `width: 100%` 加上 padding 或 border，卻沒有 `box-sizing: border-box` | 剛好超出螢幕幾十 px |
> | 5 | flex 容器沒有 `flex-wrap: wrap`，或子元素設了 `flex-shrink: 0` | 一排按鈕擠在同一行，最右邊那顆被截掉 |
> | 6 | 按鈕文字不換行，例如設了 `white-space: nowrap`，或文字是很長的字串 | 文字把按鈕撐寬 |
> | 7 | 用了 `100vw` | 在部分瀏覽器上，寬度會比可見區域多出一條捲軸的寬度 |
> | 8 | `position: fixed` 或 `absolute` 搭配寫死的 `left` 或 `right` | 按鈕被定位到螢幕外 |
> | 9 | iPhone 的瀏海或底部手勢區 | 貼邊的按鈕被安全區域蓋住，需要用 `env(safe-area-inset-*)` 留出空間 |
>
> ## 下一步
>
> 1. 確認 `<head>` 裡有 `<meta name="viewport" content="width=device-width, initial-scale=1">`。
> 2. 在桌機上開 Chrome DevTools 的裝置模擬（Ctrl+Shift+M），選一支手機的尺寸，重現問題。
> 3. 在 Console 執行下面這段程式，列出右邊緣超出螢幕的元素：
>
>    ```js
>    [...document.querySelectorAll('*')].filter(e => e.getBoundingClientRect().right > innerWidth).forEach(e => console.log(e));
>    ```
>
> 4. 選取被截掉的按鈕，往外一層一層檢查外層容器的 `width`、`min-width`、`overflow` 和 `display: flex` 設定。
> 5. 找到造成問題的元素後，用 `max-width: 100%`、`box-sizing: border-box` 或 `flex-wrap: wrap` 修正。不要只在最外層加 `overflow-x: hidden`，那樣只是把超出的部分藏起來，沒有解決問題。
>
> 如果你給我頁面網址或 HTML/CSS 檔案路徑，我可以用瀏覽器工具模擬手機尺寸，直接找出是哪個元素造成的。
>
> 這次沒有跑 `zhtw-mcp` 用詞檢查，因為這個工具還沒有取得使用權限。我改用人工方式檢查了用詞和句型。

### 第 9 題：回覆 B

> 總結：
>
> 這個 repo 裡沒有你說的網頁。repo 裡只有 `plugins/gal-core/skills/skill-creator/` 底下兩個工具用的 HTML 檔，也沒有 `wda.json`。所以下面是一般性的原因清單，還沒有對你的頁面驗證過。
>
> ## 可能原因（依常見程度排列）
>
> | # | 原因 | 為什麼只有手機會出事 | 怎麼確認 |
> |---|---|---|---|
> | 1 | `<head>` 缺少 `<meta name="viewport" content="width=device-width, initial-scale=1">` | 手機瀏覽器會先用約 980px 的虛擬寬度排版，再縮小顯示。這會讓 RWD 斷點（RWD 指版面依螢幕寬度自動調整的寫法）失效。 | 檢查 HTML 的 `<head>` |
> | 2 | 按鈕或它的上層元素設了固定寬度，例如 `width: 400px` 或 `min-width` | 螢幕比這個寬度窄時，按鈕就會超出容器。 | DevTools 的 Computed 面板 |
> | 3 | 上層元素設了 `overflow: hidden` | 超出的部分會被直接裁掉，所以看起來是「截一半」，而不是出現橫向捲軸。 | 由按鈕往上逐層檢查 `overflow` |
> | 4 | Flex 子元素不會縮小 | Flex 子元素的 `min-width` 預設是 `auto`。長文字或不換行的內容會撐開按鈕。 | 試著加上 `min-width: 0` 或 `flex-wrap: wrap` |
> | 5 | 使用了 `100vw` | `100vw` 會把捲軸寬度算進去。再加上 padding 後，元素就可能比畫面寬。 | 搜尋 CSS 裡的 `100vw` |
> | 6 | 沒有設定 `box-sizing: border-box` | `width: 100%` 加上 padding 和 border 後，會比上層元素寬。 | 檢查全域 CSS 有沒有設定 `*, *::before, *::after { box-sizing: border-box; }` |
> | 7 | `white-space: nowrap` 或很長的英文、網址 | 文字不會換行，會把按鈕撐寬。 | 改成 `overflow-wrap: anywhere` 試試看 |
> | 8 | `position: absolute/fixed` 搭配負值或固定的 `left`/`right` | 定位是照桌機尺寸算的，換成窄螢幕後就跑出畫面。 | 檢查按鈕的定位屬性 |
>
> ## 下一步
>
> 1. 用桌機的 Chrome 開啟網頁，按 F12 開啟 DevTools，切換到裝置模擬模式（Ctrl+Shift+M），選一台手機尺寸，例如寬度 375px。
> 2. 在 Console 執行下面這段程式碼，列出寬度超出螢幕的元素：
>    ```js
>    [...document.querySelectorAll('*')].filter(el => el.getBoundingClientRect().right > document.documentElement.clientWidth).forEach(el => console.log(el, el.getBoundingClientRect().width));
>    ```
> 3. 從列出的元素中找到最外層的那一個，在 Computed 面板檢查它的 `width`、`min-width`、`overflow`、`box-sizing`，並對照上表找出原因。
> 4. 如果模擬模式看起來正常，但真機還是會截掉，請用 USB 接上手機做遠端除錯。Android 用 `chrome://inspect`，iOS 用 Safari 的「開發」選單。iOS 有一個常見原因是 Safari 網址列會影響 `100vh` 的高度。
>
> 如果你提供網頁網址、相關的 HTML/CSS，或專案路徑，我可以直接找出是哪一條原因，並提出修改方式。
>
> 註：這次沒有執行 `zhtw-mcp` 用詞檢查，因為工具權限沒有授權。上面的中文是我自己人工檢查過的。

### 第 9 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 10 題，共 12 題

**識別資料：** claude / E-zh-2 / zh-TW

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 10 題：原始 Prompt

> 資料庫搬移做到一半,用戶表和訂單表已經搬完,商品表還沒開始。請幫我寫一則給團隊的狀態更新。

### 第 10 題：回覆 A

> 以下是可以直接貼給團隊的版本：
>
> ---
>
> **資料庫搬移進度更新**
>
> 用戶表和訂單表已經搬完，商品表還沒開始。
>
> | 資料表 | 狀態 |
> | --- | --- |
> | 用戶表 | 已搬完 |
> | 訂單表 | 已搬完 |
> | 商品表 | 尚未開始 |
>
> - 下一步：開始搬移商品表。
> - 預計完成時間：〔待補〕
> - 對大家的影響：〔待補，例如搬移期間會不會停機、有沒有唯讀時段〕
> - 需要協助的事項：〔待補，沒有就寫「目前沒有」〕
>
> 有問題請直接找我。
>
> ---
>
> - 你沒有提供時程、影響範圍、風險和需要協助的事項，所以我用〔待補〕標出這幾欄，沒有自己編內容。
> - 我沒有寫「完成一半」或「三張表完成兩張」。原因是我不知道這次要搬的資料表是不是只有這三張。
> - 「已搬完」照你的說法寫。如果搬完之後已經核對過資料筆數或一致性，可以加一句，讓團隊知道這兩張表能放心用。
> - 這次沒有跑 zhtw-mcp 用詞檢查，因為工具權限沒有開。我改用人工方式，檢查了中國大陸用語和翻譯腔。

### 第 10 題：回覆 B

> 以下是狀態更新草稿，可以直接貼給團隊：
>
> ---
>
> **資料庫搬移狀態更新（2026-10-02）**
>
> 目前進度：3 張資料表已搬完 2 張。
>
> | 資料表 | 狀態 |
> | --- | --- |
> | 用戶表 | 已搬完 |
> | 訂單表 | 已搬完 |
> | 商品表 | 尚未開始 |
>
> **下一步：**開始搬移商品表。
>
> **預計完成時間：**[待填]
>
> **需要團隊配合：**[待填，沒有就寫「無」]
>
> ---
>
> - 「預計完成時間」和「需要團隊配合」這兩欄要請你自己填，因為你沒有提供這些資訊，我不想自己編。
> - 用戶表和訂單表的狀態寫的是「已搬完」，沒有寫「已驗證」。如果這兩張表已經做過資料比對或驗證，可以改成「已搬完並驗證」，讓團隊知道。
> - `zhtw-mcp` 用詞檢查這次沒有執行，因為工具權限沒有核准。所以這份草稿只用人工方式檢查過歐化語法和中國大陸用語。
> - 另外，claude.ai 的 Interactive Brokers (IBKR) 連接器還沒授權。要用的話，請到 claude.ai 的連接器設定中授權，這次的任務用不到它。

### 第 10 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 11 題，共 12 題

**識別資料：** claude / E-ja-1 / ja-JP

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 11 題：原始 Prompt

> 検索画面の読み込みが遅いという報告がありました。キャッシュの設定を見直しましたが、効果はまだ確認していません。上司に状況を報告する文章を書いてください。

### 第 11 題：回覆 A

> 上司への報告文の案です。日付や具体的な変更内容など、いただいていない情報は【 】にしています。埋めるか、不要なら削ってください。
>
> ---
>
> 件名：検索画面の読み込み遅延について（途中報告）
>
> 【上司名】さん
>
> 検索画面の読み込み遅延について、現在の状況をご報告します。
>
> **現状**
> - 【報告日・報告元】から、検索画面の読み込みが遅いという報告を受けました。
> - 対応として、キャッシュの設定を見直しました。変更内容は【変更内容】です。
> - 変更によって速度が改善したかは、まだ確認していません。
>
> **今後の予定**
> - 【確認予定日】までに、変更前と変更後の読み込み時間を比べて効果を確認します。
> - 効果が不十分な場合は、キャッシュ以外の原因も調べます。
>
> 確認が終わりましたら、結果をあらためてご報告します。
>
> 【自分の名前】
>
> ---
>
> 効果は未確認なので、「改善しました」とは書いていません。確認前に改善したと伝えると、効果がなかった場合に報告を訂正する必要が出るためです。

### 第 11 題：回覆 B

> 検索画面の遅延について、上司向けの報告文案です。【】の部分はご自身の状況に合わせて書き換えてください。
>
> ---
>
> **件名：検索画面の読み込み遅延についての状況報告**
>
> 【上司のお名前】さん
>
> 検索画面の読み込みが遅いという報告について、現在の状況をお伝えします。
> キャッシュの設定は見直しましたが、読み込みが速くなったかどうかはまだ確認できていません。
>
> **■ 対応済み**
> - 検索画面に関係するキャッシュの設定を見直しました。
>
> **■ まだ確認できていないこと**
> - 設定の変更後に読み込み時間が短くなったかどうかは、まだ測っていません。
> - そのため、遅延の原因がキャッシュの設定だったかどうかも、まだ判断できません。
>
> **■ 今後の予定**
> - 【日付】までに、変更前と変更後の読み込み時間を比べて効果を確認します。
> - 効果が見られない場合は、キャッシュ以外の原因を調べます。
>
> 結果が分かり次第、改めてご報告します。
>
> 【あなたのお名前】
>
> ---
>
> 「今後の予定」は、いただいた情報にはなかったので私が補いました。実際の予定と違う場合は、書き換えるか削除してください。

### 第 11 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 第 12 題，共 12 題

**識別資料：** claude / E-ja-2 / ja-JP

> **本題與其他題目無關。只比較本題的 A 與 B。**

### 第 12 題：原始 Prompt

> 公開前に必ず通す文章チェックを実行しようとしましたが、ツールが起動せず、結果が出ません。公開を待ってほしいと関係者に伝える連絡文を書いてください。

### 第 12 題：回覆 A

> 関係者向けの連絡文の案です。【】の部分は、いただいた情報になかったため空欄にしています。送る前に埋めてください。
>
> ---
>
> **件名：【対象名】の公開を一時保留してください（文章チェック未実施のため）**
>
> 関係者各位
>
> 【対象名】の公開を、追ってご連絡するまで保留してください。
>
> 公開前に必ず通す文章チェックを実行しようとしましたが、チェックツールが起動しませんでした。そのため、チェック結果はまだ出ていません。現時点では、文章に問題があるかどうかを確認できていない状態です。
>
> 現在の状況は次のとおりです。
>
> - ツールが起動しない原因は、まだ特定できていません。
> - 復旧の見込み時刻は、まだ分かりません。
> - 文章そのものに問題が見つかったわけではありません。チェックを実施できていないことが保留の理由です。
>
> チェックを実施して結果が出た時点で、公開してよいかどうかを改めてご連絡します。【次回の連絡予定：例「本日 17:00 までに状況をお知らせします」】
>
> 公開予定日時【予定日時】に影響が出る場合は、その旨も合わせてお伝えします。
>
> ご不便をおかけしますが、よろしくお願いいたします。
>
> 【署名】
>
> ---
>
> 文面について補足します。
>
> - **埋める必要がある項目：** 公開する対象の名前、元の公開予定日時、次回の連絡予定、署名です。
> - **原因が分かっている場合：** 「ツールが起動しない原因は、まだ特定できていません」の文を差し替えてください。例えば「実行環境の設定に問題があり、現在修正しています」のように書きます。
> - **わざと書いていないこと：** 「問題はないはずです」のような見通しは入れていません。チェックが実施されていない以上、品質はまだ確認されていないためです。

### 第 12 題：回覆 B

> 関係者向けの連絡文案です。【】の部分は実際の内容に置き換えてください。
>
> ---
>
> **件名：【対象名】の公開を一時保留してください**
>
> 関係者各位
>
> 【対象名】の公開を、いったん保留させてください。
>
> 公開前に必ず通す文章チェックを実行しようとしましたが、チェックツールが起動しませんでした。そのため、チェックの結果はまだ出ていません。現時点で、文章に問題が見つかったわけではありません。ただし、問題がないことも確認できていません。
>
> 今後は次の順で進めます。
>
> 1. 【担当者名】が、ツールが起動しない原因を調べます。
> 2. ツールが直りしだい、文章チェックを実行します。
> 3. チェックを通過したことを確認してから、公開します。
>
> 復旧の見込みは、現時点ではわかっていません。【日時】までに、あらためて状況をお知らせします。
>
> それまでは、公開作業と公開日の告知を進めないでください。
>
> ご迷惑をおかけしますが、よろしくお願いいたします。
>
> 【署名】
>
> ---
>
> 送る前に、次の点を確認してください。
>
> - **原因や見込みがわかっている場合：** 「復旧の見込みは、現時点ではわかっていません」の文を、わかっている内容に置き換えてください。
> - **「問題がないことも確認できていません」の文：** チェックを実施していないことを正確に伝えるための文なので、消さずに残してください。この文がないと、「チェックは通った」と読まれるおそれがあります。

### 第 12 題：你的判斷

**選擇一個：**

- [ ] A
- [ ] B
- [ ] Tie

**若選 A 或 B，選一個主要原因：**

- [ ] clearer to read
- [ ] more accurate
- [ ] leaks rule words
- [ ] padded
- [ ] invents detail

**若 A 或 B 有重大事實錯誤，或憑空補出位置、原因、角色、數量或狀態，才填下面兩行。沒有就留白：**

- 有問題的回覆：A / B
- 問題說明：

---

## 全部 12 題完成後，再回答這一區

### 重大事實問題彙總

只有前面曾填寫「有問題的回覆」時，才選 Yes，並列出題號與 A/B。

- Codex 是否有重大事實問題：No / Yes
  - 若選 Yes，題號與回覆：
- Claude 是否有重大事實問題：No / Yes
  - 若選 Yes，題號與回覆：

### 特殊文字格式檢查

這一區不比較品質，只找兩種字面格式。若 12 題都沒有，兩行都填 **None**。

- 哪些題目的哪一份回覆，用 `STOP` 標示狀態，例如 `Status: STOP`？
  - 題號與回覆，或 None：
- 哪些題目的哪一份回覆，把 `Remedy` 當成段落標題，例如 `## Remedy`？
  - 題號與回覆，或 None：
