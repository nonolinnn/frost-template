# AI 協作日誌 — FROST 門檻簽名錢包 Demo

這份日誌記錄開發 2-of-2 FROST 門檻簽名 Solana 錢包 demo 過程中的 AI 協作歷程。以第一人稱（我）的視角撰寫。

**我的起點：** 對 Rust、FROST 密碼學、Solana 均無任何先備經驗。

**開發工具：** Claude code、Codex、Gemini；主要透過 Claude 創建不同 Agent 進行開發，透過多代理架構（Multi-agent Setup），確保 Backend Agent 僅持有密碼學與資料庫的 Context，避免無效資訊導致的 Logic Drifting (邏輯飄移)

---

## 開發策略

面對完全陌生的技術領域（FROST 門檻簽名、HD 錢包衍生、Rust/axum、Solana），我採用「快速原型 → 驗證 → 深入理解 → 審查修正」的迭代式開發策略。

核心判斷是：在不理解 FROST 密碼學底層數學的前提下，先讀論文再開始寫是低效的。密碼學論文的抽象描述在沒有具體系統可以對照時很難消化。相反地，採行範例導向開發策略，優先建立可觀測的系統行為，再以此為基礎進行逆向邏輯解構，以此克服密碼學學術文件的認知門檻。

具體來說：
- **Phase 1-3**：AI 輔助實作，每個 PRD 完成後做 review 判斷。在此階段，審查核心聚焦於系統的合理性與最小權限原則的落實，確保密碼學黑盒內外的資料交換符合預期。
- **Phase 4（整合測試）**：親手用 Docker 跑起完整系統、執行 integration test、在 UI 上逐步操作每個 round。這些具體經驗為後續的深入理解提供了錨點。
- **開發過程**：在等待 AI 建構程式過程中，會再利用其他 AI 工具查找資訊，補足陌生領域的先輩知識。
- **最終走讀**：系統跑通後，逐檔 code review。從資料庫 schema → 密碼學核心模組 → API 路由 → 前端元件，由底層往上完整走讀。這時候讀程式碼有了「我親手跑過、看過實際行為」的基礎，理解深度遠超單純讀文件。
- **功能面驗收**：對齊比對 assignment 原文與面試需求，找出潛在風險並做有意識的取捨判斷。

最終的技術掌控力並非源於理論的預習，而是源於對故障排查與 Code Walkthrough 過程中每一個決策節點的深度參與。

---

## Prompt 工程與多代理上下文管理

單一 AI 對話處理不了這個系統的完整規模——密碼學、Rust 後端、前端、Docker、設計，每一塊的 context 累積起來會導致 Context Fragmenting（上下文碎片化），進而引發邏輯漂移。我採用多代理協作框架，把「統籌判斷」與「實作執行」解耦：由一個具備全局狀態的 Dispatcher (Main Agent) 負責任務分解，專業 Agent 則在受限的 Context 內執行原子任務。

### 兩層 context 策略

每次 dispatch 一個 agent，它的 context 由兩個層次構成：

**Layer 1 — 標準化 durable context**

透過 Claude Code 的多代理框架管理跨 session 的共用知識。每個 agent 啟動時都會取得一份涵蓋技術棧規格、安全邊界約束和 codebase convention 的標準 context，而不是每次在 prompt 裡重新解釋。這樣做有兩個效果：節省每次 dispatch 的 prompt 空間，以及避免不同 agent 各自對同一件事產生不同理解（例如 Rust dependency 要加在哪、私鑰絕對不能離開 Node 這類硬約束）。

**Layer 2 — PRD（task-specific context）**

每個任務對應一份 PRD，PRD 是給 agent 的結構化 prompt，不是模糊的指令。以 fr-004（DKG 實作）為例，除了 acceptance criteria，Implementation Notes 還包含 frost-ed25519 的正確 API 呼叫方式（避免 agent 猜測或查錯版本）、明確的安全邊界約束（KeyPackage 只能存在 Node 自己的 DB，Coordinator 只碰公開資料），以及明確的 Out of Scope 邊界（HD 衍生、簽名、前端不在這個 PRD 範圍內）。

「把完整系統丟給 AI，請它全部做完」和「給它一份有明確邊界、參考資料和安全約束的有界 spec」，產出品質是兩個量級的差距。
每個 PRD 是具備高度約束力的結構化指令。針對密碼學核心（如 fr-004），我採取 「研究先行」 策略：先由 Research Agent 檢索 frost-ed25519 的官方文件與特定版本 (v2.1.0) 範例，經我人工驗證後寫入 Implementation Notes。

### Review mode 決策

每個 PRD 都有明確的 `review_mode`：

