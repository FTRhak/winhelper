# Project index

A map of the code, so a task can go straight to the right file and line range
instead of reading the whole tree. Design rationale lives in [PLAN.md](PLAN.md);
this file only says **where things are**.

**Keep it current:** when you add, remove or rename a module, a public item, a
window message, a menu command or a test, update the matching row here in the
same change. Line numbers are approximate anchors (±10); grep the symbol if one
has drifted.

Snapshot: M0 + visual design + metrics sampling (CPU / RAM / GPU) + network lookup
(public IP, geo, LAN IP; on start and "Refresh IP now") done, ~3.2 k lines of Rust in
`src/`, `src/ui/`, `src/metrics/` and `src/net/`. `config.rs` does not exist yet.

---

## 1. Lookup by task

| I need to… | Go to |
|---|---|
| Change startup / single instance | [app.rs](../src/app.rs), [win.rs](../src/win.rs) `SingleInstance` |
| Handle a new window message | [ui/window.rs](../src/ui/window.rs) `wnd_proc` (~L478–595) |
| Add a custom `WM_APP_*` message | see §4 for used IDs; define next to its owner |
| Change what a row displays / formatting | [ui/view.rs](../src/ui/view.rs) (`PanelView`, `percent_text`, `memory_text`) |
| Feed real data into the panel | write into `Ui.view` via `state::with`, then invalidate the panel ([ui/state.rs](../src/ui/state.rs), `window::invalidate` ~L277); see `window::on_metrics` (~L171) / `on_netinfo` (~L187) for the pattern |
| Change geo providers / their parsing | [net/geo.rs](../src/net/geo.rs) `PROVIDERS`, `parse_*`; keep README "Privacy" in sync |
| Change how the LAN IP is picked | [net/local.rs](../src/net/local.rs) `choose`, `usable` |
| HTTP timeouts / user agent / TLS | [net/http.rs](../src/net/http.rs) `TIMEOUT_MS`, `USER_AGENT`, `Session::get` |
| When the network lookup runs | [net/mod.rs](../src/net/mod.rs) `Lookup::start` (once), `Lookup::refresh` (menu); add periodic / change-notify triggers in `run` |
| How a lookup result changes the rows (stale, kept location) | [ui/view.rs](../src/ui/view.rs) `NetView::update` |
| Change how CPU / RAM / GPU are measured | [metrics/cpu.rs](../src/metrics/cpu.rs), [metrics/memory.rs](../src/metrics/memory.rs), [metrics/gpu.rs](../src/metrics/gpu.rs) |
| Change the sampling rate / when it is fast | [metrics/mod.rs](../src/metrics/mod.rs) `FAST_INTERVAL`, `SLOW_INTERVAL`, `run`; `window::update_sampling` (~L158) decides "active" (focused or hovered) |
| Move / resize something on the panel | [ui/layout.rs](../src/ui/layout.rs) constants (96-DPI px) |
| Change how something is drawn | [ui/paint.rs](../src/ui/paint.rs) `PanelPaint::draw` and row helpers |
| Change colors / load thresholds | [ui/theme/mod.rs](../src/ui/theme/mod.rs) `DARK`, `LIGHT`, `Load::of` |
| Change fonts | [ui/paint.rs](../src/ui/paint.rs) `Fonts::new` (~L32) |
| New GDI primitive / RAII wrapper | [ui/gdi.rs](../src/ui/gdi.rs) |
| Context menu item or command | [ui/menu.rs](../src/ui/menu.rs) (`CMD_*`, `build`, `command`, `Command`) + dispatch in `window::show_menu` (~L421) |
| Tray icon behavior | [ui/tray.rs](../src/ui/tray.rs); events handled in `wnd_proc` `WM_APP_TRAY` |
| Close button (hover popup) | [ui/close_button.rs](../src/ui/close_button.rs); placement math in `layout::close_button_origin` |
| Hover border / hover tracking | `window::on_hover`, `end_hover`, `on_hover_timer` (~L256–308), `layout::in_hover_zone` |
| Opacity / always-on-top | `window::set_opacity` (~L309), `toggle_topmost` (~L324) |
| DPI handling | `ui::scale` ([ui/mod.rs](../src/ui/mod.rs)), `wnd_proc` `WM_DPICHANGED`, `window::panel_size` |
| Theme detection / live switch | `theme::system_theme`, `theme::apply_menu_theme`, `window::on_theme_change` |
| Debug preview data | [ui/demo.rs](../src/ui/demo.rs) (`WINHELPER_DEMO`, `WINHELPER_THEME`) |
| Add a `windows` crate feature | [Cargo.toml](../Cargo.toml) feature list |
| Icon / manifest / version info | [build.rs](../build.rs), [res/](../res/) |
| Visual spec | [design/WinHelper.bl](../design/WinHelper.bl) (Brilliant canvas; read via the Brilliant tools, not as text) |

