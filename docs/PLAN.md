# WinHelper — Implementation Plan

## 1. Goal

WinHelper is a small, always-available desktop panel for Windows 10/11 that shows at a glance:

- **System load**: CPU usage, memory usage, GPU usage, disk activity (active time, read / write rates), CPU and GPU temperature
- **Network**: internet status, download / upload speed, current public IP, local (LAN) IP, and the approximate geographic location of the public IP

Non-goals for v1: historical logging, per-process breakdown, alerts, multi-GPU detail views, cross-platform support.

### Guiding principles

- **Native and lightweight**: Rust + raw Win32 API (via the `windows` crate). No web view, no GUI framework.
- **Small footprint**: target release binary < 1 MB, RAM < 10 MB, CPU < 0.5 % at idle.
- **Unobtrusive**: borderless, compact, draggable, optional always-on-top, lives in the tray.

---

## 2. UI concept

The visual spec (exact sizes, colors, all states, close button, menu) lives in `design/WinHelper.bl` (Brilliant), board **"WinHelper · Minimal"**. The older multi-row panel design on the same canvas is superseded.

The panel is a minimalist one-line strip, 30 px tall at 96 DPI (scaled for DPI) and 452 px wide, plus 26 px for each temperature sensor that reports (504 px with both). Each item is only an icon and a value; CPU and GPU add a dim temperature:

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ▣ 42% 58°  ▤ 63%  ▭ 18% 51°  ▬ 12%  │  ◍ •  ↓ 1.2 MB/s  ↑  84 KB/s      │
└──────────────────────────────────────────────────────────────────────────┘
  CPU        RAM    GPU        Disk      internet   download   upload