| PRD | review_mode | 理由 |
|-----|-------------|------|
| fr-002（API 合約 + Schema） | human | 所有後續 PRD 都依賴它，錯一個地方全部連帶錯 |
| fr-004（DKG 密碼學） | human | 密碼學邊界我必須親自確認，特別是私鑰不能離開 Node |
| fr-006（簽名 + 廣播） | human | 安全關鍵路徑 |
| fr-008（前端配線） | self | 低風險，UI 錯了容易看到、容易改 |
| fr-009（簽名 UI） | self | 同上，且 fr-008 已建好設計系統基礎 |

review_mode 本身就是 context 管理的一部分：我把自己的審查精力集中在高風險的密碼學和 API 邊界，不在前端 CSS 上浪費注意力。

### 跨 session 的 context 連續性

多代理框架的另一個挑戰是：每個 agent 啟動時都是全新的 context，沒有上一個 session 的記憶。我用兩個機制處理這個問題：

**Work Log**：每個 PRD 完成後，agent 把關鍵決定和發現寫進 Work Log。下一個 agent 讀到的不是空白，而是前一個人已經確認的事實（例如 fr-004 Work Log 記錄了 `OsRng` 取代 `thread_rng()` 的原因，fr-006 agent 就不需要重新踩這個坑）。

**精準重派**：fr-005+fr-006 合併 dispatch 時撞到 rate limit，fr-005 做完但 fr-006 coordinator 端還是 stub。重新 dispatch 時我沒有叫 agent 「重做 fr-005 和 fr-006」，而是精確描述殘餘狀態：「fr-005 已完成，fr-006 的 tss-node 端完成，只剩 coordinator 的 signing 路由是 501 stub」。這樣 agent 不浪費 token 重做已完成的工作，也不因為不清楚進度而做錯的事。

---

## 時間軸

### 需求分析 — 2026-04-21

**問題：** 在做任何事之前，我需要先搞清楚這份作業的完整範圍。規格（ASSIGNMENT_zh.md）以中文撰寫，涵蓋我從未接觸過的密碼學協議。

**AI 互動：** 我把完整規格餵給 AI，請它識別核心協議流程、技術棧限制和關鍵未知項目。

**釐清的內容：**
- 需要實作的三個核心協議流程：
  - **DKG（Distributed Key Generation）：** 三輪流程，兩個 node 協作產生共享金鑰對，過程中任何一方都不持有完整私鑰
  - **錢包衍生：** 利用共享公鑰透過 HD wallet 路徑衍生 Solana 錢包地址
  - **簽名：** 兩輪簽名加一個聚合步驟，全程不重建私鑰
- 規格規定的技術棧：Rust 1.94 + axum、Next.js 16、frost-ed25519、hd-wallet、PostgreSQL 18、Solana Devnet

**領域知識：** FROST 是一種門檻簽名方案。「2-of-2」代表每次簽名操作兩方都必須參與——任何一方都無法單獨簽名。密碼學安全性來自私鑰從不在同一處重建這個特性；協議透過雙方計算產生有效簽名。

---

### 設計與規劃 — 2026-04-21

**問題：** 為了避免架構決策做得晚，反悔的代價很高，在產生任何程式碼之前，優先制定好實作方向。

**AI 互動：** 透過與 AI agent 討論，找出最好的實作方向。

**與 AI agent 討論重點**

**1. Coordinator ↔ Node 通訊用 REST 而非 WebSocket**
- AI 的建議：用 WebSocket 降低延遲、支援即時 round 狀態
- 思考判斷：WebSocket 好處是 server-push，但功能上每個步驟都是使用者手動觸發，沒有需要即時推送的場景。5 秒 polling 完全夠用。換來的是：REST 無狀態、失敗直接重試、不需要重連邏輯、debug 也更直觀。對 demo 系統，選最容易講清楚、最不容易出奇怪問題的方案，比追求生產級效能更重要。

**2. 不採用 test-first**
- AI 的建議：TDD，先寫測試再實作
- 思考判斷：考量到我對 Rust 零經驗。在我還不懂自己在蓋什麼的情況下，先為一個不熟的語言和領域寫測試，只會比先蓋再測更慢、更容易出錯。等核心邏輯跑通再補測試。

**3. Docker Compose 作為單一的開發與交付環境**
- AI 的建議：分開維護 dev 和 prod 兩份 Compose 檔
- 思考判斷：作業需要 `docker compose up` 能跑。如果開發和交付用不同設定，就有在 dev 正常、交付時壞掉的風險。單一 Compose 設定降低這個風險。

**4. 架構決策由我審查，實作細節由 AI 自審**
- AI 的建議：可以把更多審查工作委派給它
- 思考判斷：實作上我需要理解這個系統，不是單純 AI 輸出。故駁回 AI 建議，API 合約、DB schema、密碼學邏輯必須由我審查。標準的前端配線和 Docker 設定風險較低——那些我會信任 AI 交叉審查即可。

