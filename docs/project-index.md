# Project index

A map of the code, so a task can go straight to the right file and line range
instead of reading the whole tree. Design rationale lives in [PLAN.md](PLAN.md);
this file only says **where things are**.

**Keep it current:** when you add, remove or rename a module, a public item, a
window message, a menu command or a test, update the matching row here in the
same change. Line numbers are approximate anchors (±10); grep the symbol if one
has drifted.

Snapshot: M0 + minimalist strip + detail tooltip (design "WinHelper · Minimal") + metrics
sampling (CPU / RAM / GPU / disk, CPU / GPU temperature, throughput, internet status) + network lookup (public IP, geo,
LAN IP; 1 s after startup, on reconnect and "Refresh IP now") done, ~5.2 k lines of Rust in `src/`,
`src/ui/`, `src/metrics/` and `src/net/`. `config.rs` does not exist yet.

---

## 1. Lookup by task

| I need to… | Go to |
|---|---|
| Change startup / single instance | [app.rs](../src/app.rs), [win.rs](../src/win.rs) `SingleInstance` |
| Handle a new window message | [ui/window.rs](../src/ui/window.rs) `wnd_proc` (~L593–717) |
| Add a custom `WM_APP_*` message | see §4 for used IDs; define next to its owner |
| Change what the strip / tooltip displays, formatting | [ui/view.rs](../src/ui/view.rs) (`PanelView`, `Status`, `compact_percent`, `percent_text`, `compact_temp`, `temp_text`, `memory_text`, `disk_rates_text`, `rate_parts`, `footer_text`) |
| Feed real data into the panel | write into `Ui.view` via `state::with`, then invalidate the strip and `ui.tip` ([ui/state.rs](../src/ui/state.rs), `window::invalidate` ~L343); see `window::on_metrics` (~L189) / `on_netinfo` (~L222) for the pattern |
| Change geo providers / their parsing | [net/geo.rs](../src/net/geo.rs) `PROVIDERS`, `parse_*`; keep README "Privacy" in sync |
| Change how the LAN IP is picked | [net/local.rs](../src/net/local.rs) `choose`, `usable` |
| HTTP timeouts / user agent / TLS | [net/http.rs](../src/net/http.rs) `TIMEOUT_MS`, `USER_AGENT`, `Session::get` |
| When the network lookup runs | [net/mod.rs](../src/net/mod.rs) `Lookup::start` (idle), `Lookup::refresh` (first via `NET_START_TIMER` in [ui/window.rs](../src/ui/window.rs) 1 s after the panel is shown; menu; also `window::on_metrics` on offline → online); add periodic / change-notify triggers in `run`; keep README "Privacy" in sync |
| How a lookup result changes the rows (stale, kept location) | [ui/view.rs](../src/ui/view.rs) `NetView::update` |
| Change how CPU / RAM / GPU are measured | [metrics/cpu.rs](../src/metrics/cpu.rs), [metrics/memory.rs](../src/metrics/memory.rs), [metrics/gpu.rs](../src/metrics/gpu.rs) |
| Change how disk activity is measured | [metrics/disk.rs](../src/metrics/disk.rs) (`DiskSampler`, `active_from_idle`) |
| Change how temperatures are read | CPU: [metrics/thermal.rs](../src/metrics/thermal.rs) (`CpuTemp`, `celsius`, `Zones` placeholder filter); GPU: [metrics/gpu.rs](../src/metrics/gpu.rs) (`GpuTemp`, `hottest`) |
| Add a PDH counter | [metrics/pdh.rs](../src/metrics/pdh.rs) (`Query::open` / `add` / `collect`, `value`, `array`) |
| Strip width / temperature slots / resize on sensor change | `layout::strip_width`, `window::panel_size`, `fit_to_content` (~L299) |
| Change how throughput / internet status are measured | [metrics/traffic.rs](../src/metrics/traffic.rs) (`counts` filter, `rate`), [metrics/connectivity.rs](../src/metrics/connectivity.rs) (`classify`) |
| Change the sampling rate / when it is fast | [metrics/mod.rs](../src/metrics/mod.rs) `FAST_INTERVAL`, `SLOW_INTERVAL`, `run`; `window::update_sampling` (~L172) decides "active" (focused or hovered) |
| Move / resize something on the strip or tooltip | [ui/layout.rs](../src/ui/layout.rs) constants (96-DPI px; tooltip row tops are derived `TIP_*` consts) |
| Change how something is drawn | [ui/paint.rs](../src/ui/paint.rs) `StripPaint::draw` (~L149), `TipPaint::draw` (~L301) and their helpers |
| Change an icon | [ui/paint.rs](../src/ui/paint.rs) `mod glyph` (Segoe MDL2 Assets code points) |
| Change colors / load / temperature thresholds | [ui/theme/mod.rs](../src/ui/theme/mod.rs) `DARK`, `LIGHT`, `Load::of`, `Heat::of` |
| Change fonts | [ui/paint.rs](../src/ui/paint.rs) `Fonts::new` (~L58) |
| New GDI primitive / RAII wrapper | [ui/gdi.rs](../src/ui/gdi.rs) |
| Context menu item or command | [ui/menu.rs](../src/ui/menu.rs) (`CMD_*`, `build`, `command`, `Command`) + dispatch in `window::show_menu` (~L461) |
| Tray icon behavior | [ui/tray.rs](../src/ui/tray.rs); events handled in `wnd_proc` `WM_APP_TRAY` |
| Close button (hover popup) | [ui/close_button.rs](../src/ui/close_button.rs); placement math in `layout::close_button_origin` |
| Detail tooltip (hover popup) | [ui/tooltip.rs](../src/ui/tooltip.rs) (`DELAY`, `show_near`, `hide`); placement math in `layout::tip_origin`; opened from `window::on_hover_timer`, moved by `follow_strip` |
| Hover border / hover tracking | `window::on_hover`, `end_hover`, `on_hover_timer` (~L330–402), `layout::in_hover_zone` |
| Opacity / always-on-top | `window::set_opacity` (~L404), `toggle_topmost` (~L419) |
| DPI handling | `ui::scale` ([ui/mod.rs](../src/ui/mod.rs)), `wnd_proc` `WM_DPICHANGED`, `window::panel_size` |
| Theme detection / live switch | `theme::system_theme`, `theme::apply_menu_theme`, `window::on_theme_change` |
| Debug preview data | [ui/demo.rs](../src/ui/demo.rs) (`WINHELPER_DEMO`, `WINHELPER_THEME`) |
| Add a `windows` crate feature | [Cargo.toml](../Cargo.toml) feature list |
| Icon / manifest / version info | [build.rs](../build.rs), [res/](../res/) |
| Visual spec | [design/WinHelper.bl](../design/WinHelper.bl) board "WinHelper · Minimal" (Brilliant canvas; read via the Brilliant tools, not as text) |

