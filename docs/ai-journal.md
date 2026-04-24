# AI 協作日誌 — FROST 門檻簽名錢包 Demo

這份日誌記錄開發 2-of-2 FROST 門檻簽名 Solana 錢包 demo 過程中的 AI 協作歷程。以第一人稱（我）的視角撰寫。

**作業背景：** 40% 的分數依據 AI 協作過程的文件記錄。我使用多代理 AI 協作流程，由 PM agent（Pam）統籌協調後端、前端、設計等專業 agent。

**我的起點：** 對 Rust、FROST 密碼學、Solana 均無任何先備經驗。

---

## 我追蹤的評量標準

| 評量標準 | 狀態 | 備註 |
|---|---|---|
| 未知領域探索 | 已記錄 | 進場時對 Rust/FROST/Solana 零經驗 |
| 複雜問題拆解 | 已記錄 | 11 個有明確依賴關係的有界 PRD |
| Prompt 工程與 context 管理 | 已記錄 | 多代理協作，durable context 寫入 AGENTS.md |
| 修正與 debug | 進行中 | PG 18 volume 問題已記錄；後續還有更多 |
| 策略取捨與方向判斷 | 已記錄 | REST vs WebSocket、測試策略、審查模式、UI 方向 |

---

## 時間軸

### 需求分析 — 2026-04-21

**問題：** 在做任何事之前，我需要先搞清楚這份作業的完整範圍。規格（ASSIGNMENT_zh.md）以中文撰寫，涵蓋我從未接觸過的密碼學協議。

**AI 互動：** 我把完整規格餵給 AI，請它識別核心協議流程、技術棧限制和關鍵未知項目。

**AI 幫我釐清的內容：**
- 需要實作的三個核心協議流程：
  - **DKG（Distributed Key Generation）：** 三輪流程，兩個 node 協作產生共享金鑰對，過程中任何一方都不持有完整私鑰
  - **錢包衍生：** 利用共享公鑰透過 HD wallet 路徑衍生 Solana 錢包地址
  - **簽名：** 兩輪簽名加一個聚合步驟，全程不重建私鑰
- 規格規定的技術棧：Rust 1.94 + axum、Next.js 16、frost-ed25519、hd-wallet、PostgreSQL 18、Solana Devnet

**我學到的領域知識：** FROST 是一種門檻簽名方案。「2-of-2」代表每次簽名操作兩方都必須參與——任何一方都無法單獨簽名。密碼學安全性來自私鑰從不在同一處重建這個特性；協議透過雙方計算產生有效簽名。

---

### 腦力激盪與規劃 — 2026-04-21

**問題：** 在產生任何程式碼之前，我需要一個整體開發策略。架構決策做得晚，反悔的代價很高。

**AI 互動：** 我把 AI 當作討論板，聽完各種選項後，由我自己做最終判斷。

**我做的判斷（不是 AI 決定的）：**

**1. Coordinator ↔ Node 通訊用 REST 而非 WebSocket**
- AI 的建議：用 WebSocket 降低延遲、支援即時 round 狀態
- 我的判斷：這是 demo 系統。來回延遲可接受。REST 實作、debug、說明都簡單得多。面試看的是清晰度和正確性，不是生產級效能。

**2. 不採用 test-first**
- AI 的建議：TDD，先寫測試再實作
- 我的判斷：我看了評分標準——40% 驗收條件、40% AI 歷程、20% 工程品質。工程品質是最小的一塊。更重要的是，我對 Rust 零經驗。在我還不懂自己在蓋什麼的情況下，先為一個不熟的語言和領域寫測試，只會比先蓋再測更慢、更容易出錯。等核心邏輯跑通再補測試。

**3. Docker Compose 作為單一的開發與交付環境**
- AI 的建議：分開維護 dev 和 prod 兩份 Compose 檔
- 我的判斷：評審需要 `docker compose up` 能跑。如果開發和交付用不同設定，我就有在 dev 正常、交付時壞掉的風險。單一 Compose 設定降低這個風險。

**4. 架構決策由我審查，實作細節由 AI 自審**
- AI 的建議：可以把更多審查工作委派給它
- 我的判斷：我想真的理解這個系統，不是橡皮圖章 AI 輸出。API 合約、DB schema、密碼學邏輯必須由我審查，因為這些決策會串聯影響所有後續工作。標準的前端配線和 Docker 設定風險較低——那些我會信任 AI 自審，自己抽查即可。