**5. UI 方向：精緻的開發者工具風格**
- AI 的建議：功能性但極簡的 UI 以節省時間
- 思考判斷：雖然是面試作業，為了避免過於陽春的介面影響用戶體驗，還是在 UI 設計上有所把關。

---

### 問題拆解 — 2026-04-21

**問題：** 整個系統太大，任何單一 AI agent 都無法在一個 context window 內不失去連貫性地完成。我需要把它切成各自可執行的小塊。

**AI 互動：** 重新讓 AI agent 分析開發內容，把系統拆解為階段和 PRD。

**結果：** 4 個階段、11 個 PRD，每個範圍對應一次專業 agent 的工作 session。

- **Phase 1：基礎建設**
  - fr-001：Docker Compose 環境
  - fr-002：API 合約與 DB schema
  - fr-003：UI 設計系統與 mockup

- **Phase 2：後端核心**
  - fr-007：REST API 層（axum）
  - fr-004：DKG 協議實作（Rust）
  - fr-005：錢包衍生
  - fr-006：簽名協議實作（Rust）

- **Phase 3：前端實作**
  - fr-008：Next.js 應用程式鷹架與路由
  - fr-009：DKG、錢包、簽名 UI 接線

- **Phase 4：整合與交付**
  - fr-010：端對端整合測試
  - fr-011：Docker Compose 強化與提交準備

**為什麼這樣拆：** 
- 1. 降低認知負荷與對抗「邏輯漂移」：我將複雜系統拆解為 11 個具備原子性 (Atomicity) 的 PRD。每個任務的規模都經過精確計算，確保其程式碼增量與邏輯複雜度不超過單一 Agent Session 的 Context Window。這能有效防止 AI 因為處理過多無關資訊而產生的「邏輯漂移 (Logic Drifting)」或幻覺。
- 2. 契約驅動開發 (Contract-Driven Development)：明確定義 API 合約與 DB Schema (fr-002) 為所有開發任務的「前置依賴」。透過先建立系統骨架，我強制後續負責不同模組的 Agent 必須遵循統一的通訊協議，徹底消除因 Agent 自行推論而導致的介面不相容風險。
- 3. 關鍵路徑 (Critical Path) 與風險隔離：我將系統開發劃分為 「密碼學核心」 與 「應用層配線」 兩大維度，將 fr-004、fr-005、fr-006 識別為不可延誤的關鍵節點，確保密碼學核心優先跑通，前端和 Docker 整合可在後期補完。

---

### 環境建置（fr-001）— 2026-04-21

**問題：** 建立一個包含 coordinator 服務（Rust/axum）、兩個 node 服務（Rust/axum）、前端（Next.js）和 PostgreSQL 18 的 Docker Compose 環境。所有服務需要能從零開始可靠地 build 起來。

**AI 互動：** 我把技術棧需求給後端 agent，請它產出 Docker Compose 設定和 Dockerfiles。

**AI 產出的內容：**
- 有 5 個服務的 `docker-compose.yml`
- 使用 cargo-chef 做依賴層快取的多階段 Rust Dockerfile（讓重複 build 時不必從頭重新編譯所有依賴）

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

**與 AI 討論：**

**1. 三個 DB 隔離方向正確**
- AI 的設計：三個服務各自有獨立資料庫，在基礎設施層強制資料隔離
- 思考判斷：方向正確。安全邊界在基礎設施層強制，比靠應用層程式碼隔離更可靠。這也符合 demo 的展示目的——讓評審能清楚看到 node 之間沒有私鑰資料流動。

**2. Signing lifecycle 7 個狀態與 UI 一致**
- AI 設計了 signing session 的 7 個狀態（`pending` → `round1_collecting` → `round1_complete` → `round2_collecting` → `round2_complete` → `aggregating` → `complete`）
- 我把這 7 個狀態對比 fr-003 wireframe 裡的 status stepper，確認每個 UI 步驟都有對應的資料庫狀態支撐，不會有 UI 顯示某個階段但後端沒有對應狀態可以查詢的漏洞。

**補充**

- **Coordinator 在 DKG 階段不需要 FROST 函式庫：** Coordinator 在 DKG 階段的角色只是轉發 opaque JSON——它把 node A 的輸出轉給 node B，反之亦然。真正執行 FROST 運算的是兩個 node。這對我來說是反直覺的；我本來以為 coordinator 需要「理解」密碼學內容。
- **Nonce 重複使用是安全漏洞：** 在 FROST 簽名中，nonce 重複使用會洩漏私鑰份額。AI 的設計用 `UNIQUE constraint`（防止資料庫層面的重複插入）加上應用層在聚合後刪除 nonce 記錄，做到縱深防禦。這兩層保護都有其必要，光靠其中一層都不夠。