```

Everything else lives in a **detail tooltip** (264 × 356 px) that opens below the strip after the cursor rests on it for 400 ms:

```
┌──────────────────────────────────┐
│ SYSTEM                           │
│ ▣ CPU               58 °C  42 %  │
│ ████████░░░░░░░░░░░░░░░░░░░░░░░  │
│ ▤ RAM        10.1 / 16.0 GB 63 % │
│ ████████████░░░░░░░░░░░░░░░░░░░  │
│ ▭ GPU               51 °C  18 %  │
│ ███░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │
│ ▬ Disk  R 4.1 · W 0.3 MB/s 12 %  │
│ ██░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │
│──────────────────────────────────│
│ NETWORK                ● Online  │
│ Public IP          203.0.113.42  │
│ Location       Kyiv, Ukraine UA  │
│ LAN IP             192.168.1.15  │
│ ↓ Download             1.2 MB/s  │
│ ↑ Upload                84 KB/s  │
│──────────────────────────────────│
│ IP updated 14:02 · right-click…  │
└──────────────────────────────────┘
```

Behavior:

- Drag anywhere to move. Position is saved and restored on next launch.
- Hovering the strip lightens its border and shows a 22 px close button 6 px beside it, vertically centered (flipped to the left near the work-area edge). Clicking it hides the strip to the tray; Exit stays in the menu.
- The tooltip opens after 400 ms of hover, below the strip (above it near the bottom of the work area, shifted to stay inside it horizontally). It follows the strip when dragged, refreshes live while open, is click-through, and closes when the hover ends or the context menu opens.
- Icons are Segoe MDL2 Assets glyphs (also in Segoe Fluent Icons): CPU `E950`, RAM `E964`, GPU `E7F4`, disk `EDA2`, globe `E774`, arrows `E74B` / `E74A`. Numbers sit in fixed-width slots so the strip never shifts.
- Temperatures (°C) follow the CPU / GPU percentage, dimmed like a unit: amber at 75–89 °C, red at 90 °C and up. Without a readable sensor the temperature and its slot are left out; the strip resizes (keeping its right edge) when a sensor appears or disappears.
- Disk shows active time with the same load colors; the tooltip adds read / write per second over all physical disks, both in the unit of the larger rate.
- Internet status: green dot when online, red globe and dot when offline, gray while unknown. The transfer rates gray out while offline.
- Right-click (panel or tray icon) opens a context menu:
  - Always on top (toggle)
  - Opacity: 100 % / 85 % / 70 %
  - Refresh IP now
  - Copy IP to clipboard
  - Start with Windows (toggle)
  - Exit
- Double-click the tray icon to show or hide the panel.
- Load colors: on the strip the value stays neutral below 60 %, turns amber from 60 to 85 % and red above 85 % (the icon turns red too). Tooltip bars are green / amber / red at the same thresholds.
- Follows the system light/dark theme (read `AppsUseLightTheme` from the registry).

---

## 3. Technology stack

| Concern | Choice | Notes |
|---|---|---|
| Language | Rust (stable, edition 2021) | |
| Win32 bindings | [`windows`](https://crates.io/crates/windows) crate | Enable only the needed features to keep compile time down |
| Window and rendering | Win32 `CreateWindowExW` + GDI | Double-buffered painting into a memory DC |
| CPU and RAM | `GetSystemTimes`, `GlobalMemoryStatusEx` | Optionally PDH for Task Manager parity |
| GPU | PDH (Performance Data Helper) counters | `\GPU Engine(*)\Utilization Percentage` |
| Disk | PDH | `\PhysicalDisk(_Total)\% Idle Time`, `Disk Read / Write Bytes/sec` |
| CPU temperature | PDH | `\Thermal Zone Information(*)\High Precision Temperature` (ACPI zones, no admin) |
| GPU temperature | D3DKMT (`Wdk_Graphics_Direct3D`) | `D3DKMTQueryAdapterInfo(KMTQAITYPE_ADAPTERPERFDATA)`, WDDM 2.4+ |
| HTTP | WinHTTP (`WinHttpOpen`/`WinHttpSendRequest`…) | Avoids `reqwest`/`tokio` and their binary size |
| JSON | `serde_json` + `serde` (or a tiny manual parser) | Responses are small and flat |
| Local IP | `GetAdaptersAddresses` (IP Helper) | |
| Throughput | `GetIfTable2` (IP Helper) | Byte-counter deltas of physical interfaces |
| Internet status | `GetNetworkConnectivityHint` (IP Helper, Win10 2004+) | Looked up at run time; falls back to the IP lookup result |
| Network change events | `NotifyIpInterfaceChange` | Triggers an IP refresh |
| Tray icon | `Shell_NotifyIconW` | |
| Settings | INI/TOML file in `%APPDATA%\WinHelper\` | Or registry `HKCU\Software\WinHelper` |
| Autostart | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | |
| Manifest/resources | `embed-resource` or `winres` build dependency | DPI awareness, icon, version info |

---

## 4. Architecture

```
┌────────────────────┐   WM_APP_METRICS    ┌─────────────────────┐
│  Metrics thread    │ ──────────────────▶ │                     │
│  (1 s / 10 s)      │   PostMessageW      │   UI thread         │
│  CPU / RAM / GPU,  │                     │   (message loop,    │
│  traffic, online   │                     │                     │
└────────────────────┘                     │    painting, tray,  │
┌────────────────────┐   WM_APP_NETINFO    │    context menu)    │
│  Network thread    │ ──────────────────▶ │                     │
│  public IP + geo   │                     └─────────────────────┘
│  (on demand, every │            ▲
│  10 min, on change)│            │ shared state: Arc<Mutex<Snapshot>>
└────────────────────┘
```

- **UI thread** owns the window and only paints. It never blocks on I/O.
- **Metrics thread** samples CPU / RAM / GPU, network throughput and internet connectivity every 1 s while the panel is focused or hovered and every 10 s otherwise (the first sample always comes after 1 s). When Windows reports internet access again after being offline, the UI asks the network thread for a new IP lookup. It writes a `MetricsSnapshot` into shared state, then posts `WM_APP_METRICS`, which leads to `InvalidateRect`. The UI flips the rate via `Sampler::set_active`; becoming active wakes the thread so it samples at once if the last sample is ≥ 1 s old. Samples are never closer than 1 s, so CPU/GPU deltas stay meaningful.
- **Network thread** waits on a channel or event (startup, timer, network change callback, or "Refresh" menu command), performs the HTTP lookup with a timeout, and posts `WM_APP_NETINFO`.
- Shutdown: a shared `AtomicBool` stop flag plus an event to wake the workers. The threads are joined in `WM_DESTROY`.

### Module layout

```
winhelper/
├─ Cargo.toml
├─ build.rs                 # embeds manifest + icon + version info
├─ res/
│  ├─ app.manifest          # PerMonitorV2 DPI, Common Controls v6
│  └─ winhelper.ico
└─ src/
   ├─ main.rs               # entry, #![windows_subsystem = "windows"]
   ├─ win.rs                # RAII handle wrappers, single-instance mutex, error box
   ├─ app.rs                # App state, startup/shutdown orchestration
   ├─ ui/
   │  ├─ window.rs          # strip class, WndProc, drag, hover, DPI, theme changes
   │  ├─ close_button.rs    # hover close popup owned by the strip
   │  ├─ tooltip.rs         # detail tooltip popup owned by the strip
   │  ├─ state.rs           # UI-thread state shared by the window procedures
   │  ├─ view.rs            # PanelView model + value formatting
   │  ├─ layout.rs          # design metrics, close-button / tooltip placement math
   │  ├─ theme/mod.rs       # dark/light palettes, load thresholds, menu theming
   │  ├─ paint.rs           # strip, tooltip and close-button drawing
   │  ├─ gdi.rs             # GDI RAII wrappers and draw helpers
   │  ├─ demo.rs            # debug-only sample data (WINHELPER_DEMO / WINHELPER_THEME)
   │  ├─ tray.rs            # Shell_NotifyIcon
   │  └─ menu.rs            # context menu (panel and tray)
   ├─ metrics/
   │  ├─ mod.rs             # MetricsSnapshot, sampler loop
   │  ├─ cpu.rs
   │  ├─ memory.rs
   │  ├─ gpu.rs             # PDH-based GPU utilization, D3DKMT GPU temperature
   │  ├─ disk.rs            # PDH PhysicalDisk active time and read / write rates
   │  ├─ thermal.rs         # PDH thermal zones → CPU temperature
   │  ├─ pdh.rs             # PDH query RAII + value / array helpers
   │  ├─ traffic.rs         # GetIfTable2 byte-counter rates
   │  └─ connectivity.rs    # GetNetworkConnectivityHint (loaded at run time)
   ├─ net/
   │  ├─ mod.rs             # NetInfo, refresh loop
   │  ├─ http.rs            # minimal WinHTTP GET wrapper
   │  ├─ geo.rs             # provider(s) + JSON parsing + fallback
   │  └─ local.rs           # GetAdaptersAddresses, change notifications
   └─ config.rs             # load/save settings, autostart toggle
