<!-- markdownlint-disable MD033 MD041 -->

<br/>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo/akagi-logo-dark.png">
    <img alt="Akagi" src="assets/logo/akagi-logo-light.png" width="55%">
  </picture>
</p>

<p align="center">
  <i>「死ねば助かるのに………」 - 赤木しげる</i>
  <br/><br/>
  專為 <b>雀魂</b>、<b>天鳳</b> 等日麻平台打造的即時 AI 對局輔助系統。<br/>
  Akagi V3：以單一 Rust 執行檔與 Tauri 架構徹底重構前作
  <a href="https://github.com/shinkuan/Akagi/tree/v2">Akagi</a> 與
  <a href="https://github.com/Xe-Persistent/Akagi-NG">AkagiNG</a>。
  <br/><br/>
  <b>本版本為 <a href="https://github.com/seriaati">seriaati</a> 維護之 Akagi 分支版本。</b>
  原始專案由 <a href="https://github.com/shinkuan">shinkuan</a> 開發：
  <a href="https://github.com/shinkuan/Akagi">shinkuan/Akagi</a>。
  各項功能擴充與改進詳見 <a href="#分支特色與改進">分支特色與改進</a>。
  <br/><br/>
  <a href="https://github.com/seriaati/Akagi/issues">回報 Bug</a>
  ·
  <a href="https://github.com/seriaati/Akagi/issues">功能建議</a>
  ·
  <a href="https://deepwiki.com/seriaati/Akagi">DeepWiki</a>
</p>

<p align="center">
  <a href="https://github.com/seriaati/Akagi/stargazers"><img src="https://img.shields.io/github/stars/seriaati/Akagi?logo=github" alt="GitHub stars" /></a>
  <a href="https://github.com/seriaati/Akagi/releases"><img src="https://img.shields.io/github/v/release/seriaati/Akagi?label=release&logo=github&include_prereleases" alt="Latest release" /></a>
  <a href="https://github.com/seriaati/Akagi/issues"><img src="https://img.shields.io/github/issues/seriaati/Akagi?logo=github" alt="Open issues" /></a>
  <a href="./LICENSE.txt"><img src="https://img.shields.io/badge/license-Apache%202.0-blue?logo=apache" alt="License: Apache-2.0" /></a>
  <a href="https://github.com/seriaati/Akagi/actions/workflows/release.yml"><img src="https://img.shields.io/github/actions/workflow/status/seriaati/Akagi/release.yml?branch=v3&logo=githubactions&label=build" alt="Build status" /></a>
  <a href="https://deepwiki.com/seriaati/Akagi"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki" /></a>
</p>

<p align="center">
  其他分支：
</p>

<p align="center">
  <a href="https://github.com/shinkuan/Akagi/tree/v2"><img src="https://img.shields.io/badge/Akagi-v2_(Python)-blue?logo=github" alt="v2 branch" /></a>
  <a href="https://github.com/Xe-Persistent/Akagi-NG"><img src="https://img.shields.io/badge/Akagi-NG_(Electron)-blue?logo=github" alt="NG branch" /></a>
</p>

<p align="center">
  <a href="./README.md">English</a>
  ·
  <b>繁體中文</b>
  ·
  <a href="./README.zh-CN.md">简体中文</a>
</p>

## 簡介

> 本專案的目的，是讓你能即時掌握自己在麻將對戰中的表現並從中學習。
> 本專案僅供 **教育用途**。作者不對使用者的任何行為負責。
> 遊戲開發商與發行商保留對違反其服務條款者採取行動的權利；
> 任何後果（如帳號停權等）皆由使用者自行承擔。

Akagi 透過本機 Proxy 或內建瀏覽器監看你在雀魂 / 天鳳的對局，
鏡射遊戲狀態，並在可拖曳的 HUD 中顯示 **向聽**、**待牌**、
**和牌率**、**聽牌率**、**對各家放銃風險**，以及
**推薦切牌**。執行檔本身就內建一個 AI 模型 —— 不需要安裝任何東西 ——
它的建議會在每巡顯示；若想要更強的託管模型，可以把它指向
雲端推論 API。

## 分支特色與改進