---

### UI 設計（fr-003）— 2026-04-21

**問題：** 確認 UI 結構確實正確反映了協議——特別是「逐步觸發」的需求（每個 Node × Round 有各自的按鈕）是否有被捕捉到。

**AI 互動：** 我請設計 agent 先產 ASCII wireframe，讓我可以先 review。

**為什麼先做 ASCII：** 審查 wireframe 需要 2 分鐘。審查 HTML mockup 再要求結構性重工花的時間長得多。用低保真格式前置版面審查，整體上節省時間。

**我在 wireframe 中審查的內容：**
- 三頁籤結構：DKG | Wallets | Signing
- DKG 頁籤：確認每個 node 每個 round 有各自的觸發按鈕（不是單一的「執行 DKG」按鈕），這是規格對 demo 的要求
- Wallet 頁籤：地址顯示及衍生路徑
- Signing 頁籤：訊息輸入、每個 node 的 round 觸發器、聚合簽名輸出

**我確認 wireframe 版面後：** agent 產出了完整的 HTML mockup（使用 Tailwind CSS），以及記錄調色盤、字體排印和元件模式的設計指南，供前端 agent 遵循。

---

## 後端 API 基礎建設（fr-007）— 2026-04-22

**問題：** 建立 Coordinator 與 TSS Node 的 HTTP Server 基礎架構，為後續的密碼學邏輯 PRD（DKG、簽名）提供穩定且可擴展的運行殼層（Shell）。

**AI 互動：** 派遣後端 Agent 實作 Axum Server。我明確要求所有 Handler 採 **Interface Stubbing (501 Not Implemented)**，確保在填入業務邏輯前，基礎設施層已達到穩定狀態。

**關鍵技術決策與審查：**

### 1. 語義化錯誤建模 (Semantic Error Modeling)
- **技術選型**：棄用 `anyhow`，堅持使用 `snafu` 進行強型別錯誤定義。
- **決策理由**：`anyhow` 會抹除型別資訊，導致無法根據 Error Variant 精準映射 HTTP Status Code（如 400 vs 409）。
- **雙層契約設計**：
    - **Node-to-Coordinator**：Node 報錯經由 Coordinator 邊界翻譯為 `NodeError` 並回傳 `502 Bad Gateway`。這確保了服務邊界清晰，不直接向前端暴露 Node 內部的實作細節。
    - **Coordinator-to-Frontend**：定義 **Machine-readable Error Codes**（如 `DKG_NOT_COMPLETE`），讓前端能程式化判斷錯誤類型，而非依賴不穩定的 Message 字串解析。

### 2. 建置密封性 (Build Hermeticity) 與 Migration 調整
- **架構修正**：將原本散落在根目錄的 Migration 文件移入各自 Crate 內部（`./migrations`）。
- **判斷與效益**：這確保了每個 Crate 是 **自包含 (Self-contained)** 的。這對於 `sqlx::migrate!` 這類編譯時巨集 (Compile-time macro) 至關重要，能避免因不同調度路徑或 Docker 多階段編譯導致的建置失敗。
- **安全邊界**：本系統刻意不設計 Shared Schema，透過資料庫物理隔離強制落實 **數據孤島 (Data Siloing)**，這是防範私鑰碎片在資料層級流動的底層物理障礙。

### 3. 動態依賴校正 — 避免「型別定義衝突」
- **風險發現**：在 Review 階段，我意識到若 fr-004（DKG）先於 fr-007 執行，將導致不同 Agent 對 `AppState`（如連線池、HTTP Client）產生不一致的型別推論與定義。
- **修正決策**：主動介入並更新 PRD 優先順序，強制要求基礎設施（The Shell）必須最先建立。
- **系統洞察**：任何會被多個 PRD 共用的 **「全局狀態型別」** 都必須由最早的 PRD 建立。一旦架構骨架確定，後續 Agent 只能在現有的型別框架下進行擴充，從根本上杜絕了因 Agent 幻覺導致的編譯期型別不匹配 (Type Mismatch)。

**審查結論：**
所有的 API 路由與資料庫 Migration 均通過我的手動校核。雖然目前 handler 皆為 stub，但強型別的錯誤處理與自包含的建置結構，為後續硬核的密碼學實作提供了極高的 **開發者體驗 (DX)** 與安全保障。

---

## 後端 DKG 協議實作（fr-004）— 2026-04-22

**問題：** 實作 FROST 門檻簽名的核心機制——分散式金鑰生成（DKG）。兩個節點必須透過三輪協議協作產生共享金鑰對，且在物理層面保證私鑰從不以完整形式存在。這是整個系統「去中心化安全」的根基。

