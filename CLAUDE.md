# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project

WinHelper is a tiny native Windows 10/11 desktop panel (Rust + raw Win32 via the `windows` crate) showing CPU / RAM / GPU load plus public IP, LAN IP and geo-location of the public IP.

The full design lives in [docs/PLAN.md](docs/PLAN.md). Read it before making architectural decisions, and keep it in sync if a decision changes.

**Code map:** [docs/project-index.md](docs/project-index.md) indexes every file, key type, message ID and test. Check it first to find where a change goes, then read only the files and line ranges it points to. Update it in the same change whenever modules, public items, messages, menu commands or tests change.

**Status:** M0 (skeleton) done. The visual design from `design/WinHelper.bl` is implemented ahead of the data sources: the full panel layout in dark/light (follows `AppsUseLightTheme`), load colors, N/A and stale states, hover border and close button, a tray icon (double-click toggles), and the themed context menu with always-on-top and opacity. Its data comes from `ui::view::PanelView`, which stays empty until the metrics and network threads fill it. Debug builds accept `WINHELPER_DEMO=normal|high|offline` and `WINHELPER_THEME=dark|light` to preview design states. Not done yet: the offline tooltip (M3), the Refresh/Copy IP and autostart items (shown grayed until M3/M4), and settings persistence (M4). Next: M1 (CPU + RAM). Follow the milestones in PLAN.md §8.

## Commands

```sh
cargo build                      # debug build
cargo build --release            # size-optimized release build
cargo run                        # launch the panel
cargo test                       # unit tests (pure logic only)
cargo fmt --check
cargo clippy -- -D warnings
```

CI runs fmt, clippy (`-D warnings`), test and release build on `windows-latest`. Code must pass all four before a change is done.

Primary target: `x86_64-pc-windows-msvc`. Optional: `aarch64-pc-windows-msvc`.

## Hard constraints

- **No GUI frameworks, web views, or async runtimes.** Win32 + GDI only. No `tokio`, `reqwest`, `winit`, `egui`, etc. HTTP goes through WinHTTP.
- **Footprint budget:** release binary < 1 MB, RAM < 10 MB, idle CPU < 0.5 %. Think twice before adding any dependency; current allowed set is `windows`, `serde`, `serde_json`, and `embed-resource` 3.x (build). `serde`/`serde_json` are not added yet — add them when settings/geo parsing land.
- Enable only the `windows` crate features actually used (keeps compile time down). Add new features to `Cargo.toml` explicitly when needed.
- Edition 2021, stable Rust.

## Architecture (summary)

- **UI thread** owns the window, message loop, painting, tray and menu. It never blocks on I/O.
- **Metrics thread** samples CPU/RAM/GPU every 1 s, writes `MetricsSnapshot` into `Arc<Mutex<...>>`, posts `WM_APP_METRICS`.
- **Network thread** refreshes public IP + geo on start, every 10 min, on network change (debounced ~2 s) or manual refresh; posts `WM_APP_NETINFO`.
- Shutdown: shared `AtomicBool` stop flag + wake event; workers joined in `WM_DESTROY`.

Planned module layout: `src/{main,app,config}.rs`, `src/ui/{window,paint,tray,menu}.rs`, `src/metrics/{mod,cpu,memory,gpu}.rs`, `src/net/{mod,http,geo,local}.rs`, plus `build.rs` and `res/` (manifest, icon). See PLAN.md §4.

## Coding conventions

- Keep `unsafe` confined to thin wrapper modules. Wrap Win32 handles (HQUERY, HINTERNET, HDC, HFONT, HKEY, …) in RAII types that release them in `Drop`.
- Separate pure logic from Win32 calls so it can be unit tested without the OS: CPU delta math, GPU instance-name parsing/aggregation, geo JSON parsing, settings round-trip.
- Degrade, don't crash: missing GPU counters → `N/A`; network failure → keep last values grayed out with the error in the tooltip; corrupt settings → defaults.
- Use English PDH counter names (`PdhAddEnglishCounterW`) so localized Windows works.
- Use wide-string (`W`) Win32 APIs throughout.
- Scale all pixel metrics by DPI (`GetDpiForWindow`); handle `WM_DPICHANGED`.
- Public IP lookup sends the user's IP to a third party: it must stay disableable via settings and documented in the README.

## Environment notes

- Development is on Windows; the shell may be PowerShell or Git Bash.
- Manual verification (Task Manager comparison, multi-monitor DPI, VPN/Wi-Fi switching, offline mode) is listed in PLAN.md §9. Mention which of these a change still needs when it can't be covered by unit tests.