---

## 2. File map

| File | Lines | Role | Unsafe? | Tests |
|---|---|---|---|---|
| [src/main.rs](../src/main.rs) | 17 | Entry, `windows_subsystem`, error box on failure | – | – |
| [src/app.rs](../src/app.rs) | 21 | `run()`: single instance → `window::create` → message loop | – | – |
| [src/win.rs](../src/win.rs) | 91 | `OwnedHandle` (Send + Sync), `Event` (auto-reset, `set` / `wait`), `SingleInstance`, `error_box` | yes | – |
| [src/metrics/mod.rs](../src/metrics/mod.rs) | 214 | `Sampler` thread handle, `MetricsSnapshot`, `WM_APP_METRICS`, intervals, `run` loop, `remaining` | yes | yes (+ ignored live test) |
| [src/metrics/pdh.rs](../src/metrics/pdh.rs) | 117 | PDH `Query` RAII (`open`, `add` English path, `collect`), `Counter`, `value`, `array` | yes | – |
| [src/metrics/cpu.rs](../src/metrics/cpu.rs) | 86 | `CpuTimes`, `usage` delta math, `CpuSampler` (`GetSystemTimes`) | yes | yes |
| [src/metrics/memory.rs](../src/metrics/memory.rs) | 48 | `Memory` (re-exported by `ui::view`), `read` (`GlobalMemoryStatusEx`) | yes | yes |
| [src/metrics/gpu.rs](../src/metrics/gpu.rs) | 219 | `engine_key`, `aggregate`, `GpuSampler` (PDH); `hottest`, `GpuTemp` (D3DKMT adapter perf data, handles closed on drop) | yes | yes |
| [src/metrics/disk.rs](../src/metrics/disk.rs) | 93 | `Disk` (re-exported by `ui::view`), `active_from_idle`, `DiskSampler` (PDH `PhysicalDisk(_Total)`) | – | yes |
| [src/metrics/thermal.rs](../src/metrics/thermal.rs) | 144 | `celsius`, `Zones` (ignores constant placeholder zones), `CpuTemp` (PDH `Thermal Zone Information`) | – | yes |
| [src/metrics/traffic.rs](../src/metrics/traffic.rs) | 177 | `Traffic` (re-exported by `ui::view`), `counts` interface filter, `rate` delta math, `GetIfTable2` RAII `Table`, `TrafficSampler` | yes | yes |
| [src/metrics/connectivity.rs](../src/metrics/connectivity.rs) | 72 | `Connectivity` (`GetNetworkConnectivityHint` via `GetProcAddress`), `classify` | yes | yes |
| [src/net/mod.rs](../src/net/mod.rs) | 162 | `Lookup` thread handle, `NetInfo`, `WM_APP_NETINFO`, `run` loop (starts idle, looks up on each `refresh`), `local_time` | yes | ignored live test |
| [src/net/http.rs](../src/net/http.rs) | 144 | WinHTTP RAII `Handle`, `Session::open` / `get` (HTTPS, 5 s timeouts, system proxy, 64 KB cap) | yes | – |
| [src/net/geo.rs](../src/net/geo.rs) | 183 | `Geo`, `PROVIDERS` chain (ipwho.is → ipinfo.io → ipify), JSON / text parsing | – | yes |
| [src/net/local.rs](../src/net/local.rs) | 161 | `lan_ip` via `GetAdaptersAddresses`, `choose` best adapter | yes | yes |
| [src/ui/mod.rs](../src/ui/mod.rs) | 57 | Module decls, `BASE_DPI`, `scale(px, dpi)` | – | yes |
| [src/ui/window.rs](../src/ui/window.rs) | 717 | Strip class, `create`, message loop, `wnd_proc`, metrics + netinfo apply (reconnect → IP refresh, resize when a temperature sensor appears / disappears), sampling rate, hover + tooltip timing, `follow_strip`, opacity, topmost, visibility, menu dispatch, theme change, DPI | yes | – |
| [src/ui/close_button.rs](../src/ui/close_button.rs) | 174 | Close button: owned popup window with its own wndproc | yes | – |
| [src/ui/tooltip.rs](../src/ui/tooltip.rs) | 135 | Detail tooltip: owned click-through popup, `DELAY`, `show_near`, `hide`, own wndproc | yes | – |
| [src/ui/state.rs](../src/ui/state.rs) | 66 | Thread-local `Ui` state, `init` / `clear` / `with` | – | – |
| [src/ui/view.rs](../src/ui/view.rs) | 366 | `PanelView`, `Status`, `NetView` + `update`, re-exports `Memory` / `Traffic` / `Disk`, formatting | – | yes |
| [src/ui/layout.rs](../src/ui/layout.rs) | 238 | Strip / close / tooltip geometry constants, `strip_width`, `bar_fill`, `close_button_origin`, `tip_origin`, `in_hover_zone` | – | yes |
| [src/ui/theme/mod.rs](../src/ui/theme/mod.rs) | 296 | `Theme`, `Palette`, `DARK` / `LIGHT`, `Load`, `Heat`, `system_theme`, `apply_menu_theme` | yes | yes |
| [src/ui/paint.rs](../src/ui/paint.rs) | 618 | `glyph` code points, `Fonts`, `StripPaint`, `TipPaint`, `close_button` painter | – | – |
| [src/ui/gdi.rs](../src/ui/gdi.rs) | 209 | `PaintGuard`, `BackBuffer`, `Font`, `fill`, `frame`, `round_rect`, `text`, `text_width` | yes | – |
| [src/ui/menu.rs](../src/ui/menu.rs) | 183 | Popup menu build / show, `MenuState`, `Command`, `OPACITY_LEVELS` | yes | yes |
| [src/ui/tray.rs](../src/ui/tray.rs) | 66 | `add`, `remove`, `taskbar_created_message`, `WM_APP_TRAY` | yes | – |
| [src/ui/demo.rs](../src/ui/demo.rs) | 94 | Debug-only `view()` and `theme_override()` | – | – |
| [build.rs](../build.rs) | 19 | `embed-resource` compile of `res/winhelper.rc` with version macros | – | – |