```

---

## 5. Data sources in detail

### 5.1 Memory

- `GlobalMemoryStatusEx` → `dwMemoryLoad` (%), `ullTotalPhys`, `ullAvailPhys`.
- Display the percentage and "used / total GB".

### 5.2 CPU

- **Option A (simple)**: call `GetSystemTimes` twice. Then
  `usage = 1 − Δidle / (Δkernel + Δuser)` (kernel time includes idle).
- **Option B (Task Manager parity)**: PDH counter
  `\Processor Information(_Total)\% Processor Utility`, which accounts for turbo/frequency scaling the way Task Manager does.
- Plan: start with A, add B behind a setting if the numbers visibly differ from Task Manager.

### 5.3 GPU

Windows exposes GPU load via PDH (WDDM 2.0+, Windows 10 1709+):

1. `PdhOpenQueryW`
2. `PdhAddEnglishCounterW(query, "\\GPU Engine(*)\\Utilization Percentage")`. Use the *English* variant so the code works on localized Windows.
3. Call `PdhCollectQueryData` once on init, then every tick.
4. `PdhGetFormattedCounterArrayW(PDH_FMT_DOUBLE)` → instances named like
   `pid_1234_luid_0x..._phys_0_eng_3_engtype_3D`.
5. Aggregate as Task Manager does:
   - group by engine (`luid` + `phys` + `eng` index), **summing** across processes. Grouping by `engtype` instead would merge, and so overcount, two engines of the same type (e.g. two Copy engines);
   - per adapter, take the **max** over its engines (3D, Compute, VideoDecode, Copy, …);
   - show the busiest adapter. Preferring the discrete adapter on hybrid iGPU + dGPU systems is not implemented.
6. Clamp to 0–100 %.

Optional extra: VRAM usage via `\GPU Adapter Memory(*)\Dedicated Usage`, with the adapter total from DXGI `IDXGIAdapter3::QueryVideoMemoryInfo`. Adapter names come from DXGI (`EnumAdapters1` → `DXGI_ADAPTER_DESC1`, matched by LUID).

If the counters are unavailable (old driver, RDP session, VM), show `GPU  N/A` instead of failing.

**Temperature**: adapters from `D3DKMTEnumAdapters2` (handles opened once, closed on drop), then `D3DKMTQueryAdapterInfo(KMTQAITYPE_ADAPTERPERFDATA)` per adapter; `Temperature` is in tenths of °C. Adapters reading 0 (no sensor, e.g. Microsoft Basic Render Driver) or outside 1–150 °C are skipped; the hottest one is shown. None left → the temperature is hidden.

### 5.3a Disk

- PDH `\PhysicalDisk(_Total)\% Idle Time`, `\Disk Read Bytes/sec`, `\Disk Write Bytes/sec` in one query (baseline collection at open, rates averaged over the sampling interval).
- Active time = 100 − idle, clamped to 0–100 % (Task Manager's "Active time"; `_Total` averages the disks). Missing counters (`diskperf -N`) → Disk `N/A`.

### 5.3b CPU temperature

Windows has no documented per-core sensor API. The ACPI thermal zones are readable without admin through PDH `\Thermal Zone Information(*)\High Precision Temperature` (tenths of a kelvin; falls back to `Temperature` in kelvin). The hottest zone is shown. Caveats:

- On many desktops a zone is a motherboard or package sensor, not the hottest core, so it reads lower than vendor tools.
- Many boards expose placeholder zones stuck at a constant value (typically 301 K = 27.85 °C). A zone only counts once its reading has changed, so the temperature appears a few samples after start, and never on machines with only placeholder zones.
- WMI `MSAcpi_ThermalZoneTemperature` needs admin and reads the same zones; vendor drivers (e.g. a ring-0 MSR reader) are out of scope.

### 5.4 Local IP

- `GetAdaptersAddresses(AF_INET, GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER | GAA_FLAG_INCLUDE_GATEWAYS)`.
- Choose the adapter with `OperStatus == IfOperStatusUp`, not loopback, preferring one with a default gateway, then the lowest `Ipv4Metric`. APIPA (169.254/16) addresses are skipped. IPv4 only for now; showing IPv6 in the tooltip needs an `AF_UNSPEC` query.
- `NotifyIpInterfaceChange` notifies the network thread on Wi-Fi switch, VPN connect, and similar events. Debounce for about 2 s before refreshing.

### 5.5 Public IP and location

A single HTTPS request returns both the IP and the location:

| Provider | Endpoint | Notes |
|---|---|---|
| **Primary**: ipwho.is | `https://ipwho.is/` | `ip`, `city`, `country`, `country_code`, `connection.isp`; `success: false` on errors |
| **Fallback**: ipinfo.io | `https://ipinfo.io/json` | `ip`, `city`, `region`, `country`, `org`; free tier without a token, rate limited. `country` is only the code, so the location shows just the city |
| IP-only fallback | `https://api.ipify.org` | Used when both geo providers fail, so at least the IP is shown |