**5. UI 方向：精緻的開發者工具風格，深色主題**
- AI 的建議：功能性但極簡的 UI 以節省時間
- 我的判斷：評審會直接用這個 UI。過於陽春的介面讀起來像「時間不夠用」。精緻的深色開發者工具風格傳遞出用心，又不至於過度設計。

---

### 問題拆解 — 2026-04-21

**問題：** 整個系統太大，任何單一 AI agent 都無法在一個 context window 內不失去連貫性地完成。我需要把它切成各自可執行的小塊。

**AI 互動：** 我與 PM agent 合作，把系統拆解為階段和 PRD。

**結果：** 4 個階段、11 個 PRD，每個範圍對應一次專業 agent 的工作 session。

- **Phase 1：基礎建設**
  - fr-001：Docker Compose 環境
  - fr-002：API 合約與 DB schema
  - fr-003：UI 設計系統與 mockup

- **Phase 2：後端核心**
  - fr-004：DKG 協議實作（Rust）
  - fr-005：錢包衍生
  - fr-006：簽名協議實作（Rust）
  - fr-007：REST API 層（axum）

- **Phase 3：前端實作**
  - fr-008：Next.js 應用程式鷹架與路由
  - fr-009：DKG、錢包、簽名 UI 接線

- **Phase 4：整合與交付**
  - fr-010：端對端整合測試
  - fr-011：Docker Compose 強化與提交準備

**為什麼這樣拆：** 每個 PRD 小到足以讓專業 agent 在一個 session 中掌握完整 context。依賴關係明確——API 合約（fr-002）先於任何實作 PRD，避免 agent 各自發明不相容的介面。架構決策集中在 Phase 1 前置，讓後續 agent 不需要臨時做基礎性選擇。

---

### 環境建置（fr-001）— 2026-04-21

**問題：** 建立一個包含 coordinator 服務（Rust/axum）、兩個 node 服務（Rust/axum）、前端（Next.js）和 PostgreSQL 18 的 Docker Compose 環境。所有服務需要能從零開始可靠地 build 起來。

**AI 互動：** 我把技術棧需求給後端 agent，請它產出 Docker Compose 設定和 Dockerfiles。

**AI 產出的內容：**
- 有 5 個服務的 `docker-compose.yml`
- 使用 cargo-chef 做依賴層快取的多階段 Rust Dockerfile（讓重複 build 時不必從頭重新編譯所有依賴）

**修正過程 — PostgreSQL 18 資料目錄：**
- 問題：PostgreSQL 容器一直回報「unhealthy」並崩潰。
- 如何發現：`docker compose logs postgres` 顯示資料目錄路徑的初始化錯誤。
- 根本原因：PostgreSQL 18 改變了資料目錄結構。舊路徑是 `/var/lib/postgresql/data`，PostgreSQL 18 使用 `/var/lib/postgresql/<version>/`（例如 `/var/lib/postgresql/18/`）。AI 在 volume mount 中使用了舊路徑，導致容器初始化失敗。
- 修正：把 `docker-compose.yml` 中的 volume mount 路徑更新為正確的版本化路徑。
- 教訓：AI 的訓練資料有截止日期。非常近期的軟體版本的 breaking change 可能未被反映。服務在啟動時崩潰，要先讀 log 再假設設定邏輯有問題——實際的錯誤訊息幾乎都比我的初始猜測更具體。

---

### UI 設計（fr-003）— 2026-04-21

**問題：** 在寫任何 HTML 之前，我需要先確認 UI 結構確實正確反映了協議——特別是「逐步觸發」的需求（每個 Node × Round 有各自的按鈕）是否有被捕捉到。

**AI 互動：** 我請設計 agent 先產 ASCII wireframe，在任何程式碼之前，這樣我可以便宜地審查版面。

**為什麼先做 ASCII：** 審查 wireframe 需要 2 分鐘。審查 HTML mockup 再要求結構性重工花的時間長得多。用低保真格式前置版面審查，整體上節省時間。

**我在 wireframe 中審查的內容：**
- 三頁籤結構：DKG | Wallets | Signing
- DKG 頁籤：確認每個 node 每個 round 有各自的觸發按鈕（不是單一的「執行 DKG」按鈕），這是規格對 demo 的要求
- Wallet 頁籤：地址顯示及衍生路徑
- Signing 頁籤：訊息輸入、每個 node 的 round 觸發器、聚合簽名輸出