Other: [docs/PLAN.md](PLAN.md) (design, milestones §8, manual tests §9),
[.github/workflows/ci.yml](../.github/workflows/ci.yml) (fmt, clippy `--all-targets -D warnings`,
test, release build), `design/` (Brilliant files; `.brilliant/` is tool metadata, ignore it).
The implemented design is the canvas board **"WinHelper · Minimal"**; the older
"WinHelper — Panel Design" board and its palettes are kept for reference only.

---

## 3. Key types and functions

### Data model ([ui/view.rs](../src/ui/view.rs))
```rust
struct Memory { used: u64, total: u64 }          // bytes; .percent() -> f32 (defined in metrics/memory.rs)
struct Traffic { down: f64, up: f64 }            // bytes/s (defined in metrics/traffic.rs)
struct Disk { active: f32, read: f64, write: f64 } // %, bytes/s (defined in metrics/disk.rs)
struct NetView { public_ip, lan_ip, location, country_code: Option<String>, stale: bool,
                 updated: Option<String> }       // "14:02" of the last successful lookup
struct PanelView { cpu: Option<f32>, ram: Option<Memory>, gpu: Option<f32>,
                   cpu_temp: Option<f32>, gpu_temp: Option<f32>, disk: Option<Disk>,
                   traffic: Option<Traffic>, online: Option<bool>, net: NetView }
enum Status { Online, Offline, Unknown }         // PanelView::status(): `online`, else stale / known IP
fn round_percent(f32) -> u32                     // clamp 0..=100, NaN -> 0
fn compact_percent(Option<f32>) -> String        // strip: "42%" | "N/A"
fn percent_text(Option<f32>) -> String           // tooltip: "42 %" | "N/A"
fn round_celsius(f32) -> u32                     // clamp 0..=999, NaN -> 0
fn compact_temp(f32) -> String                   // strip: "58°"
fn temp_text(f32) -> String                      // tooltip: "58 °C"
fn disk_rates_text(read, write: f64) -> String   // "R 4.1 · W 0.3 MB/s" (unit of the larger)
fn memory_text(Memory) -> String                 // "10.1 / 16.0 GB" (GiB)
fn rate_parts(f64) -> (String, &str)             // ("1.2", "MB/s"); ≤ 4 + 4 chars, 1024-based
fn rate_text(f64) -> String                      // "1.2 MB/s"
fn footer_text(&NetView) -> String               // tooltip footer
NetView::update(&mut self, NetInfo)              // LAN always replaced; public None → keep + stale
```
`None` means unavailable → painted as N/A / "—" / "–"; a `None` temperature is not drawn
at all and drops its strip slot. `net.stale` grays IP and location.
An IP-only answer (ipify) for the same IP keeps the known location.