ipwho.is is primary because it returns the country name, which the "City, Country" row needs; ipinfo.io returns only the code.

- Refresh triggers: app start (1 s after the panel is shown, so startup never waits on the network), every 10 minutes, a network change event, or the manual "Refresh IP" command. Implemented so far: app start, offline → online and the manual command.
- Timeouts: 5 s each for resolve, connect, send and receive (`WinHttpSetTimeouts`). The system proxy is used (`WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY`).
- Shutdown does not wait for a lookup that is mid-request; the thread is detached and ends with the process.
- On failure, keep the last known values, gray them out, and show the error in the tooltip.
- Make the provider list configurable in settings.
- **Privacy**: the request reveals the user's IP to the third-party provider. State this in the README and add a setting to disable the public IP and location lookup.

### 5.6 Throughput and internet status

- **Throughput**: `GetIfTable2` each sample; sum the `InOctets` / `OutOctets` deltas of interfaces that are up, have `HardwareInterface` set, are not filter interfaces and are not loopback, then divide by the elapsed time. Filter and virtual interfaces (WFP filters, Hyper-V vEthernet, VPN adapters) would count the same bytes again; VPN traffic is still counted once, on the physical adapter that carries it. Interfaces are matched by LUID; one that is new or whose counters went backwards (adapter reset) adds nothing for that sample. The rate is an average over the sampling interval, so it is per 1 s while the strip is active and a 10 s average otherwise.
- Display: binary units (1 KB = 1024 B) `B/s`, `KB/s`, `MB/s`, `GB/s`; one decimal below 10, so a number is at most 4 characters and fits its fixed slot.
- **Internet status**: `GetNetworkConnectivityHint` → `InternetAccess` / `ConstrainedInternetAccess` = online, `None` / `LocalAccess` = offline, `Unknown` / `Hidden` = unknown. The function exists from Windows 10 2004, so it is resolved with `GetProcAddress`; when it is missing or the level is unknown, the status falls back to the last IP lookup (stale = offline, a known IP = online).
- The offline → online transition triggers a public IP lookup (a cheap stand-in for the planned `NotifyIpInterfaceChange` trigger).

