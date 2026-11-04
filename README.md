# Ferrum Browser

A desktop web browser written from scratch in Rust, built phase by phase.

**Phase 1 — Foundation** is complete:

- a native desktop window (eframe / egui),
- an address bar with real URL parsing and validation,
- back / forward / reload navigation with history,
- a status line that explains invalid input instead of failing silently.

Page rendering is intentionally **not** implemented yet — Phase 1 only proves
the shell and the navigation model, and shows the parsed URL so you can verify
each part.

---

## Running it

```powershell
cargo run            # debug build (console window stays visible)
cargo run --release  # optimised build (no console window)
cargo test           # address-bar parser tests
```

The first build compiles egui and winit, so expect a few minutes. Later builds
are incremental and fast.

### Toolchain notes (this machine)

Rust was installed with the **GNU** toolchain
(`stable-x86_64-pc-windows-gnu`) together with WinLibs GCC, so no Visual Studio
Build Tools are required. If `cargo` is "not recognised" in a fresh terminal,
open a new terminal or add these folders to `PATH`:

```
%USERPROFILE%\.cargo\bin
...\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_*\mingw64\bin
```

---

## Project layout

```
browser/
├── Cargo.toml        # package metadata + dependencies
├── README.md
└── src/
    ├── main.rs       # entry point: window options, event loop
    ├── app.rs        # BrowserApp: state, toolbar, content, styling
    └── url_input.rs  # address parsing/validation + unit tests
```

That split is deliberate. `main.rs` should stay boring: process setup only.
Anything you want to unit-test (like URL parsing) must live outside `main.rs`,
because a binary crate's `main.rs` is awkward to test from the outside.

---

## `Cargo.toml`, explained

```toml
[package]
name = "ferrum-browser"   # crate + executable name
version = "0.1.0"         # semver, used when publishing
edition = "2024"          # language edition; opt-in syntax/behaviour changes

[dependencies]
eframe = "0.37"           # window + event loop + egui integration
url = "2"                 # WHATWG-compliant URL parsing
```

- **`[package]`** describes the crate. `name` becomes the default binary name
  (`cargo run` runs `target/debug/ferrum-browser`).
- **`[dependencies]`** lists libraries from crates.io. `"0.37"` means
  "0.37.x, and nothing newer than 0.38" — cargo picks the newest compatible
  release and records the exact version in `Cargo.lock`.
- **`Cargo.lock`** is generated on the first build and pins every transitive
  dependency. Commit it for applications (this project), not for libraries.
- **`[profile.release]`** (optional) tweaks optimised builds, e.g.
  `opt-level = 2` to cut compile time.

`eframe` is the *framework*; `egui` is the *UI library* it draws. eframe owns
the OS window, input, and GPU context, and calls into egui for widgets.

---

## Application lifecycle

```
main()
  └─ eframe::run_native(app_name, options, creator)
       ├─ creates the native window + graphics context
       ├─ creator(cc)  -> BrowserApp::new(cc)      // once
       ├─ loop:
       │    ├─ App::logic(ctx)                     // non-UI per-frame work
       │    └─ App::ui(ui, frame)                  // rebuild the interface
       ├─ App::save(storage)                       // persistence (optional)
       └─ App::on_exit(gl)                         // once, on shutdown
```

Key ideas:

1. **Immediate mode.** There are no retained widgets. `App::ui` runs every frame
   and rebuilds the toolbar and content from `BrowserApp`'s fields. "Updating
   the UI" therefore just means mutating state; the next frame reflects it.
2. **State lives in the app struct.** `address`, `current`, `history`, `cursor`,
   `error` — everything that must survive between frames.
3. **eframe calls you, you never call it.** `run_native` blocks. You influence
   the window through `NativeOptions` / viewport commands, not by creating
   windows yourself.

---

## What Phase 1 implements

### Address bar and validation (`url_input.rs`)

`parse_address` applies these rules in order:

| Input | Result |
| --- | --- |
| `` (empty) | `Empty` — "Type a web address to get started." |
| `rust tutorial` | `Spaces` — addresses can't contain whitespace |
| `hello` | `NotAnAddress` — no dot, colon, or `localhost` |
| `example.com` | `https://example.com/` (scheme added) |
| `localhost:3000/app` | `https://localhost:3000/app` |
| `http://example.com` | kept as `http` |
| `ftp://example.com` | `UnsupportedScheme("ftp")` |
| `javascript:alert(1)` | `UnsupportedScheme` — never executed |

The parser returns a typed `AddressError`, and the UI renders its `Display`
text in the status bar in red. Errors are values, not panics — the app keeps
running and the user keeps their text to edit.

### Navigation buttons

- **← / →** walk the `history` vector via `cursor`. They are disabled (greyed)
  when there is nowhere to go, computed each frame from state.
- **⟳** re-runs the current page's load path. In Phase 1 that only bumps the
  load counter, which is the seam a real engine will plug into.
- Opening a new address after going back truncates the forward branch, exactly
  like a real browser.
- Toolbar keyboard shortcuts: `Ctrl+L` (focus address bar), `Alt+←`, `Alt+→`,
  `F5`, and `Enter` to submit.

### Styling

`build_style()` customises egui's `Style`: dark palette, larger type scale,
tighter spacing. This runs once in `BrowserApp::new`, because egui keeps styles
in the context, not in the widget tree.

---

## What's next (Phase 2 and beyond)

1. A real web view: embed a rendering engine (e.g. WebView2 on Windows) or build
   a tiny HTML/CSS renderer on top of egui's painter.
2. Tabs, favicons, and per-tab history.
3. Loading state, progress, and error pages for failed navigations.
4. Persistence: restore the session with `eframe::Storage`.