**我確認 wireframe 版面後：** agent 產出了完整的 HTML mockup（使用 Tailwind CSS），以及記錄調色盤、字體排印和元件模式的設計指南，供前端 agent 遵循。

---

### API 合約與 DB Schema（fr-002）— 2026-04-21

**問題：** 在後端實作開始之前，需要有明確的 API 合約和資料庫 schema，讓所有後續 agent 有共同的介面可以遵循，避免各自發明不相容的設計。

**AI 互動：** 我把規格需求和協議流程給後端 agent，請它設計完整的 REST API 端點和 DB schema，並說明審查框架：API 合約和 schema 是高風險決策，我會親自審查，實作細節由 agent 自審。

**AI 設計並產出的內容：**

*API 端點：*
- 12 個 Frontend → Coordinator 端點，涵蓋 DKG 啟動/輪詢、錢包衍生、簽名發起/輪詢、狀態查詢
- 5 個 Coordinator → Node 端點，供 coordinator 在各協議 round 中呼叫各個 node
- 每個端點有完整的 request/response schema 和明確的 error cases

*資料庫設計：*
- 三個獨立資料庫：`coordinator_db`、`node_a_db`、`node_b_db`
- 7 個 sqlx migration 檔，定義各服務的資料表結構

**我的審查判斷：**

**1. 三個 DB 隔離方向正確**
- AI 的設計：三個服務各自有獨立資料庫，在基礎設施層強制資料隔離
- 我的判斷：方向正確。安全邊界在基礎設施層強制，比靠應用層程式碼隔離更可靠。這也符合 demo 的展示目的——讓評審能清楚看到 node 之間沒有私鑰資料流動。

**2. Signing lifecycle 7 個狀態與 UI 一致**
- AI 設計了 signing session 的 7 個狀態（`pending` → `round1_collecting` → `round1_complete` → `round2_collecting` → `round2_complete` → `aggregating` → `complete`）
- 我把這 7 個狀態對比 fr-003 wireframe 裡的 status stepper，確認每個 UI 步驟都有對應的資料庫狀態支撐，不會有 UI 顯示某個階段但後端沒有對應狀態可以查詢的漏洞。

**我在這個過程學到的：**

- **Coordinator 在 DKG 階段不需要 FROST 函式庫：** Coordinator 在 DKG 階段的角色只是轉發 opaque JSON——它把 node A 的輸出轉給 node B，反之亦然。真正執行 FROST 運算的是兩個 node。這對我來說是反直覺的；我本來以為 coordinator 需要「理解」密碼學內容。
- **Nonce 重複使用是安全漏洞：** 在 FROST 簽名中，nonce 重複使用會洩漏私鑰份額。AI 的設計用 `UNIQUE constraint`（防止資料庫層面的重複插入）加上應用層在聚合後刪除 nonce 記錄，做到縱深防禦。這兩層保護都有其必要，光靠其中一層都不夠。

---

### 後端 API 基礎建設（fr-007）— 2026-04-22

**問題：** 後端需要完整的 HTTP server 基礎建設 — 路由、資料庫連線池、migration、錯誤處理、middleware — 讓後續的密碼學邏輯 PRD（DKG、錢包衍生、簽名）有現成的 shell 可以填入業務邏輯。

**AI 互動：** 派後端 agent 根據 API 合約文件，為 Coordinator 和 TSS Node 兩個服務建立完整的 axum server 架構。所有 route handler 先 return 501 Not Implemented，等業務邏輯 PRD 來填。

**AI 產出的內容：**
- Coordinator 15 個 .rs 檔案：`main.rs`（AppState + PgPool + sqlx migrate + CORS + tracing + graceful shutdown）、`config.rs`、`error.rs`（snafu）、`routes/`（DKG 3 端點、wallets 3 端點、signing 5 端點）、`models/`、`db/`
- TSS Node 12 個 .rs 檔案：同樣完整架構，含 `signing_nonces` delete（nonce 安全清除）
- 兩個 crate 都 zero compile errors

**我的審查判斷：**
- handler 全部 return 501：正確，這是設計如此，確保基礎設施層和業務邏輯層職責分離
- CORS 設為 permissive：demo 用途可接受
- `dead_code` warnings：預期內，等業務邏輯消費那些 type 就會消失