### Network ([net/mod.rs](../src/net/mod.rs))
```rust
struct Geo { ip: String, location: Option<String>, country_code: Option<String> }  // net/geo.rs
struct NetInfo { lan_ip: Option<String>, public: Option<Geo>, at: Option<String> }  // public None = all providers failed
Lookup::start(panel: HWND) -> Option<Lookup>     // spawns thread "network"; idle until the first refresh
Lookup::refresh()                                // menu + reconnect; requests during a lookup collapse into one
Lookup::take_latest() -> Option<NetInfo>
impl Drop for Lookup                             // stop + wake; joins if idle, detaches if mid-request
```
Loop: wait on the wake event until `pending`, then `local::lan_ip()` + `geo::lookup(session)`,
stamp `at` (`GetLocalTime`), store, post `WM_APP_NETINFO`. The WinHTTP session is opened
lazily and reused. Providers in order: `ipwho.is/` (name + code) → `ipinfo.io/json`
(city + code) → `api.ipify.org/` (IP only). IPs are validated with `IpAddr` parsing. LAN IP:
IPv4 of an up, non-loopback adapter, preferring one with a gateway, then lowest
`Ipv4Metric`; APIPA addresses skipped. No lookup in demo mode.

### Metrics ([metrics/mod.rs](../src/metrics/mod.rs))
```rust
struct MetricsSnapshot { cpu: Option<f32>, ram: Option<Memory>, gpu: Option<f32>,
                         cpu_temp: Option<f32>, gpu_temp: Option<f32>, disk: Option<Disk>,
                         traffic: Option<Traffic>, online: Option<bool> }
Sampler::start(panel: HWND) -> Option<Sampler>   // spawns thread "metrics"; None → panel stays N/A
Sampler::set_active(bool)                        // fast (1 s) vs slow (10 s); activation wakes the thread
Sampler::take_latest() -> Option<MetricsSnapshot>
impl Drop for Sampler                            // stop flag + wake event, then join
```
Loop: sample when `last + interval` has passed, else `Event::wait` the remainder
(woken early by stop / activation). First sample always after 1 s. CPU = `GetSystemTimes`
delta; RAM = `GlobalMemoryStatusEx` (used = total − avail); GPU = PDH English counter
`\GPU Engine(*)\Utilization Percentage`, summed per engine (`luid_…_phys_N_eng_M`),
max over engines; `GpuSampler::open` → `None` means GPU N/A. Disk = PDH
`\PhysicalDisk(_Total)\% Idle Time` (active = 100 − idle) + `Disk Read / Write Bytes/sec`.
CPU temp = PDH `\Thermal Zone Information(*)\High Precision Temperature` (0.1 K; falls back to
`Temperature` in K), hottest zone that has changed at least once (constant 27.85 °C placeholder
zones never count). GPU temp = `D3DKMTQueryAdapterInfo(KMTQAITYPE_ADAPTERPERFDATA)` per adapter
(0.1 °C), hottest non-zero; `GpuTemp::open` → `None` hides it. Traffic = `GetIfTable2`
octet deltas of up, hardware, non-filter, non-loopback interfaces (matched by LUID) over
the elapsed time; baseline taken in `TrafficSampler::new`. Online =
`GetNetworkConnectivityHint` (resolved at run time; missing → `None`). No sampler in demo mode.