---

## 6. Window implementation notes

- Window styles: `WS_POPUP` plus `WS_EX_TOOLWINDOW` (no taskbar button), `WS_EX_LAYERED` (opacity via `SetLayeredWindowAttributes`), and optionally `WS_EX_TOPMOST`.
- Dragging: return `HTCAPTION` from `WM_NCHITTEST` for the client area.
- DPI: declare PerMonitorV2 in the manifest, handle `WM_DPICHANGED`, and scale all metrics via `GetDpiForWindow`.
- Painting: in `WM_PAINT`, draw into a compatible memory bitmap and `BitBlt` it to the window. Return 1 from `WM_ERASEBKGND` to avoid flicker.
- Font: `Segoe UI` for labels, `Consolas` for numbers and `Segoe MDL2 Assets` for icons, created once per DPI. The status dot is the `●` text glyph, because GDI shapes are not anti-aliased.
- Tooltip: a separate `WS_POPUP` owned by the strip with `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE` (click-through, never activates) and `CS_DROPSHADOW`. It is shown from the 100 ms hover poll timer once the hover is 400 ms old.
- Rounded corners on Win11: `DwmSetWindowAttribute(DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND)`.
- Single instance: a named mutex `Local\WinHelper.SingleInstance`. If it already exists, bring the existing window to the front and exit.
- Restore the saved position. If it is off-screen (for example, a monitor was disconnected), clamp it to the nearest monitor via `MonitorFromPoint`.

---

## 7. Build configuration