**AI 互動：** 派遣後端 Agent 執行。由於涉及核心密碼學邏輯，此需要 review。

- **審查策略**：我並非僅檢查代碼是否通過編譯，而是對照 FROST 論文流程與 `frost-ed25519` 的文檔，對 API 的調用順序、狀態流轉與資料隔離邊界進行逐行審查。

**關鍵設計決策與安全審查：**
1. **協議原子性與狀態守衛**：
    - 嚴格落實三輪流程：R1 (Commitment) → R2 (Secret Sharing) → R3 (Verification)。
    - **狀態鎖定**：Coordinator 在每一輪皆實作前置條件檢查（必須雙方皆完成前一輪）。這種 **強順序性約束 (Strict Ordering Constraint)** 確保了協議不會因網路異常或併發請求而進入非法狀態。
2. **私鑰碎片的物理隔離 (Security Invariant)**：
    - 通過代碼走讀確認 `KeyPackage`（私鑰份額）僅於 Node 內部的資料庫持久化。
    - **攔截 AI 幻覺**：在審查中攔截了 Agent 試圖將敏感包裝 (Packages) 寫入廣播 Log 的行為，強制要求 Coordinator 僅能處理 **不具備私鑰推導性 (Opaque)** 的公開資料。
3. **冪等性 (Idempotency) 與重複執行防護**：
    - 確保「每個節點、每一輪次」具備單次執行屬性。這不僅是為了資料一致性，更是為了防止密碼學隨機數在重複插入中導致的安全漏洞。

### 核心技術解構與協議分析 (Protocol Insights)

在實作過程中，我針對 FROST 協議與 Rust 運行時的結合進行了深度的技術校準：

- **非同步環境下的隨機數安全 (Async Safety)**：
    - 發現 `rand::thread_rng()` 具備 `!Send` 屬性，無法於 Axum 的非同步 Handler 中安全跨執行緒傳遞。
    - **決策**：改採 `rand::rngs::OsRng`。這不僅解決了編譯期錯誤，更因其直接調用系統熵源，提升了密碼學種子的不可預測性。
- **識別碼 (Identifier) 的確定性衍生**：
    - 放棄手動分配節點 ID，改採 `Identifier::derive()` 根據節點標籤確定性地生成識別碼。這簡化了分散式環境下的節點管理，並確信了在重啟或擴展時 ID 的一致性。
- **Solana 格式相容性實作**：
    - 深入分析 `VerifyingKey::serialize()`。其輸出的 32-byte compressed Edwards Y 座標與 Solana 底層的公鑰格式完全相容。
    - **成果**：透過 Base58 編碼，我們成功在 DKG 完成瞬間即推導出合法的 Solana 帳戶地址，實現了協議層與鏈層的無縫對接。

**審查結論：**
透過對 DKG 三輪流程的「分解式審查」，我確保了密碼學邏輯的正確性不僅體現在「公式」上，更落實在「系統工程」的安全隔離與狀態機轉換中。

---

### 前端 DKG 與錢包介面（fr-008）— 2026-04-22

**問題：** 需要把 fr-003 的 UI 設計和 fr-002 的 API 合約落地為可互動的前端介面。DKG 和錢包是使用者操作的前兩步，必須能逐步觸發每個 Node × Round。

**AI 互動：** 派前端 agent，給它 HTML mockup、設計指南和 API 合約作為參考。這個 PRD 設為 self-review（AI 自審），因為前端配線屬於我在規劃階段判定的「低風險、可信任 AI 自審」類別。

**AI 產出的技術亮點：**
- **DKG 互動面板**：實作了 6 段式的進度管理。透過 **客戶端護欄 (Client-side Guardrails)**，確保 Node × Round 的觸發順序嚴格遵循 FROST 協議規範（例如：Round 2 必須在所有節點完成 Round 1 後才解鎖）。
- **連線狀態監測器**：AI 主動實作了系統連線指示燈。這大幅提升了 **開發者體驗 (DX)**，讓我們在開發過程中能即時判斷後端容器的存活狀態，而非盲目檢查 Console 報錯。
- **類型安全的 API Client**：透過 TypeScript 介面完整封裝了 Coordinator 的 15 個端點，確保了資料流的透明度。

**關鍵觀察與調整：**
1. **雙重防禦機制**：
    - DKG 的操作限制不僅在前端 `disabled`，後端 Coordinator 同樣具備 `409 Conflict` 的邏輯校驗。這種 **縱深防禦 (Defense in Depth)** 的思路確保了即便繞過 UI，協議安全性依然穩固。
2. **現代棧技術嗅覺 (Tailwind v4)**：
    - Agent 自動採用了 Tailwind v4 的 **CSS-first 配置模式**，將樣式 Token 直接整合進 `globals.css`。這證明了 AI 能根據最新的框架規格進行實作，避免了過時配置 (Legacy config) 的負擔。