### UI state ([ui/state.rs](../src/ui/state.rs))
```rust
struct Ui { panel: HWND, close: HWND, tip: HWND, dpi: u32, theme: Theme, fonts: Fonts,
            view: PanelView, hovered: bool, hover_since: Option<Instant>, tip_shown: bool,
            close_hovered: bool, focused: bool, sampler: Option<Sampler>, net: Option<Lookup>,
            topmost: bool, opacity: u8 }
fn with<R>(f: impl FnOnce(&mut Ui) -> R) -> Option<R>
```
**Rule:** never call a re-entrant Win32 API (`SetWindowPos`, `ShowWindow`,
`TrackPopupMenu`, …) inside `with`; copy values out, drop the borrow, then call.
`with` returns `None` on re-entry rather than panicking.

### Hover and tooltip flow ([ui/window.rs](../src/ui/window.rs))
`WM_NCMOUSEMOVE` → `on_hover` (sets `hovered`, `hover_since`, shows the close button,
starts `HOVER_TIMER`) → `on_hover_timer` every 100 ms: cursor outside strip + close →
`end_hover` (hides tooltip and close); else once `hover_since` is `tooltip::DELAY` (400 ms)
old → `tooltip::show_near`. `show_menu` hides the tooltip and clears `hover_since` (so the
timer cannot reopen it during `TrackPopupMenu`), then restarts the delay. `WM_MOVE` /
`WM_DPICHANGED` → `follow_strip` repositions the close button and an open tooltip.