---

## 2. File map

| File | Lines | Role | Unsafe? | Tests |
|---|---|---|---|---|
| [src/main.rs](../src/main.rs) | 17 | Entry, `windows_subsystem`, error box on failure | – | – |
| [src/app.rs](../src/app.rs) | 21 | `run()`: single instance → `window::create` → message loop | – | – |
| [src/win.rs](../src/win.rs) | 91 | `OwnedHandle` (Send + Sync), `Event` (auto-reset, `set` / `wait`), `SingleInstance`, `error_box` | yes | – |
| [src/metrics/mod.rs](../src/metrics/mod.rs) | 178 | `Sampler` thread handle, `MetricsSnapshot`, `WM_APP_METRICS`, intervals, `run` loop, `remaining` | yes | yes (+ ignored live test) |
| [src/metrics/cpu.rs](../src/metrics/cpu.rs) | 86 | `CpuTimes`, `usage` delta math, `CpuSampler` (`GetSystemTimes`) | yes | yes |
| [src/metrics/memory.rs](../src/metrics/memory.rs) | 48 | `Memory` (re-exported by `ui::view`), `read` (`GlobalMemoryStatusEx`) | yes | yes |
| [src/metrics/gpu.rs](../src/metrics/gpu.rs) | 219 | `engine_key`, `aggregate`, PDH `Query` RAII, `GpuSampler` | yes | yes |
| [src/net/mod.rs](../src/net/mod.rs) | 151 | `Lookup` thread handle, `NetInfo`, `WM_APP_NETINFO`, `run` loop (on start + on `refresh`) | yes | ignored live test |
| [src/net/http.rs](../src/net/http.rs) | 144 | WinHTTP RAII `Handle`, `Session::open` / `get` (HTTPS, 5 s timeouts, system proxy, 64 KB cap) | yes | – |
| [src/net/geo.rs](../src/net/geo.rs) | 183 | `Geo`, `PROVIDERS` chain (ipwho.is → ipinfo.io → ipify), JSON / text parsing | – | yes |
| [src/net/local.rs](../src/net/local.rs) | 161 | `lan_ip` via `GetAdaptersAddresses`, `choose` best adapter | yes | yes |
| [src/ui/mod.rs](../src/ui/mod.rs) | 56 | Module decls, `BASE_DPI`, `scale(px, dpi)` | – | yes |
| [src/ui/window.rs](../src/ui/window.rs) | 597 | Panel class, `create`, message loop, `wnd_proc`, metrics + netinfo apply, sampling rate, hover, opacity, topmost, visibility, menu dispatch, theme change, DPI | yes | – |
| [src/ui/close_button.rs](../src/ui/close_button.rs) | 174 | Separate owned popup window with its own wndproc | yes | – |
| [src/ui/state.rs](../src/ui/state.rs) | 58 | Thread-local `Ui` state, `init` / `clear` / `with` | – | – |
| [src/ui/view.rs](../src/ui/view.rs) | 166 | `PanelView`, `NetView` + `update`, re-exports `Memory`, formatting | – | yes |
| [src/ui/layout.rs](../src/ui/layout.rs) | 129 | Design geometry constants, `bar_fill`, `close_button_origin`, `in_hover_zone` | – | yes |
| [src/ui/theme/mod.rs](../src/ui/theme/mod.rs) | 236 | `Theme`, `Palette`, `DARK` / `LIGHT`, `Load`, `system_theme`, `apply_menu_theme` | yes | yes |
| [src/ui/paint.rs](../src/ui/paint.rs) | 328 | `Fonts`, `PanelPaint::draw`, row painters, `close_button` painter | – | – |
| [src/ui/gdi.rs](../src/ui/gdi.rs) | 209 | `PaintGuard`, `BackBuffer`, `Font`, `fill`, `frame`, `round_rect`, `text`, `text_width` | yes | – |
| [src/ui/menu.rs](../src/ui/menu.rs) | 183 | Popup menu build / show, `MenuState`, `Command`, `OPACITY_LEVELS` | yes | yes |
| [src/ui/tray.rs](../src/ui/tray.rs) | 66 | `add`, `remove`, `taskbar_created_message`, `WM_APP_TRAY` | yes | – |
| [src/ui/demo.rs](../src/ui/demo.rs) | 57 | Debug-only `view()` and `theme_override()` | – | – |
| [build.rs](../build.rs) | 19 | `embed-resource` compile of `res/winhelper.rc` with version macros | – | – |