3. **客戶端狀態鎖（Client-side Locking）**：
    - DKG round 觸發後，`executingRound` state 立即鎖定對應的 `"node-a-2"` key，按鈕進入 disabled + spinner 狀態，直到請求 settle 才解鎖。這防止了使用者在輪詢間隔中多次點擊導致的 Race Condition，與後端的 `ROUND_ALREADY_COMPLETE` 409 形成前後端雙層防護。
4. **連線指示燈的實作選擇**：
    - 指示燈複用 5 秒 polling 的 `getDkgStatus()` 請求結果，成功為綠、任何 exception 為紅，不另開獨立的 `/health` 請求。取捨是若只有這支 API 異常而其他端點正常，會誤報後端離線，但 demo 規模下可接受。

**審查結論：**
透過分級審查策略（主抓 API 契約，放權 UI 組件），我在極短時間內完成了從 Mockup 到可互動系統的轉化。目前 DKG 流程已可在前端完整跑通，並成功對接後端的資料庫狀態。

---

## 後端錢包衍生與簽名實作（fr-005 + fr-006）— 2026-04-23

**問題：** 實作系統最關鍵的閉環：(1) 從 DKG 共享公鑰透過 **Non-hardened Edwards Derivation** 衍生 Solana 地址；(2) 協作產生子金鑰簽名份額，完成 FROST 聚合簽名並廣播至 Solana Devnet。

**AI 互動：** 採取 **「策略性 Context 綁定 (Strategic Context Bundling)」** 策略。
- **決策**：將 fr-005 (衍生) 與 fr-006 (簽名) 合併派發給同一位後端 Agent。
- **理由**：兩者共享高度相似的密碼學 Context 與相同的 Crate 環境，且簽名邏輯直接依賴衍生的子份額模組。合併派發能最大化 Agent 的推論連貫性並節省啟動成本。

**關鍵觀察與安全邊界核查：**
- **非硬化衍生 (Non-hardened) 的優勢**：確認了 Coordinator 僅需公開資料即可獨立計算錢包地址，無需與 Node 通訊。而 Node 在簽名時能各自衍生對應的子私鑰份額，數學上確保了 **「分離計算，最終一致」**。
- **事務一致性與防禦縱深**：
    - **單一訊息承諾 (Message Commitment)**：Transaction Message 僅由 Coordinator 生成一次並持久化，確保兩個 Node 簽署的是完全一致的二進位資料。
    - **Nonce 的一次性保障**：落實 Nonce 使用即刻刪除的機制，防止密碼學重放攻擊。
- **交易生命週期管理**：
    - **決策**：棄用 `send_and_confirm_transaction`，改採 `send_transaction` 配合背景 **非同步輪詢 (Async Polling)**。
    - **理由**：FROST 產出的簽章是針對特定 Message 預先生成的，任何 SDK 內部的自動重新簽署嘗試都會導致交易失效。這展現了對 TSS 簽章「不可更改性」的深度理解。

**審查結論：**
透過 fr-005 與 fr-006 的整合實作，我驗證了 TSS 錢包在鏈上的 **透明性 (Transparency)**：產出的簽章與原生 Ed25519 完全一致。這代表我們的系統在不改變 Solana 鏈上行為的前提下，大幅提升了私鑰管理的安全性。

---

### 前端簽名與交易介面（fr-009）— 2026-04-23

**問題：** 實作 FROST 簽名流程的前端介面。使用者需要能建立簽名請求、逐步觸發每個 Node 的兩輪簽名、聚合並廣播到 Solana Devnet，以及追蹤交易狀態直到確認。這是三個核心流程中互動最複雜的一個。（與後端 fr-005+fr-006 平行進行，前端依賴 fr-002 API 合約，不依賴後端業務邏輯實作。）

**AI 互動：** 派前端 agent，給它 HTML mockup 的 Signing tab、設計指南和 API 合約。設為 self-review，跟 fr-008 一樣的理由——前端配線屬於低風險類別。fr-008 已經建好 tab navigation、設計系統和 API client 基礎，這次只需要填入 Signing tab 的完整內容。
這個 PRD 執行順利，self-review 策略在此得到驗證——fr-008 預先提升的 selectedWalletIndex state 讓 fr-009 無需重構。Build 乾淨，zero TypeScript errors。

---

### Docker Compose 整合與測試（fr-010 & fr-011）— 2026-04-23

**問題：** 所有服務都個別開發完成，但從未在 Docker 環境中一起運行過。需要確保 `docker compose up` 能啟動完整系統，並撰寫自動化測試驗證三大核心流程端對端跑通。