**修正過程 — migration 路徑問題：**
- 問題：`sqlx::migrate!` 使用相對路徑 `"../migrations/coordinator"`，從 crate 目錄回溯上層再進入 migrations 資料夾
- 我的判斷：這不直覺也容易斷。應該把 migrations 搬進各自 crate 目錄（`backend/coordinator/migrations/`），讓路徑變成 `"./migrations"` — 每個 crate 自包含，路徑清楚
- 決定：不為此重新派工，順便在下一個 PRD（fr-004）處理即可。這是「小問題不值得獨立修復輪次」的判斷

**修正過程 — PRD 依賴順序錯誤：**
- 問題：原本的 plan 中 fr-004（DKG 邏輯）和 fr-007（API 基礎設施）是平行的，都只依賴 fr-002。但邏輯上 fr-007 是 fr-004/005/006 的基礎設施層 — 如果 DKG 邏輯先跑，agent 會需要自己搭 axum routing，然後 fr-007 再來做同樣的事就會衝突或重工
- 如何發現：在 review fr-002 完成、準備 dispatch 下一批 PRD 時，我比對了 fr-004 和 fr-007 的 scope 描述，發現它們之間有隱含的依賴
- 修正：補上 fr-004/005/006 depends_on fr-007 的依賴關係，確保基礎設施先做
- 教訓：問題拆解時容易把「概念上的獨立」和「實作上的順序」搞混。兩個 PRD 在功能面上各管各的，不代表它們在程式碼層面沒有依賴。這種依賴需要在 review 時主動檢查

---

### 前端 DKG 與錢包介面（fr-008）— 2026-04-22

**問題：** 需要把 fr-003 的 UI 設計和 fr-002 的 API 合約落地為可互動的前端介面。DKG 和錢包是使用者操作的前兩步，必須能逐步觸發每個 Node × Round。

**AI 互動：** 派前端 agent，給它 HTML mockup、設計指南和 API 合約作為參考。這個 PRD 設為 self-review（AI 自審），因為前端配線屬於我在規劃階段判定的「低風險、可信任 AI 自審」類別。

**AI 產出的內容：**
- `app/page.tsx`：主頁面 shell — header、三 tab navigation（DKG / Wallets / Signing）、網路連線狀態指示燈（green/yellow/red）
- `app/components/dkg-panel.tsx`：完整 DKG 介面 — 雙 Node panel、per-round 觸發按鈕（含前置條件檢查和 disable 邏輯）、6 段進度條、Master Public Key 顯示 + 複製、5 秒 polling
- `app/components/wallets-panel.tsx`：錢包管理 — 建立錢包、列表（截短地址 + copy）、餘額查詢 + refresh、選擇 sender
- `app/lib/api.ts`：typed API client，所有 Coordinator 端點的 fetch wrapper
- `app/components/transactions-panel.tsx`：Signing tab placeholder（留給 fr-009）
- 設計系統 token（globals.css）在前一輪已建立

**我觀察到的：**
- agent 在 spec 之外加了網路連線狀態指示燈（backend 沒跑時顯示紅色）。這個判斷不錯 — 因為開發過程中 backend 經常不在線，有即時的連線回饋比看 console error 直覺得多
- DKG round 的前置條件邏輯（Round 2 需要兩個 node 都完成 Round 1）同時在 client-side（disable 按鈕）和 server-side（Coordinator 回 409）雙重執行。跟 nonce 的縱深防禦思路一致
- Tailwind v4 用 CSS-based config（`@theme inline`）取代 `tailwind.config.js`，自訂 color token 直接寫在 `globals.css` 裡就好

---