Other: [docs/PLAN.md](PLAN.md) (design, milestones §8, manual tests §9),
[.github/workflows/ci.yml](../.github/workflows/ci.yml) (fmt, clippy `--all-targets -D warnings`,
test, release build), `design/` (Brilliant files; `.brilliant/` is tool metadata, ignore it).

---

## 3. Key types and functions

### Data model ([ui/view.rs](../src/ui/view.rs))
```rust
struct Memory { used: u64, total: u64 }          // bytes; .percent() -> f32 (defined in metrics/memory.rs)
struct NetView { public_ip, lan_ip, location, country_code: Option<String>, stale: bool }
struct PanelView { cpu: Option<f32>, ram: Option<Memory>, gpu: Option<f32>, net: NetView }
fn round_percent(f32) -> u32                     // clamp 0..=100, NaN -> 0
fn percent_text(Option<f32>) -> String           // "42 %" | "N/A"
fn memory_text(Memory) -> String                 // "10.1 / 16.0 GB" (GiB)
NetView::update(&mut self, NetInfo)              // LAN always replaced; public None → keep + stale
```
`None` means unavailable → painted as N/A / "—". `net.stale` grays network rows.
An IP-only answer (ipify) for the same IP keeps the known location.

### Network ([net/mod.rs](../src/net/mod.rs))
```rust
struct Geo { ip: String, location: Option<String>, country_code: Option<String> }  // net/geo.rs
struct NetInfo { lan_ip: Option<String>, public: Option<Geo> }  // public None = all providers failed
Lookup::start(panel: HWND) -> Option<Lookup>     // spawns thread "network"; first lookup at once
Lookup::refresh()                                // "Refresh IP now"; requests during a lookup collapse into one
Lookup::take_latest() -> Option<NetInfo>
impl Drop for Lookup                             // stop + wake; joins if idle, detaches if mid-request
```
Loop: wait on the wake event until `pending`, then `local::lan_ip()` + `geo::lookup(session)`,
store, post `WM_APP_NETINFO`. The WinHTTP session is opened lazily and reused.
Providers in order: `ipwho.is/` (name + code) → `ipinfo.io/json` (city + code) →
`api.ipify.org/` (IP only). IPs are validated with `IpAddr` parsing. LAN IP: IPv4 of an
up, non-loopback adapter, preferring one with a gateway, then lowest `Ipv4Metric`;
APIPA addresses skipped. No lookup in demo mode.

### Metrics ([metrics/mod.rs](../src/metrics/mod.rs))
```rust
struct MetricsSnapshot { cpu: Option<f32>, ram: Option<Memory>, gpu: Option<f32> }
Sampler::start(panel: HWND) -> Option<Sampler>   // spawns thread "metrics"; None → panel stays N/A
Sampler::set_active(bool)                        // fast (1 s) vs slow (10 s); activation wakes the thread
Sampler::take_latest() -> Option<MetricsSnapshot>
impl Drop for Sampler                            // stop flag + wake event, then join
```
Loop: sample when `last + interval` has passed, else `Event::wait` the remainder
(woken early by stop / activation). First sample always after 1 s. CPU = `GetSystemTimes`
delta; RAM = `GlobalMemoryStatusEx` (used = total − avail); GPU = PDH English counter
`\GPU Engine(*)\Utilization Percentage`, summed per engine (`luid_…_phys_N_eng_M`),
max over engines; `GpuSampler::open` → `None` means GPU N/A. No sampler in demo mode.