**AI 互動：** 派後端 agent 修正 Docker 設定並撰寫測試，Docker 設定直接影響評審能不能一鍵跑起系統，需要 review。

---

### Docker 環境除錯與 Integration Test 驗證 — 2026-04-23

**問題：** 在本機實際運行 Docker 環境時，遇到三個連鎖問題需要排查。

**問題排查過程：**

1. **Integration test 卡住**：`./tests/integration-test.sh` 在 `POST /api/dkg/start` 卡住。透過 `curl -s /api/dkg/status` 手動確認 DKG 其實第一次已經跑完了（status: complete），第二次 start 碰到已存在的 session 造成 hang。解法：`docker compose down -v` 清掉資料重跑。

2. **PostgreSQL 18 volume 路徑 breaking change**：清掉 volume 後重啟，postgres 報 unhealthy。查 log 發現 PG 18+ 不再接受 mount 在 `/var/lib/postgresql/data`，改為 `/var/lib/postgresql` 由 PG 自行管理子目錄。第一次跑因為 volume 是空的所以沒事，`down -v` 後就爆了。這提醒我 AI 產出的配置仍需要實際 runtime 驗證。

3. **Frontend healthcheck 失敗**：postgres 修好後 frontend 仍 unhealthy。AI 建議用 wget，確認後發現根本原因是 `node:slim` 既無 curl 也無 wget。改用 Node.js 內建 `http` 模組做 healthcheck，不依賴額外工具。

**AI 診斷結論：**
- Frontend healthcheck 根本原因：node:slim 既無 curl 也無 wget，AI 建議的 wget 方案同樣無效。改用 Node.js 內建 http 模組是最 robust 的解法，runtime image 必然有 node
- 三組 healthcheck 工具的對應：backend 用 curl（Dockerfile 有裝）、postgres 用 pg_isready（官方內建）、frontend 用 node http（runtime 自帶）

**思考判斷：** 
- 否決 AI 的 wget 建議，採用 node http 方案
- AI 的訓練資料有截止日期。PG 18 的 volume 路徑 breaking change、`node:slim` 缺少 curl 這類近期變化，AI 不一定知道。任何 Docker 配置都必須在真實環境中跑過才能信任。
- 殘留風險：healthcheck 打 /，若首頁未來改成 redirect 或非 200 回應會誤判 unhealthy。可補專用 /health 端點，但不是現在的blocker。

**Integration test 結果：** 三個問題修完後，22/22 tests passed。UI 手動測試 DKG → 錢包衍生 → 簽名流程全部跑通。

---

### 完整系統 Code Review + 功能面驗收 — 2026-04-24

**問題：** 系統開發進入收尾階段（fr-001 ~ fr-010）。為了確保交付品質並徹底掌握系統細節，必須進行全代碼走讀（Code Walkthrough），並針對原始規格書（Assignment Spec）進行最終的風險評核。

**AI 互動：** 採取「配對審查 (Pair Review)」模式。我要求 AI 扮演技術導師，由底層 Schema 往上逐層解構模組邏輯。
- **目的**：這不是委派 AI 進行自動化審查，而是由我主動發起提問，建立對系統邊界（System Boundaries）的深度理解，確保每一項技術決策皆符合設計初衷。

**技術架構解構：**

1. **資料庫安全設計**：驗證了三個獨立資料庫的物理隔離。`coordinator_db` 僅保存公開元數據與聚合公鑰，`Secret Share` 則嚴格禁錮於各 Node DB。即便 Coordinator 遭遇入侵，攻擊者也無法獲取任何私鑰碎片。

2. **兩個 derivation.rs 的分工**：Coordinator 僅持有公鑰衍生邏輯（產生地址），Node 端則持有私鑰碎片衍生邏輯。這種「權力下放」的實作確保了 Coordinator 在物理上完全不具備觸碰密碼學私鑰的可能。

3. **金鑰生命週期與單向依賴**：釐清了 DKG 是「靜態地基 (Static Foundation)」，而簽名則是「動態衍生 (Ephemeral Derivation)」。兩者透過單向依賴確保了簽名過程的異常不會回溯污染根密鑰碎片的安全性。

4. **Nonce 重用防護機制**：審查了 `signing_nonces` 表的 `UNIQUE constraint` 配合 Round 2 完成後的原子化刪除邏輯。這項 **縱深防禦 (Defense in Depth)** 徹底消弭了因 Nonce 重用導致的私鑰洩漏風險。

5. **send_transaction vs send_and_confirm_transaction**：後者會嘗試重新簽名，但 FROST 預簽交易沒有完整私鑰可以重簽，所以只能用前者 + 自己寫背景輪詢。了解到 `send_and_confirm_transaction` 是給持有完整私鑰的一般 Solana 應用用的便利工具。