### Theme ([ui/theme/mod.rs](../src/ui/theme/mod.rs))
- `hex(0xRRGGBB) -> COLORREF` (byte-swaps to `0x00BBGGRR`).
- `Palette` fields: `panel, border, border_hover, label, value, secondary, stale_label, stale_value, track, track_na, normal, elevated, high, high_value, elevated_value, country, tag_bg, tip_bg, tip_border, close_bg, close_border, close_glyph, close_hover, close_hover_glyph`.
- `normal` also colors the download arrow and the online dot; `country` the upload arrow; `high` / `high_value` the offline dot / globe.
- `Load::of(pct)`: `0..=59` Normal, `60..=85` Elevated, `>85` High. `bar_color` (green / amber / red), `text_color` (neutral / `elevated_value` / `high_value`).
- `Heat::of(°C)`: `0..=74` Normal, `75..=89` Warm, `≥90` Hot. `text_color` (`secondary` / `elevated_value` / `high_value`).
- `system_theme()` reads HKCU `…\Themes\Personalize\AppsUseLightTheme` (missing → Light); debug builds honor `WINHELPER_THEME` first.
- `apply_menu_theme` uses uxtheme ordinals 135/136 (undocumented; no-op if absent).

### Layout ([ui/layout.rs](../src/ui/layout.rs)), all 96-DPI px
Strip height `30`; `strip_width(cpu_temp, gpu_temp)` = `BASE_STRIP_WIDTH 452` + `TEMP_ITEM 26`
per reporting sensor (504 with both). Base = `2·PAD_X 12` + 4 metric items (CPU, RAM, GPU, Disk;
`ICON 14` + `ICON_GAP 5` + `PERCENT_SLOT 27`) + separator `1` + globe item (`14 + 5 + DOT 6`)
+ 2 rate items (`ARROW 12` + `3` + `RATE_SLOT 27` + `3` + `UNIT_SLOT 22`) + `7·ITEM_GAP 12`.
A temperature (`TEMP_GAP 4` + `TEMP_SLOT 22`, Consolas 10) follows the CPU / GPU percentage.
Close button `22` square, gap `6`, glyph `11`, vertically centered. Tooltip `264 × 356`,
`TIP_PAD_X 14`, `TIP_PAD_Y 12`, gap to strip `6`. Row tops: SYSTEM 12, metrics 33 / 64 / 95 /
126 (CPU, RAM, GPU, Disk; row 16 + gap 4 + bar 3), separator 161, NETWORK 174, net rows
197 + 23·i (Public IP, Location, LAN IP, Download, Upload), separator 317, footer 330.