```toml
[package]
name = "winhelper"
version = "0.1.0"
edition = "2021"

[dependencies]
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_UI_WindowsAndMessaging",
  "Win32_UI_Shell",
  "Win32_UI_HiDpi",
  "Win32_Graphics_Gdi",
  "Win32_Graphics_Dwm",
  "Win32_Graphics_Dxgi",
  "Win32_System_Performance",
  "Win32_System_SystemInformation",
  "Win32_System_Threading",
  "Win32_System_Registry",
  "Win32_System_DataExchange",   # clipboard
  "Win32_NetworkManagement_IpHelper",
  "Win32_NetworkManagement_Ndis",
  "Win32_Networking_WinHttp",
  "Win32_Networking_WinSock",
] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[build-dependencies]
embed-resource = "3"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

Targets: `x86_64-pc-windows-msvc` (primary) and `aarch64-pc-windows-msvc` (optional).

---

## 8. Milestones

| # | Milestone | Deliverable |
|---|---|---|
| M0 | Project skeleton | `cargo new`, `windows` crate, manifest/icon embedding, empty borderless draggable window, single instance |
| M1 | CPU and RAM | Metrics thread, snapshot posting, GDI bars and text, 1 s refresh when active / 10 s otherwise |
| M2 | GPU | PDH wildcard query, per-adapter aggregation, N/A fallback; verified against Task Manager |
| M3 | Network | WinHTTP wrapper, provider and fallback chain, local IP, change notifications, manual refresh |
| M4 | Shell integration | Tray icon, context menu, always-on-top, opacity, copy IP, autostart, settings persistence |
| M5 | Polish | DPI scaling, dark/light theme, Win11 rounded corners, tooltips, color thresholds |
| M6 | Release | Size optimization, README with screenshots, GitHub Actions build that publishes a signed (optional) `.exe` |

---

## 9. Testing strategy

- **Unit tests** (pure logic, no Win32):
  - CPU delta math from fixed `FILETIME` inputs
  - GPU instance-name parsing and aggregation (sum per engine, max per adapter)
  - JSON parsing for each geo provider, including missing fields
  - Settings load/save round-trip and defaults when the file is corrupt
- **Integration and manual checks**:
  - Compare CPU, RAM, and GPU values against Task Manager under load (e.g. a stress tool or a game)
  - Compare disk active time and read / write rates with Task Manager's Disk graphs during a large file copy; compare the GPU temperature with Task Manager's GPU page and the CPU temperature with a vendor tool (expect the ACPI zone to read lower); on a machine without sensors the temperature slots stay hidden
  - Switch Wi-Fi ↔ Ethernet and connect/disconnect a VPN; the IP and location should update within about 5 s
  - Run offline; the app should keep running and show stale or N/A values, a red globe and grayed rates; reconnecting should turn the dot green and refresh the IP
  - Compare download / upload speed with Task Manager's Ethernet / Wi-Fi graphs during a large download, also with a VPN connected (no double counting)
  - Hover the strip near each screen edge: the tooltip opens after ~400 ms, flips above at the bottom edge, stays on screen, follows a drag, and hides when the cursor leaves or the menu opens
  - Test multi-monitor and mixed DPI (100 % / 150 %): drag between monitors and check that rendering stays crisp
  - Test with a localized (non-English) Windows UI to confirm the PDH English counter names work
- **CI**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build --release` on `windows-latest`.

---

## 10. Risks and mitigations

| Risk | Mitigation |
|---|---|
| GPU PDH counters missing (old WDDM, VM, RDP) | Detect at startup and show `N/A`; never crash |
| GPU numbers differ from Task Manager | Use the same aggregation (sum per engine, max across engines); document it |
| Geo provider rate limits or outages | Fallback chain, 10 min cache, manual refresh only on demand |
| VPN or proxy affects the result | This is expected (it shows the egress IP); WinHTTP respects the system proxy via `WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY` |
| Antivirus false positives on a small unsigned exe | Optional code signing; avoid packers such as UPX |
| `unsafe` Win32 code bugs | Keep `unsafe` in thin wrapper modules with RAII guards for handles (HQUERY, HINTERNET, HDC, HFONT) |

---

## 11. Future ideas (post-v1)

- Sparkline history (last 60 s) for each metric, in the tooltip
- Disk free space
- Per-core CPU temperature (needs a vendor driver; the ACPI zone reading is what v1 shows)
- Click-through mode (`WS_EX_TRANSPARENT`) for gaming overlays