### 後端 DKG 協議實作（fr-004）— 2026-04-22

  **問題：** 實作 FROST 門檻簽名的核心——分散式金鑰生成（DKG）。兩個 node
  透過三輪協議協作產生共享金鑰對，任何一方都不持有完整私鑰。這是整個系統密碼學正確性的基礎。

  **AI 互動：** 派後端 agent 實作完整的 DKG 三輪流程。給它 frost-ed25519 的 API 範例、API 合約和 DB
  schema 作為參考。這是 human review PRD——密碼學邏輯我必須自己看懂才能放行。

  **AI 產出的內容：**
  - TSS Node 端三個 round handler（+282 行）：直接調用 `frost_ed25519::keys::dkg::{part1, part2,
  part3}`
  - Coordinator 端 DKG 協調邏輯（+475 行）：透過 reqwest proxy 到 node，追蹤 per-node round
  進度，偵測 DKG 完成
  - 順便完成了 fr-007 review 時提出的 migration 路徑重構（搬進各 crate 目錄）

  **我的審查判斷：**
  - 我逐步追蹤了三輪的資料流向：R1 各自產生承諾 → R2 交換承諾後計算加密份額 → R3
  驗證份額並產生最終金鑰。Coordinator 在每輪都有前置條件檢查（必須雙方都完成上一輪），防止跳步
  - 確認 KeyPackage（私鑰份額）只存在 node 自己的 DB，Coordinator 只存公開資料（group public key 的
  Base58 字串）
  - 確認重複執行防護：每個 node 的每個 round 做過就不能再做
  - 確認前端 6 個獨立按鈕（2 nodes × 3 rounds）的設計正確反映了協議本質——每個 node
  各自操作，不存在「同時觸發」的需求。面試官能逐步觀察協議推進

  **我學到的：**
  - FROST DKG 的三輪本質：Round 1 是承諾（commitment），Round 2 是交換（secret sharing），Round 3
  是驗證與最終化。每輪都必須雙方完成才能進下一輪，這不是實作上的限制而是密碼學協議本身的要求
  - `rand::thread_rng()` 在 Rust 中是 `!Send`，不能用在 axum 的 async handler 裡，改用 `OsRng`
  - FROST `Identifier` 可以從字串確定性衍生（`Identifier::derive()`），不需要手動分配數字 ID
  - `VerifyingKey::serialize()` 回傳 32-byte compressed Edwards Y 座標，Base58 編碼後直接就是 Solana
  地址格式

---

### 前端簽名與交易介面（fr-009）— 2026-04-22

  **問題：** 實作 FROST 簽名流程的前端介面。使用者需要能建立簽名請求、逐步觸發每個 Node
  的兩輪簽名、聚合並廣播到 Solana
  Devnet，以及追蹤交易狀態直到確認。這是三個核心流程中互動最複雜的一個。

  **AI 互動：** 派前端 agent，給它 HTML mockup 的 Signing tab、設計指南和 API 合約。設為
  self-review，跟 fr-008 一樣的理由——前端配線屬於低風險類別。fr-008 已經建好 tab
  navigation、設計系統和 API client 基礎，這次只需要填入 Signing tab 的完整內容。

  **AI 產出的內容：**
  - `transactions-panel.tsx` 完整重寫（~550 行）：建立簽名請求表單（錢包下拉選單 + 收款地址 Base58
  驗證 + 金額輸入）、split-view 版面（左側 request list + 右側 detail panel）、水平 status timeline
  stepper（6 步驟）、雙 Node panel 各有獨立的 per-round Execute 按鈕、Aggregate & Broadcast
  按鈕（兩個 node 都完 Round 2 才解鎖）、交易結果卡片附 Solana Explorer 連結
  - `api.ts` 加入 5 個 signing API functions（createSigningRequest, listSigningRequests,
  getSigningRequest, executeSigningRound, aggregateAndBroadcast）
  - `page.tsx` 更新，把 dkgComplete 和 selectedWalletIndex 傳給 TransactionsPanel

  **我觀察到的：**
  - 簽名流程只有 2 rounds per node（不像 DKG 有 3 rounds），但多了一個 Aggregate & Broadcast
  步驟作為獨立按鈕，有自己的解鎖條件。整體互動模式跟 DKG panel
  一致，但生命週期更長（要追蹤到鏈上確認）
  - 錢包下拉選單會自動帶入 Wallets tab 已選的 sender（透過 page.tsx 提升的 selectedWalletIndex
  state），跨 tab 狀態傳遞設計在 fr-008 就預先做好了
  - Explorer URL 同時支援 API 回傳的 explorer_url 和 client-side 用 tx_signature 自行組裝，做了
  fallback 處理

  ---

## 持續記錄的模板

```markdown
### [階段/任務名稱] — [日期]

**問題：** 你要解決什麼？

**AI 互動：** 你如何引導 AI？

**AI 設計/產出了什麼：** AI 做了什麼具體工作？

**我的審查判斷：** 我確認了什麼、調整了什麼方向、做了哪些策略取捨？
- 判斷：[內容]
- AI 的設計：[替代方案]
- 我的理由：[為什麼這樣判斷]

**修正過程：** AI 有沒有出錯？你怎麼修正的？
- 問題：[出了什麼錯]
- 如何發現：[怎麼注意到的]
- 修正：[做了什麼]
- 教訓：[學到什麼]

**我學到的：** 透過這次互動獲得的新領域知識
```