### UI state ([ui/state.rs](../src/ui/state.rs))
```rust
struct Ui { panel: HWND, close: HWND, dpi: u32, theme: Theme, fonts: Fonts,
            view: PanelView, hovered: bool, close_hovered: bool, focused: bool,
            sampler: Option<Sampler>, net: Option<Lookup>, topmost: bool, opacity: u8 }
fn with<R>(f: impl FnOnce(&mut Ui) -> R) -> Option<R>
```
**Rule:** never call a re-entrant Win32 API (`SetWindowPos`, `ShowWindow`,
`TrackPopupMenu`, …) inside `with`; copy values out, drop the borrow, then call.
`with` returns `None` on re-entry rather than panicking.

### Theme ([ui/theme/mod.rs](../src/ui/theme/mod.rs))
- `hex(0xRRGGBB) -> COLORREF` (byte-swaps to `0x00BBGGRR`).
- `Palette` fields: `panel, border, border_hover, label, value, secondary, stale_label, stale_value, track, track_na, normal, elevated, high, high_value, country, close_bg, close_border, close_glyph, close_hover, close_hover_glyph`.
- `Load::of(pct)`: `0..=59` Normal, `60..=85` Elevated, `>85` High. `bar_color`, `text_color` (only High changes text).
- `system_theme()` reads HKCU `…\Themes\Personalize\AppsUseLightTheme` (missing → Light); debug builds honor `WINHELPER_THEME` first.
- `apply_menu_theme` uses uxtheme ordinals 135/136 (undocumented; no-op if absent).

### Layout ([ui/layout.rs](../src/ui/layout.rs)), all 96-DPI px
Panel `220 × 174`; `PAD_X 12`, `LABEL_WIDTH 28`, `COL_GAP 8`, `VALUE_WIDTH 38`,
`ROW_HEIGHT 16`, bar `6` tall radius `3` (track = 114 px).
Row tops: CPU 10, RAM 33, RAM detail 51 (h 13), GPU 71, separator 94, IP 102,
LAN 125, LOC 148. Close button `24` square, gap `6`, glyph `12`.

### Fonts ([ui/paint.rs](../src/ui/paint.rs) `Fonts`)
`label` Segoe UI 12 px semibold · `text` Segoe UI 12 px · `tag` Segoe UI 10 px semibold ·
`value` Consolas 12 px · `detail` Consolas 11 px · `glyph` Segoe MDL2 Assets 12 px.
Recreated on `WM_DPICHANGED`.

### Painting
`window::on_paint` → `gdi::PaintGuard` + `gdi::BackBuffer` → `PanelPaint { width, height, dpi, palette, fonts, view, hovered }.draw(dc)` → blit.
Internal helpers: `Px` (scales design coords), `Columns` (label / bar / value x-ranges),
`metric_row`, `net_row`, `location_row`, `net_colors`.

### Window creation (`window::create`, ~L66)
Class `WinHelper.Panel`, `CS_DBLCLKS | CS_DROPSHADOW`; `WS_POPUP` with
`WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TOPMOST`. Then: close button →
`state::init` → menu theme → opacity 100 → rounded corners → border → tray →
`place_initial` (top-right of the primary work area, `SCREEN_MARGIN` 16) → `SW_SHOWNOACTIVATE`.
Whole client area returns `HTCAPTION` (drag anywhere), so mouse moves / right
clicks arrive as `WM_NC*` messages.

---

## 4. Messages, IDs, constants