**規格對齊與邊界風險評核：**

在對照 Assignment 原文後，我識別出兩項潛在風險並做出決策：

- **UX 決策：簽名請求列表**：
  - **觀察**：原規格傾向僅顯示「待處理請求」，但我選擇顯示完整歷史紀錄並以狀態標籤（Status Tags）區分。
  - **判斷**：在 Demo 情境下，保留完整生命週期的透明度能提供 Reviewer 更佳的追蹤體驗，優於單純的過濾顯示。

- **核心修正：強化 Solana 交易確認邏輯**：
  - **識別風險**：原實作僅依賴 `status.err.is_none()`，這會導致交易在 `processed` 階段（尚未達成叢集共識）即被標記為完成。
  - **補強策略**：
    - 重新定義 **「確認」** 語義：嚴格遵循規格書，僅當 Solana 網路回報為 `Confirmed` 或 `Finalized` 時才更新狀態。
    - **重構錯誤攔截順序**：優化輪詢邏輯，優先攔截鏈上錯誤（立即回傳 Failed），再進行層級判定，確保了系統狀態與區塊鏈共識的同步。

**驗收結論：**
透過這次全代碼走讀與針對性修正，我將「功能完成」提升到了「規格嚴謹」的高度。目前系統已通過最終測試鏈路，確保在 `docker compose up` 後能提供符合區塊鏈最終性（Finality）預期的操作體驗。

---

### 靜態分析輔助：用不同 AI 審視 codebase — 2026-04-25

**場景：** 功能跑通後，用 Codex 做一輪獨立審查，讓沒有上下文偏見的視角重新掃描整份程式碼。

**AI 互動：** 把完整 codebase 丟給 Codex 做靜態掃描，再把 Codex 的分析拿來和 Claude 交叉確認。

**Codex 回報了 4 個問題：**
1. `create_session` 和 `create_signing_request` 缺少 DB transaction 包裝
2. Idempotency：節點在 coordinator 記錄前就存了狀態，retry 會卡住
3. `integration-test.sh` 沒有等 DKG 完成就繼續執行（race condition）
4. Frontend `useEffect` 有 ESLint error（`react-hooks/set-state-in-effect`）

**我的處理方式不是全部照單全收。** 把 Codex 的分析拿去和 Claude 交叉確認後，發現問題 1 和 3 其實程式碼裡已經處理了——`create_session` 確實有用 `pool.begin()` 包 transaction，`integration-test.sh` 也有 `run_dkg_round_if_pending()` 做等待邏輯。Codex 看到的是舊版結構，或判斷有誤。

問題 2（Idempotency）是真實的架構缺口：節點在 coordinator commit 前就持久化自己的狀態，重試路徑會卡死。在 demo 規模下不會觸發，但如果是生產服務就是個問題。我選擇誠實記錄這個缺口而不是強行補丁——修正它需要在節點層加冪等鍵或引入分散式事務語意，不是一個小改動。

問題 4（ESLint error）是真實的，見下一段。

**這段經驗的收穫：** 用第二個 AI 交叉確認是有效的。Codex 找到了一個真實問題 + 一個值得誠實記錄的架構弱點；另外兩個則因為我有足夠的程式碼理解能力，沒有被誤導。AI 的輸出需要自己去驗證，不能當作 ground truth。

---

### Frontend Build 問題排查 — 2026-04-25

**場景：** 功能面驗收完成、準備提交前，跑 `next build` 時遇到兩個非業務邏輯的執行問題。

**問題一：ESLint build error（`react-hooks/set-state-in-effect`）**

`page.tsx` 的 polling 邏輯把 `setState` 放在 `useEffect` 外的 callback，lint rule 判定為潛在的 unmount 後 setState 風險。我把錯誤貼給 Claude，AI 的修法是把 async poll 邏輯整個移進 `useEffect`，加 `cancelled` flag 防止 unmount 後的 setState。這個改法我理解：`cancelled` flag 是處理 async race condition 的標準 React pattern，不只是為了過 lint。

**問題二：Google Fonts 網路依賴**

`next/font/google` 在 build time 會去 Google CDN 抓字型。本地沒問題，但在 CI 或網路限制環境下會 fail，評審環境有這個風險。`package.json` 裡其實已經有 `geist` npm 套件，只要改 import 就能變成本地字型、零網路依賴：

```tsx
// 改前
import { Geist } from "next/font/google";
// 改後
import { GeistSans } from "geist/font/sans";
```

排查過程是：跑到錯誤 → 截出錯誤訊息 → 問 AI「這個 next/font/google 在什麼情況下會 fail」→ 理解原因後確認 `geist` 套件確實已存在 → 修改。AI 的作用是幫我快速定位問題根因，實際的驗證和決策是我做的。