### Fonts ([ui/paint.rs](../src/ui/paint.rs) `Fonts`)
Segoe MDL2 Assets: `icon` 14 · `tip_icon` 13 · `arrow` 12 · `close` 11. Consolas: `value` 12 ·
`detail` 11 · `unit` 10. Segoe UI: `text` 12 · `small` 11 · `caption` 10 semibold ·
`dot` 10 (draws `●`; GDI shapes are not anti-aliased). Recreated on `WM_DPICHANGED`.
Glyphs (`paint::glyph`): CPU `E950`, RAM `E964`, GPU `E7F4`, disk `EDA2`, globe `E774`, down `E74B`,
up `E74A`, close `E8BB`.

### Painting
`window::on_paint` → `gdi::PaintGuard` + `gdi::BackBuffer` → `StripPaint { width, height, dpi, palette, fonts, view, hovered }.draw(dc)` → blit.
`tooltip::on_paint` → same, with `TipPaint { width, height, dpi, palette, fonts, view }`.
Internal helpers: `Px` (`v`, `rect` scale design coords), `dot`, `status_colors`,
`status_text`; `StripPaint::{metric, rate}`; `TipPaint::{separator, metric, text_row,
location_row, rate_row}`.

### Window creation (`window::create`, ~L71)
Class `WinHelper.Panel`, `CS_DBLCLKS | CS_DROPSHADOW`; `WS_POPUP` with
`WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TOPMOST`. Then: close button → tooltip →
`state::init` → menu theme → opacity 100 → rounded corners → borders → tray →
`place_initial` (top-right of the primary work area, `SCREEN_MARGIN` 16) → `SW_SHOWNOACTIVATE`.
Whole client area returns `HTCAPTION` (drag anywhere), so mouse moves / right
clicks arrive as `WM_NC*` messages. The tooltip (`WinHelper.Tip`) is
`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE`, answers `HTTRANSPARENT`, and
keeps full opacity (the menu's opacity applies to the strip and close button only).

---

## 4. Messages, IDs, constants

| Name | Value | Defined in | Handled in |
|---|---|---|---|
| `WM_APP_TRAY` | `WM_APP + 1` | tray.rs | window.rs `wnd_proc`: R-click → menu, dbl-click → toggle visible |
| `WM_APP_HIDE_PANEL` | `WM_APP + 2` | close_button.rs | window.rs → `set_visible(false)` |
| `WM_APP_METRICS` | `WM_APP + 3` | metrics/mod.rs | window.rs `on_metrics`: copy snapshot into `view`, refresh IP on offline → online, `fit_to_content` when a temperature appears / disappears, repaint strip + tooltip if changed |
| `WM_APP_NETINFO` | `WM_APP + 4` | net/mod.rs | window.rs `on_netinfo`: `NetView::update`, repaint strip + tooltip if changed |
| `TaskbarCreated` | registered | tray.rs | window.rs: re-add tray icon |
| `HOVER_TIMER` | timer id 1, 100 ms | window.rs | `on_hover_timer` (hover end + tooltip delay) |
| `NET_START_TIMER` | timer id 2, one-shot 1 s | window.rs `create` | first `Lookup::refresh` once the message loop runs |
| `tooltip::DELAY` | 400 ms | tooltip.rs | `on_hover_timer` |
| `IDI_APP` | resource 1 | window.rs / res/winhelper.rc | class icon, tray icon |
| `CMD_TOPMOST..CMD_EXIT` | 1–5 | menu.rs | `menu::command` → `Command` |
| `CMD_OPACITY_BASE` | 100 (+ level) | menu.rs | `Command::Opacity(u8)`; levels `[100, 85, 70]` |
| Mutex | `Local\WinHelper.SingleInstance` | app.rs | – |
| Window classes | `WinHelper.Panel`, `WinHelper.Close`, `WinHelper.Tip` | window.rs, close_button.rs, tooltip.rs | – |

Refresh IP (`CMD_REFRESH_IP` → `Command::RefreshIp` → `Lookup::refresh`) is enabled
when the lookup thread runs (`MenuState::can_refresh_ip`; grayed in demo mode).
Copy IP (`CMD_COPY_IP`) and Autostart (`CMD_AUTOSTART`) exist but are grayed: wire them up in M3 / M4.

`wnd_proc` cases: `WM_NCHITTEST`, `WM_NCMOUSEMOVE`, `WM_TIMER`, `WM_MOVE` (`follow_strip`),
`WM_NCRBUTTONUP`, `WM_CONTEXTMENU`, `WM_APP_TRAY`, `WM_APP_HIDE_PANEL`,
`WM_APP_METRICS`, `WM_APP_NETINFO`, `WM_ACTIVATE` (sets `focused`, updates sampling rate),
`WM_SETTINGCHANGE` (theme), `WM_ERASEBKGND`, `WM_PAINT`, `WM_DPICHANGED`,
`WM_DESTROY` (kill timer, remove tray, stop sampler + lookup, `state::clear`, quit), `TaskbarCreated`.
Tooltip `wnd_proc`: `WM_NCHITTEST` → `HTTRANSPARENT`, `WM_MOUSEACTIVATE` → `MA_NOACTIVATE`,
`WM_ERASEBKGND`, `WM_PAINT`.

---

## 5. Tests (pure logic only)

| Module | Covers |
|---|---|
| ui/mod.rs | `scale`: identity, 125/150/200 %, rounding, zero DPI |
| ui/view.rs | percent rounding/clamping (both forms), temperature forms, disk rates in a shared unit, GiB formatting, `rate_parts` units / rounding / slot width / NaN / negative, `Status` fallback order, `footer_text`, `NetView::update` (success → failure → success incl. `updated`, IP-only answer keeps / drops location) |
| net/geo.rs | ipwho.is / ipinfo / ipify parsing: full replies, `success:false`, missing / blank fields, invalid IP, bad country code, non-JSON |
| net/local.rs | `choose`: gateway first, then metric; skips down / loopback / APIPA; empty → `None` |
| net/mod.rs | `live::looks_up_real_network` is `#[ignore]` (hits the real providers) |
| metrics/mod.rs | `remaining` (interval due math); `live::samples_real_counters` (CPU / RAM / GPU / traffic / online / disk / temperatures) is `#[ignore]` — run with `cargo test -- --ignored --nocapture` |
| metrics/cpu.rs | `usage` delta math: busy share, idle / full, zero / backwards deltas |
| metrics/memory.rs | `Memory::percent`, zero total |
| metrics/gpu.rs | `engine_key` parsing, sum per engine + max, same-type engines kept apart, clamp / NaN, empty → `None`; `hottest` skips 0 / out-of-range adapters |
| metrics/disk.rs | `active_from_idle` complement + clamp / NaN, `rate` junk |
| metrics/thermal.rs | `celsius` units / junk; `Zones`: constant placeholder ignored until it moves, jitter < 0.05 °C, hottest live zone |
| metrics/traffic.rs | `rate`: sums matching LUIDs, skips new / reset interfaces, needs elapsed time; `counts` filter |
| metrics/connectivity.rs | `classify`: connectivity levels → online / offline / unknown |
| ui/layout.rs | strip width 452 / 478 / 504 by temperature slots, tooltip row tops / height 356, `bar_fill`, close button flip + centering, tooltip below / above / clamped, hover bridge |
| ui/theme/mod.rs | `hex` packing, `Load` thresholds, value text color per load, `Heat` thresholds + colors |
| ui/menu.rs | command id → `Command` mapping |

---

## 6. Not built yet (planned homes, PLAN.md §4)

| Planned file | Milestone | Will hold |
|---|---|---|
| `src/net/*` additions | M3 | 10 min periodic refresh, `NotifyIpInterfaceChange` (debounced) → `Lookup::refresh`, error detail in the tooltip, Copy IP |
| `src/config.rs` | M4 | Settings load/save (serde_json), autostart, setting to disable the public IP lookup |

`serde_json` is already a dependency (only `Value` is used; default features off, `std` on);
settings with derive will also need `serde` with `derive`.