本專案為 [shinkuan/Akagi](https://github.com/shinkuan/Akagi)（`v3` 分支）之衍生版本，在此基礎上進一步擴充功能並修復各項問題：

### 新增功能

- **雀魂自動連續開局（Auto-rematch）** — 託管代打對局結束後，自動確認結算提示「再來一場」並自動略過獎勵結算畫面，實現全自動無縫連戰。
- **雀魂全角色飾品本地解鎖** — 選用的 Proxy 流量重寫功能，在本地端直接解鎖全角色、裝扮與飾品（純本地客戶端效果，預設關閉，需開啟 Proxy）。
- **混合封包擷取模式（Hybrid Capture）** — 由 Akagi 啟動的專用 Chromium 自動綁定 MITM Proxy，免除安裝系統根憑證與設定全域代理的麻煩，同時完美兼顧自動託管代打與裝扮解鎖等 Proxy 覆寫功能。
- **手牌深度進階分析**
  - **聽牌巡目預測**：估算聽牌所需摸牌巡目（中位數及 80% 累積機率），並計算流局前的預期聽牌機率。
  - **待牌打點與役種明細**：即時列出各待牌榮和時的番種與預估打點，並於 HUD 新增聽牌打點看板。
  - **各家動態推測**：估算各家對手可能的聽牌待牌範圍，並提供針對各家的現物與安牌推薦。
- **豐富 HUD 資訊面板** — 即時局況歷史記錄、聽牌預估巡目看板（聽牌後自動無縫切換為待牌資訊）、頂部狀態列顯示目前場況／順位／剩餘摸牌數，且各家面板直接標示對手與本家玩家暱稱。
- **Discord 個人動態（Rich Presence）** — 活動名稱即時顯示當前遊玩之麻將平台，並自動配對介面語言。
- **設定隨改隨存** — 所有設定修改即時生效並自動儲存，免除手動保存。
- **內建更新對接** — 更新檢查器直接對接本分支的最新 Release 發佈版本。

### 問題修復

- **對局防卡死保護**：若外部或雲端 Bot 在局中異常崩潰，系統將自動切換為備用策略出牌，防止對局超時停滯。
- **Linux 體驗優化**：強制使用 XWayland 運行，確保 HUD 懸浮窗置頂不失效；終端機可直接按下 Ctrl+C 優雅結束程式。
- **代打畫面防閃爍**：改為擷取完整畫面視口，徹底解決自動代打點擊時遊戲視窗頻繁閃爍的問題。
- **斷線重連歷程留存**：網路斷線重連後，仍完整保留該場對局已進行的前幾局歷史記錄。
- **UI 提示與存檔狀態平穩化**：解決設定自動存檔時的狀態反覆彈跳；常駐提示訊息（Sticky toasts）在流程處理完畢後自動淡出；過濾重複的設定提示。
- **渲染效能優化與記憶體流失修復**：為麻將牌面渲染（mahgen）加入節流與快取機制，大幅降低效能開銷；修復 DOM Observer 與即時日誌轉發器的記憶體流失（Memory Leak）問題。

## 截圖

<img width="2559" height="1439" alt="image" src="https://github.com/user-attachments/assets/da9e7cce-d8ef-4e6e-807b-f6f54013cf22" />

https://github.com/user-attachments/assets/42812e85-ccf0-49fd-b825-adbb5b7b58b0

https://github.com/user-attachments/assets/2ce7cb71-8b25-4895-a12b-0a638665dcab

https://github.com/user-attachments/assets/d5bc6ff6-6560-4365-ae55-660c9a522790

---

## 目錄

**使用者**
- [分支特色與改進](#分支特色與改進)
- [功能](#功能)
- [支援的平台](#支援的平台)
- [快速開始](#快速開始)
- [Bots](#bots)
- [對局歷史](#對局歷史)
- [紀錄與診斷](#紀錄與診斷)
- [下載鏡像](#下載鏡像)
- [疑難排解](#疑難排解)
- [Roadmap](#roadmap)

**開發者**
- [架構](#架構)
- [技術堆疊](#技術堆疊)
- [專案結構](#專案結構)
- [mjai Bot 外掛介面](#mjai-bot-外掛介面)
- [從原始碼建置](#從原始碼建置)
- [測試](#測試)
- [Releases 與 CI](#releases-與-ci)
- [參考資料](#參考資料)
- [授權與致謝](#授權與致謝)
- [鳴謝](#鳴謝)

---

## 功能

- **即時 HUD** — 向聽、待牌、和牌率、聽牌率、對各家放銃
  風險、推薦的進攻 / 防守切牌。可拖曳、可縮放的UI佈局。
- **兩種抓包模式**
  - **MITM proxy**（預設） — 系統層級；需一次性的 CA 信任。
  - **Chromium** — 由 Akagi 啟動受控的 Chromium 系列瀏覽器，
    透過 Chrome DevTools Protocol 攔截 WebSocket 訊框。
    無需設定 proxy 或安裝憑證；直接在啟動的視窗中遊玩即可。
- **兩種 bot 後端**
  - **內建 bot**（預設） — 嵌在執行檔內的純 Rust 神經網路。
    不需要 Python、不需要下載、不需要設定；四麻與三麻都能直接開打。
  - **雲端推論**（選用） — 把每一次決策交給透過 HTTP 存取、
    **更強的託管模型**。內建模型仍保持載入作為自動後備，因此伺服器
    連不上時也不會讓對局卡住。金鑰可直接在應用程式內購買或兌換。

  兩者皆可依模式切換：`bot.active_4p` 與 `bot.active_3p`
  會依牌桌人數自動套用。
- **對局歷史** — 每場結束的對局會自動記錄。歷史頁籤顯示
  名次圓餅圖、可選計分規則的累積 PT 折線圖（雀魂段位 /
  天鳳段位 / 自訂 uma），以及細部統計（和牌率、放銃率、
  立直率、副露率、流局率、平均和牌 / 放銃點數、平均和牌
  巡目、役滿 / 流局滿貫次數）。
- **簡單的首次啟動設定** — 語言 → 平台 → 抓包模式 →
  CA 信任 / Chromium 選擇 → bot 設定 → 完成。
- **多語系** — English、日本語、繁體中文、简体中文。
  可在設定精靈或設定即時切換。
- **三麻** — 完整支援：AI分析、依模式 bot 路由、歷史統計、3p uma 表。
- **應用程式內更新** — 啟動時自動檢查新版本，也可在 *設定 → 更新*
  手動檢查；一鍵下載、原地更新並重新啟動。唯讀安裝環境（如 AppImage）
  會自動導向 Releases 頁面手動下載。

## 支援的平台

| 平台 | 四麻 | 三麻 | AutoPlay |
|---|:---:|:---:|:---:|
| **雀魂（Mahjong Soul / Majsoul）** | &check; | &check; | &check; |
| **天鳳（Tenhou）** | &check; | &check; | &check; |
| **Riichi City** | &check; | &check; | &check; |
| **Amatsuki** | （規劃中） | （規劃中） | &cross; |

---

## 快速開始

### A. 安裝官方 Release

Akagi 提供免安裝的可攜式壓縮檔（Portable ZIP）— 針對不同作業系統提供開箱即用的獨立資料夾。
從 [Releases](https://github.com/seriaati/Akagi/releases) 下載
對應作業系統的壓縮檔，解壓縮至任何具備寫入權限的目錄（例如
`~/Apps/` 或桌面），直接執行資料夾內的 `akagi` 即可。設定檔、日誌紀錄、
對局歷史、CA 憑證與 Bot 均保存在同一目錄旁，無論移轉、
備份或移除，僅需搬移、複製或刪除該資料夾即可完成。

| OS | 檔案 | 備註 |
|---|---|---|
| Windows | `akagi-<version>-windows-x64.zip` | x86_64。需要 WebView2(Win10 1803+ 與 Win11 已預載)。SmartScreen 會警告 — 點 *More info → Run anyway*。 |
| macOS | `akagi-<version>-macos-arm64.zip` | Apple Silicon。未簽章,解壓後執行一次 `xattr -cr <解壓後資料夾>`,或第一次右鍵 → *Open*。 |
| Linux | `akagi-<version>-linux-x64.zip` | 在 `ubuntu-22.04` 上建置(glibc 2.35+)。需要 WebKit2GTK 4.1(`apt install libwebkit2gtk-4.1-0` / `dnf install webkit2gtk4.1` / `pacman -S webkit2gtk-4.1`)。 |

首次啟動時會引導你完成語言、平台、抓包模式、bot 設定，
以及 CA 信任（僅 MITM 模式才需要）。沒有 bot 要安裝 ——
內建的那個本來就在。

### B. Chromium 模式（不需信任 CA）

最簡單的方式。完成設定後Akagi會自動尋找 Chrome / Edge / Brave / Chromium 然後以獨立的個人資料啟動瀏覽器，登入雀魂後即可開始遊玩。

透過 Chrome DevTools Protocol 攔截 — 不需系統 proxy、
不需憑證。

### C. MITM 模式

系統層級的 proxy，搭配位於 `./ca/` 的自簽根 CA：

1. 信任憑證
   `./ca/akagi-ca.crt`（或 `.cer` / `.pem` / `.der`）。
2. 將遊戲客戶端的流量導向 `127.0.0.1:23410`。
   健康檢查：`GET /ping` → `pong`。
3. Windows 上常用 [Proxifier](https://www.proxifier.com/)
   把指定應用程式導向 proxy。
4. **把 loopback 排除在重導之外。** `localhost`、`127.0.0.1`、`::1`
   一律走 Direct，不要經過 Akagi。

> [!IMPORTANT]
> 第 4 步不是可選的。遊戲會透過 loopback 跟自己通訊來處理內部事務，
> 而「比對遊戲程式、目標為任意 host」的重導規則會把這些 socket
> 一併掃進 Akagi。Akagi 會拒絕它們（紀錄裡會出現
> `refusing CONNECT to loopback` 警告），但遊戲仍可能出問題 —
> 所以請從源頭排除 loopback。
>
> Proxifier 的做法：**Profile → Proxification Rules**，啟用內建的
> **Localhost** 規則（Action: *Direct*），並把它拖到遊戲規則的**上面**。
> 順序很重要 — Proxifier 只採用第一條命中的規則，Localhost
> 規則排在遊戲規則下面就永遠不會生效。

---

## Bots

### 內建 bot

Akagi 內建一個 **純 Rust 的 bot**，它是兩種模式的預設值（`bot.active_4p = "akagi-native"`、
`bot.active_3p = "akagi-native3p"`），會出現在 **Bots** 頁籤最上方，
狀態永遠是「就緒」。

它是一個以行為複製（behavior cloning）訓練出來的小型神經
網路（權重直接嵌在執行檔內），因此棋力 **刻意保持在中等水準** ——
它是個合理的預設值，而不是頂尖引擎。

### 雲端推論

內建 bot 可以選擇把決策交給 **遠端推論伺服器**，而不是執行內嵌
的模型 —— 那是一個透過網路存取、更強的託管模型。內嵌的本機模型仍會
保持載入作為自動 **後備**：當伺服器連不上、被限流，或金鑰無效時，bot
會改用本機模型的著手，讓進行中的對局不會卡住。

#### 取得雲端推論金鑰

提供兩種途徑：

- **購買金鑰** — 於應用程式內直接購買授權。
- **兌換序號** — 使用預付兌換碼兌換新金鑰，或為既有金鑰延長使用期限。

#### 請求逾時設定

向雲端伺服器請求走子決策時設有 **請求逾時限制**（預設為 **3000 ms**，可於雲端推論設定中微調，有效範圍為 500～10000 ms）。若在時限內未能收到雲端回應，該巡將自動切換由本機離線模型即時接手，確保對局節奏順暢不卡死。
- 若網路環境延遲較高或雲端伺服器負載較大，可適度調高此數值。
- 若希望縮短等待時間儘快由本地接手，則可調低此數值。
- **注意事項**：宣告立直在同一巡內需連續發起兩次請求（宣告立直與打出立直牌），若逾時閾值設定過長，可能面臨超出遊戲每巡出牌倒數時間的風險。

### 依模式切換的 bot

`bot.active_4p` 與 `bot.active_3p` 互相獨立。Akagi 會在開
局時依牌桌人數選用對應的 bot。

除了這兩種後端之外，Akagi 也能以子行程執行 **外部 mjai bot**。
那是給開發者的擴充點，而不是任何人都得走的步驟 ——
請見 [mjai Bot 外掛介面](#mjai-bot-外掛介面)。

---

## 對局歷史

每一場乾淨結束的對局（產生了 `end_game` mjai 事件）都會
被持久化到 `<config_root>/history/`：

```
<config_root>/history/
├── index.jsonl              # 每行一筆 GameRecord（以 ULID 為 key）
└── games/
    └── <ulid>.mjai.jsonl    # 完整事件流的副本
```

中途斷線會在 buffer 中留下未完成的紀錄並被靜默丟棄 —
只有完整對局會落到磁碟。

前端 **History** 頁籤顯示：

- **名次圓餅圖** — 1/2/3/4 名分布（三麻只有 3 片）。
- **累積 PT 折線圖** — 可選擇計分規則：
  - **雀魂**：選擇 `場次`（銅 / 銀 / 金 / 玉 / 王座）與
    `段位`（初心 1 星 → 魂天）。
  - **天鳳**：選擇 `段位`（新人 → 天鳳位，共 21 階）。
  - **自訂**：直接編輯 uma 與段位獎金陣列。
  切換規則 / 段位會立即重繪 — 不需要 backend round-trip。
- **細部統計** — 和牌率、放銃率、立直率、副露率、
  流局率、平均和牌 / 放銃點數、平均和牌巡目、
  役滿 / 流局滿貫次數。
- **對局清單** — 可依平台 / 人數 / 東風或半莊 / 日期過濾。
  點選列即可看到最終排名與該局統計；垃圾桶圖示會同時
  刪除 index 條目與該局的 `.mjai.jsonl`。

PT 規則與過濾條件會持久化於 `localStorage`。Bridge 啟動
時從 backend 載入紀錄，並透過 `history-recorded` Tauri
事件保持同步。

數學細節、儲存 schema，以及如何新增平台 / 統計欄位 /
過濾維度請見 [`src/history/README.md`](./src/history/README.md)。

---

## 紀錄與診斷

每次 session 的紀錄會落在 `<log_dir>/<YYYYMMDD-HHMMSS>/`：

```
<log_dir>/<session>/
├── all.log                       # 所有 tracing 輸出彙整
├── <target>.log                  # 依模組過濾的紀錄
├── proxy.binlog                  # 原始 WS 二進位訊框
├── majsoul/<flow_id>.log         # 每條 WebSocket flow 的 JSON 紀錄
├── majsoul/<flow_id>.mjai.jsonl  # 每場對局的 mjai 事件流
└── inspector.jsonl               # Inspector 看到的訊框
```

前端 **Logs** 路由有兩個頁籤：

### Diagnostic

可過濾的應用程式紀錄。可依等級（trace / debug / info /
warn / error）與模組過濾。可即時 tail 或瀏覽過去的
session；點選列可看到原始結構化欄位與來源位置。
**Open Folder** 按鈕會在系統檔案管理員中開啟該 session
資料夾。

### Inspector

協定層級的訊框檢視器。共三類條目：

- **WS Frame** — 原始二進位（base64 截短）加上 bridge
  的初步解析結果。
- **MjaiEvent** — 流向 bot 的解碼後事件。
- **BotReaction** — bot 的回應，含 `meta` 欄位
  （信心度 / q-values / bot 想送出的任何資訊）。

訊框計數會顯示每個 WS 訊框產生了多少個 mjai 事件，
在排查 bot 或 bridge 問題時很有用。

---

## 下載鏡像

Akagi 執行期間會下載三類內容：應用程式更新與 bot 安裝來自
**GitHub Releases**，Chrome for Testing 來自 **Google CDN**。
在這些站點被封鎖或不穩定的地區（如中國大陸），可以在
設定 → **下載** 中控制 GitHub 下載的路線：

- **自動**（預設）— 先短逾時直連 GitHub，失敗後依序嘗試公共
  gh-proxy 式加速鏡像。
- **僅直連** — 從不使用鏡像。
- **鏡像優先** — 已知 GitHub 連不上時跳過直連等待。
- **自訂鏡像前綴** — 公共加速站時常失效；填一個你所在地區
  可用的（如 `https://gh-proxy.com`），它會優先於內建清單使用。

Chrome for Testing 不需設定：Google 端點連不上時會自動改用
[npmmirror](https://registry.npmmirror.com) 鏡像下載。

鏡像屬於第三方，因此完整性靠簽章而非傳輸通道保證：每個
release zip 都以 [minisign](https://jedisct1.github.io/minisign/)
簽章，經鏡像下載後應用程式會驗證簽章，驗證失敗（或舊的未簽章
版本需要經過鏡像）時會拒絕安裝。手動驗證方式：

```sh
minisign -Vm akagi-<version>-<platform>.zip -p minisign.pub
```

其中 [`minisign.pub`](./minisign.pub) 位於儲存庫根目錄。

---

## 疑難排解

> [!TIP]
> 重現問題後，存下 `<log_dir>/<session>/` 整個 session
> 資料夾 — 內含應用紀錄、原始訊框、mjai 事件、bot meta，
> 是回報有用 bug 報告所需的所有資訊。

- **MITM 模式抓不到封包。** 確認 `./ca/akagi-ca.crt`
  已在系統憑證庫中信任。確認 proxy 已啟動：
  `curl http://127.0.0.1:23410/ping` 應回應 `pong`。
  確認你的 proxy 重導工具（Proxifier / 系統 proxy）
  正把遊戲客戶端送到正確的 host:port。
- **MITM 模式下遊戲卡在加載畫面。** 多半是重導工具把遊戲的 loopback
  流量也送進了 proxy。在紀錄裡找 `refusing CONNECT to loopback`，
  然後排除 `localhost`、`127.0.0.1`、`::1` — 見上方 MITM 設定第 4 步。
- **Chromium 模式擷取不到封包。** 系統自動偵測未能尋獲瀏覽器。請至「設定」或於 `config.toml` 中手動指定 `capture.chromium.executable` 的執行檔路徑。若瀏覽器已啟動卻沒有遊戲封包流入，請檢查 `--remote-debugging-port` 是否遭其他瀏覽器外掛擴充功能攔截或衝突。
- **Chromium 視窗未關閉時重新啟動 Akagi。** 由於同一份使用者設定檔（Profile）無法供多個程序同時使用，Akagi 會先嘗試關閉現存瀏覽器，再重新啟動乾淨的獨立視窗。此操作會保留登入狀態，雀魂重整後亦可正常連回進行中的對局。若背景殘留的瀏覽器無法自動退出（提示「couldn't terminate the browser already using profile …」），請手動在工作管理員中關閉該瀏覽器程序後點擊「Restart（重啟）」。請注意：不支援同時開啟多個 Akagi 實例存取同一份 Profile。
- **天鳳對局途中斷線重連。** 天鳳的重連協議推送的是當前牌桌狀態的快照（Snapshot）而非完整事件重放，因此 Akagi 無法還原斷線當前小局的推演過程。此時系統會彈出「rejoined mid-hand」通知，並暫停當前小局的手牌分析與自動託管；自下一小局起便會自動恢復正常追蹤。透過此方式重連的殘缺對局不會寫入「對局歷史」，以避免因缺失前期小局資料而導致個人戰績統計失真。
- **Bot 在對局中途崩潰。** 在「Inspector」檢視頁籤中可查看 Bot 異常退出前收到的最後一個訊框（Frame）；回報問題時請將其附於 Bug Report 中。
- **三麻模式使用了錯誤的 Bot。** 請至「設定 → Bot」檢查 `bot.active_3p` 設定項 — 四麻與三麻的 Bot 設定各自獨立。
- **下載更新 / 安裝 Bot / 下載 Chrome 時發生逾時（中國大陸等網路受限地區）。** 參見[下載鏡像](#下載鏡像) — 將「設定 → 下載」切換為「鏡像優先」，或於「自訂鏡像前綴」中填寫確認可用的加速站點。
- **如何尋求協助與回報問題？** 請前往 [GitHub Issues](https://github.com/seriaati/Akagi/issues) 提交 Bug 回報或功能改進建議。

---

## Roadmap

alpha.8 已完成：

- [x] 三麻 — 完整流程
- [x] 天鳳 bridge（僅觀戰）
- [x] Riichi City bridge（僅 MITM — 原生用戶端；僅觀戰）
- [x] 對局歷史持久化 + History 頁籤（名次圓餅 / PT 圖 / 統計）
- [x] 紀錄檢視（Diagnostic + Inspector）
- [x] i18n：en / ja / zh-TW / zh-CN，含設定精靈語言選擇
- [x] 從 GitHub release 或本機 ZIP 檔案安裝 bot
- [x] Chromium 抓包模式（不需信任 CA）
- [x] **自訂主題**（前端 theming hook）
- [x] **AutoPlay**（支援雀魂與天鳳；由 bot 自主控制牌桌）

規劃中：

- [ ] **Amatsuki** 平台支援
- [ ] **前端打磨** — 牌型佈局、動畫、無障礙

詳細的 bug 追蹤請至
[GitHub Issues](https://github.com/seriaati/Akagi/issues)。

---
---

## 架構

單一 Rust 執行檔。各子系統只持有自己的 bus handle，
彼此互不擁有。
[`src/event_bus.rs`](./src/event_bus.rs) 是所有 channel
類型的單一真相來源。

```
                ┌────────────────────────┐
   遊戲客戶端 ─│  capture (mitm | cdp)  │── CA 位於 ./ca（僅 mitm）
   WebSocket   └─────────┬──────────────┘
                          ▼
                ┌────────────────────────┐
                │  bridge::<platform>    │   wire bytes → MjaiEvent
                └─────────┬──────────────┘
                          ▼ MjaiBus
       ┌──────────────────┼──────────────────┐
       ▼                  ▼                  ▼
  game_state::tracker   bot::manager     ipc forwarder
       │                  │                  │
       ▼ PostBus          ▼ BotResponseBus   ▼ app.emit
  analysis::runner   內建 NN（行程內）    Tauri webview
       │             | 雲端 API
       ▼ AnalysisBus  | mjai 子行程
       └──► ipc forwarder ──► app.emit
```

[`src/lib.rs`](./src/lib.rs) 在啟動時把這些 bus 接起來。
前端透過 push 事件（`mjai-event`、`bot-response`、
`bot-status`…）與 pull 指令和 backend 溝通，兩者的清單
都在 [`src/ipc/README.md`](./src/ipc/README.md)。開啟
AutoPlay 時，`autoplay` manager 會取用 bot 的決策，並透過
Chromium 抓包 backend（CDP）執行：雀魂是在牌桌上點擊，天鳳的
客戶端協定夠單純，則直接透過對局連線送出動作。

## 技術堆疊

| 層級 | 技術 |
|---|---|
| Shell | [Tauri](https://tauri.app) 2 |
| Backend | Rust（edition 2021）、`tokio`、`tracing`、`clap` |
| MITM | [`hudsucker`](https://crates.io/crates/hudsucker) 0.24（`rcgen-ca`、`rustls-client`） |
| CDP capture | [`chromiumoxide`](https://crates.io/crates/chromiumoxide) 0.9 |
| 麻將引擎 | [`riichienv-core`](https://github.com/smly/RiichiEnv) 0.4 |
| 內建 bot | [`candle`](https://github.com/huggingface/candle) 0.9（純 Rust NN 推論；權重內嵌） |
| 雲端推論 | [`reqwest`](https://crates.io/crates/reqwest) 0.13（rustls） |
| Protobuf | `prost` 0.14 + `prost-reflect` 0.16 |
| 前端 | [React](https://react.dev) 19、TypeScript、[Vite](https://vitejs.dev) 8 |
| 樣式 | [Tailwind CSS](https://tailwindcss.com) v4、[shadcn/ui](https://ui.shadcn.com)（Radix Nova preset） |
| 狀態 | [Zustand](https://github.com/pmndrs/zustand) |
| 圖表 | [Recharts](https://recharts.org) |
| 牌型渲染 | [`<mah-gen>`](https://github.com/eric200203/mahgen) Web Component |
| i18n | [react-i18next](https://react.i18next.com) |
| mjai bot 執行環境 | `python-build-standalone` 3.12 + [`uv`](https://github.com/astral-sh/uv)（依平台打包；僅外掛 bot 需要 —— 內建 bot 完全用不到） |

## 專案結構

```
.
├── src/
│   ├── analysis/      向聽 / 待牌 / 和牌率 / 風險 / 切牌搜尋
│   ├── autoplay/      bot 決策 → 點擊牌桌（雀魂）或送出協定 frame（天鳳），皆走 CDP
│   ├── bot/           Bot manager：內建 bot、雲端 API client、mjai 子行程執行器
│   ├── bridge/        各平台協定 → MjaiEvent
│   │   ├── majsoul/   雀魂（liqi protobuf）
│   │   ├── riichi_city/  Riichi City（僅 MITM）
│   │   └── tenhou/    天鳳（JSON tag stream，僅觀戰）
│   ├── capture/       抓包 backend 抽象（mitm | chromium）
│   ├── config/        AppConfig（TOML）區段與解析
│   ├── event_bus.rs   子系統間的 broadcast channel
│   ├── game_state/    riichienv 驅動的鏡射、snapshot、mahgen view
│   ├── github/        GitHub Releases client（bot 安裝、自我更新）
│   ├── history/       對局回放儲存與索引
│   ├── inspector/     訊框 / 事件 / bot reaction broadcaster
│   ├── ipc/           Tauri 指令、app state、capture supervisor
│   ├── logger/        每 session 紀錄目錄與每 target 檔案 appender
│   ├── proxy/         透過 hudsucker 的 MITM HTTP/HTTPS/WS；CA 位於 ./ca
│   ├── schema/        MjaiEvent enum 與 IPC payload 類型
│   ├── updater/       應用程式內自我更新（檢查 + 套用）
│   └── lib.rs         啟動與接線
├── native_bot/        內建 bot crate：obs/action codec、candle CNN、內嵌權重
├── mjai_bot/
│   └── example/       in-tree 規則型向聽優化器
├── frontend/          React + Vite + Tailwind + shadcn UI
│   └── src/
│       ├── routes/    Overview / GameDashboard / Bots / History / Logs / Settings / Setup / InspectorView / DiagnosticView
│       ├── tiles/     儀表板磚塊（header、hands、opponents、analysis…）
│       ├── stores/    Zustand store，一個領域一個（game、bot、config、theme…）
│       └── i18n/      en / ja / zh-TW / zh-CN
├── tests/             整合測試
├── capabilities/      Tauri 權限
├── icons/             應用程式圖示
├── tauri.conf.json    視窗與 bundle 設定
└── Cargo.toml
```

各模組的開發者指南位於對應的 `src/*/README.md`。

## mjai Bot 外掛介面

> 選用功能，主要面向開發者。[內建 bot](#內建-bot) 才是預設值，完全不需要
> 這一節的任何步驟 —— 只有當你想讓 Akagi 驅動 *另一個* 引擎時才會用到。

除了自家的 bot 之外，Akagi 也能驅動任何遵循 **mjai** 協定的引擎。這種 bot
是一個獨立子行程，透過 stdin/stdout 以 JSONL 溝通：Akagi 把對局以 mjai
事件餵給它，它則回覆一個動作，以及選填的 HUD 資料。

### 自行撰寫

```
mjai_bot/<name>/
├── bot.py            # JSONL stdin → JSONL stdout
├── pyproject.toml    # requires-python = ">=3.12"
├── manifest.toml     # 選填 — supported_modes、設定 schema
└── README.md
```

`bot.py` 從 stdin 每行讀取一個 mjai 事件 JSON 陣列，並從 stdout 每行寫出
一個 mjai 動作物件（無動作時輸出 `{"type":"none"}`）。Akagi 會把 stderr
內容寫入應用程式紀錄中的 `bot=<name>` 條目。

完整的 I/O 協定、mjai 事件流、reaction 與 `meta` HUD 格式、toast 通知，
以及 `manifest.toml` 設定，請見
**[`mjai_bot/README.md`](./mjai_bot/README.md)**。
[`mjai_bot/example/`](./mjai_bot/example/) 為一個可直接複製、可運作的
規則型範例 bot。

本機開發時，把 bot 資料夾放到 `mjai_bot/<name>/`，在 **Bots** 頁籤該 bot
列上點擊 **安裝環境** 即可建立其 venv —— 不必每次改動都重新打包安裝。
環境就緒前，啟用開關會保持停用。

### 安裝

**Bots** 頁籤可以從 GitHub release 或本機 ZIP 安裝 bot。

IPC 指令 `install_bot_from_github(repo, asset_glob?, name?)` 會抓取最新
release zip，解壓至 `mjai_bot/<name>/`，驗證 `bot.py`，並執行一次
`uv sync`。後續啟動很快 —— sync 會根據
`mjai_bot/<name>/.akagi/synced.stamp` 戳記決定是否跳過。

**從 ZIP 安裝** 是離線的等價流程：點擊 **瀏覽…** 選擇 `.zip`（或貼上其
路徑）即可。它執行完全相同的解壓 / 驗證 / `uv sync` 流程，且不會改動你的
來源 `.zip`。

### AGPL 邊界

Bot 以 Akagi 啟動的 **獨立 OS 子行程** 執行。通訊嚴格透過 stdin / stdout
上的 JSONL 進行 —— 沒有 in-process 連結、沒有共享位址空間、沒有 FFI。
這是刻意設計的授權邊界：AGPL 授權的 bot（例如連結 libriichi 的 Mortal）
會留在其自己的行程內，因此把它放入 `mjai_bot/<name>/` **不會** 讓 Akagi
成為該 bot 的衍生作品。

## 從原始碼建置

**前置需求**

- Rust（最新 stable，1.80+）
- Node.js 20+ 與 npm
- Tauri 2 系統相依：
  - **Linux**：`libwebkit2gtk-4.1-dev`、`libgtk-3-dev`、
    `libayatana-appindicator3-dev`、`librsvg2-dev`、
    `protobuf-compiler`
  - **macOS**：Xcode Command Line Tools
  - **Windows**：WebView2（Windows 11 已預先安裝）

**執行 / 建置**

```bash
# Debug — 啟動 GUI;Vite dev-server 由 Tauri 代理
cargo run

# 指定設定檔路徑
cargo run -- --config ./my-config.toml

# 為當前目標建置 portable zip
cargo install tauri-cli --locked          # 若尚未安裝
bash scripts/fetch-runtime.sh             # 抓取 runtime/<triple>/
cargo tauri build --no-bundle             # 產出 target/<triple>/release/akagi
bash scripts/package-zip.sh <target-triple>
# → dist/akagi-<version>-<os>-<arch>.zip

# 僅啟動前端 dev(Vite 在 :1420)
cd frontend && npm ci && npm run dev
```

**內建執行環境**

`scripts/fetch-runtime.sh <target-triple>` 會下載對應目標的
`python-build-standalone` 3.12 與 `uv`,並放置於 `runtime/`。
`scripts/package-zip.sh` 接著會把這個目錄複製到 zip 內的
binary 旁邊;`src/bot/runtime.rs` 在執行時會用 exe-adjacent
方式找到它,因此最終的 App 即使使用者沒有系統 Python 也能運作。

## 測試

整合測試位於 [`tests/`](./tests/)：

| 檔案 | 涵蓋範圍 |
|---|---|
| `analysis_pipeline.rs` | 端到端分析（事件 → 向聽 → 切牌建議） |
| `analysis_bench.rs` | hot path 效能 |
| `bot_lifecycle.rs` | 安裝 → sync → spawn → 來回通訊 |
| `example_bot.rs` | 規則型參考 bot 跑合成對局 |
| `mortal_zip_layout.rs` | 驗證 Mortal release zip 結構 |

```bash
cargo test               # 所有測試（含整合測試）
cargo test --release     # 用於效能 bench
```

## Releases 與 CI

GitHub Actions [`release.yml`](./.github/workflows/release.yml)
會在 tag 推送(`v3.*`)或手動觸發時建置,每個目標產出一個
portable zip:

| OS runner | 目標 | 產出檔案 |
|---|---|---|
| `ubuntu-22.04`(glibc 2.35) | `x86_64-unknown-linux-gnu` | `akagi-<version>-linux-x64.zip` |
| `macos-14` | `aarch64-apple-darwin` | `akagi-<version>-macos-arm64.zip` |
| `windows-latest` | `x86_64-pc-windows-msvc` | `akagi-<version>-windows-x64.zip` |
每個壓縮包皆於主程式旁隨附 `python-build-standalone` 3.12 與 `uv`，各類插件 Bot 無需事先安裝系統 Python 即可隨開即用。

專案管理員可在開放的 PR 底下留言 `/build-artifacts`，觸發自動建置該 PR 最新 commit 的成品。[`pr-build.yml`](./.github/workflows/pr-build.yml) 工作流程將在編譯完成後回覆三大平台的可攜式成品下載連結（檔案保存 14 天）。非管理員之留言將自動略過。

發佈 job 會以 [minisign](https://jedisct1.github.io/minisign/) 為
release zip 簽章（產生 `<asset>.zip.minisig`，trusted comment 為
檔名）。公鑰保存在儲存庫根目錄的
[`minisign.pub`](./minisign.pub) 並內嵌於應用程式；凡是經過下載
鏡像的更新都必須通過簽章驗證。簽章需要儲存庫 secret
`MINISIGN_SECRET_KEY`（兩行的 minisign 私鑰檔，需以免密碼方式
產生，如 `rsign generate -W`）；secret 缺失時 workflow 會警告並
發佈未簽章產物。

Tag 必須位於 `v3` 分支。

## 參考資料

| 來源 | 應用於 | 用途 |
|---|---|---|
| [mjai JSONL 規格（Gimite）](https://gimite.net/pukiwiki/index.php?Mjai%20%E9%BA%BB%E9%9B%80AI%E5%AF%BE%E6%88%A6%E3%82%B5%E3%83%BC%E3%83%90) | `src/schema/mjai/` | `MjaiEvent` enum 與 bot wire 協議 — 15 種事件、tile-string 格式、狀態機規則。 |
| [`EndlessCheng/mahjong-helper`](https://github.com/EndlessCheng/mahjong-helper)（Go 分析 CLI） | `src/analysis/` | `util/` 的直接 Rust 移植 — 向聽、待牌、和牌率、聽牌率、風險模型、切牌搜尋。 |
| [`Xerxes-2/MajsoulMax-rs`](https://github.com/Xerxes-2/MajsoulMax-rs)（Rust MITM proxy，**GPL-3.0**） | `src/proxy/handler.rs`、`src/bridge/majsoul/parser.rs`、`src/bridge/majsoul/proto/liqi.proto` | 雀魂 5 層 WS wire 格式參考（type byte → Wrapper → 內層訊息 → action protobuf）。**僅參考格式 — 未複製程式碼。** |
| [`smly/RiichiEnv`](https://github.com/smly/RiichiEnv)（Rust RL env + Python bindings） | `Cargo.toml`（`riichienv-core` 相依）、`src/analysis/`、`src/game_state/` | 牌 / 手牌 / 向聽 / 役 / 計分原語 + 遊戲狀態模型。分析引擎與 game tracker 都建構在它之上。 |
| [`eric200203/mahgen`](https://github.com/eric200203/mahgen)（麻將牌渲染 DSL） | `src/game_state/mahgen_view.rs`、前端 `<mah-gen>` | DSL 語法，用於後端預先編碼手牌 / 副露 / 河字串。 |
| [`smly/mjai.app`](https://github.com/smly/mjai.app)（麻將 AI 競賽平台） | `mjai_bot/`、`src/bot/` | bot 子行程慣例 — JSONL stdin/stdout、argv `python bot.py <player_id>`、`AKAGI_PLAYER_ID` 環境變數、批次結尾 flush 點。 |
| [`shinkuan/Akagi`](https://github.com/shinkuan/Akagi)（原版 Akagi，Python） | 架構 / 行為對齊 | 我們所重現的原始功能集：MITM proxy、mjai bridge、可插拔 bot、推薦 HUD。 |

## 授權與致謝

Akagi v3 採用 [Apache License 2.0](./LICENSE.txt)。
Copyright 2026 Shinkuan。第三方致謝資訊位於
[`NOTICE`](./NOTICE) — 請與授權一同閱讀。依 Apache-2.0
§4(d)，重新散布時必須附上這兩個檔案。

**內附 / 連結原始碼**

- **mahjong-helper**（MIT） — `src/analysis/` 為 `util/` 的 Rust 移植。
- **riichienv-core** / RiichiEnv（Apache-2.0） — Cargo 相依。
- **mahgen**（MIT） — DSL + `<mah-gen>` custom element。

**僅供參考**（未複製程式碼；列於 `NOTICE` 以示致謝）

- **MajsoulMax-rs**（GPL-3.0） — 僅參考雀魂 WS wire 格式。
- **mjai 規格**（Gimite） — bot wire 協議。
- **mjai.app** — bot 子行程慣例。

## 鳴謝

- [Akagi](https://github.com/shinkuan/Akagi/tree/v2)（Python，v2）與
  [AkagiNG](https://github.com/Xe-Persistent/Akagi-NG)（Electron + Python） —
  v3 所立基的前作。
- [`mjai.app`](https://github.com/smly/mjai.app) 以及 Gimite
  制定的 mjai 規格 — 讓可插拔 bot 成為可能的協議。
- 感謝上游 Akagi Discord 社群持續提供的問題回報、模型貢獻與寶貴回饋。