| Name | Value | Defined in | Handled in |
|---|---|---|---|
| `WM_APP_TRAY` | `WM_APP + 1` | tray.rs | window.rs `wnd_proc`: R-click → menu, dbl-click → toggle visible |
| `WM_APP_HIDE_PANEL` | `WM_APP + 2` | close_button.rs | window.rs → `set_visible(false)` |
| `WM_APP_METRICS` | `WM_APP + 3` | metrics/mod.rs | window.rs `on_metrics`: copy snapshot into `view`, repaint if changed |
| `WM_APP_NETINFO` | `WM_APP + 4` | net/mod.rs | window.rs `on_netinfo`: `NetView::update`, repaint if changed |
| `TaskbarCreated` | registered | tray.rs | window.rs: re-add tray icon |
| `HOVER_TIMER` | timer id 1, 100 ms | window.rs | `on_hover_timer` |
| `IDI_APP` | resource 1 | window.rs / res/winhelper.rc | class icon, tray icon |
| `CMD_TOPMOST..CMD_EXIT` | 1–5 | menu.rs | `menu::command` → `Command` |
| `CMD_OPACITY_BASE` | 100 (+ level) | menu.rs | `Command::Opacity(u8)`; levels `[100, 85, 70]` |
| Mutex | `Local\WinHelper.SingleInstance` | app.rs | – |
| Window classes | `WinHelper.Panel`, `WinHelper.Close` | window.rs, close_button.rs | – |

Refresh IP (`CMD_REFRESH_IP` → `Command::RefreshIp` → `Lookup::refresh`) is enabled
when the lookup thread runs (`MenuState::can_refresh_ip`; grayed in demo mode).
Copy IP (`CMD_COPY_IP`) and Autostart (`CMD_AUTOSTART`) exist but are grayed: wire them up in M3 / M4.

`wnd_proc` cases: `WM_NCHITTEST`, `WM_NCMOUSEMOVE`, `WM_TIMER`, `WM_MOVE`,
`WM_NCRBUTTONUP`, `WM_CONTEXTMENU`, `WM_APP_TRAY`, `WM_APP_HIDE_PANEL`,
`WM_APP_METRICS`, `WM_APP_NETINFO`, `WM_ACTIVATE` (sets `focused`, updates sampling rate),
`WM_SETTINGCHANGE` (theme), `WM_ERASEBKGND`, `WM_PAINT`, `WM_DPICHANGED`,
`WM_DESTROY` (kill timer, remove tray, stop sampler + lookup, `state::clear`, quit), `TaskbarCreated`.

---

## 5. Tests (pure logic only)

| Module | Covers |
|---|---|
| ui/mod.rs | `scale`: identity, 125/150/200 %, rounding, zero DPI |
| ui/view.rs | percent rounding/clamping, GiB formatting, `NetView::update` (success → failure → success, IP-only answer keeps / drops location) |
| net/geo.rs | ipwho.is / ipinfo / ipify parsing: full replies, `success:false`, missing / blank fields, invalid IP, bad country code, non-JSON |
| net/local.rs | `choose`: gateway first, then metric; skips down / loopback / APIPA; empty → `None` |
| net/mod.rs | `live::looks_up_real_network` is `#[ignore]` (hits the real providers) |
| metrics/mod.rs | `remaining` (interval due math); `live::samples_real_counters` is `#[ignore]` — run with `cargo test -- --ignored --nocapture` |
| metrics/cpu.rs | `usage` delta math: busy share, idle / full, zero / backwards deltas |
| metrics/memory.rs | `Memory::percent`, zero total |
| metrics/gpu.rs | `engine_key` parsing, sum per engine + max, same-type engines kept apart, clamp / NaN, empty → `None` |
| ui/layout.rs | track width = 114, `bar_fill`, close button left/right flip, hover bridge |
| ui/theme/mod.rs | `hex` packing, `Load` thresholds, high-load text color |
| ui/menu.rs | command id → `Command` mapping |

---

## 6. Not built yet (planned homes, PLAN.md §4)

| Planned file | Milestone | Will hold |
|---|---|---|
| `src/net/*` additions | M3 | 10 min periodic refresh, `NotifyIpInterfaceChange` (debounced) → `Lookup::refresh`, error text for the offline tooltip, Copy IP |
| `src/config.rs` | M4 | Settings load/save (serde_json), autostart, setting to disable the public IP lookup |

`serde_json` is already a dependency (only `Value` is used; default features off, `std` on);
settings with derive will also need `serde` with `derive`.
