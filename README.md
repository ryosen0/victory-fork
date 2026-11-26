# ryosen0/victory-fork — personal build (NVIDIA RTX 3060 Ti / Ryzen 7 9800X3D)

A personal product fork of [VIctory](https://github.com/Marvisak/VIctory), a
compatibility layer that runs an Xbox Series title natively on Windows from
files the user supplies. This is **not** an upstream-contributing fork: it is
a dedicated product branch, assembled from upstream `main` plus verified race
fixes, and tuned specifically for **one machine**:

> **Windows 11, AMD Ryzen 7 9800X3D, NVIDIA GeForce RTX 3060 Ti 8 GB (Ampere,
> 32-lane waves), 32 GB RAM, driver 617.42+. Series X profile.**

Every change lands here only after an A/B run on that card proves a gain (or
a stability/visual fix) with zero regression; dead flags are pruned promptly.
*One commit = one change = one measured result.*

**This repository contains no game files, code or assets.** You need your own
copy of the game. It is not affiliated with or endorsed by the game's
developer or publisher.

---

## Credits & Community Attribution

- **[VIctory](https://github.com/Marvisak/VIctory)** by Marvisak and its
  contributors is the foundation of everything here: the loader, the runtime
  services, the Direct3D 12 backend and the shader recompiler. Licensed
  GPL-3.0-or-later; so is this fork. Upstream's history is kept intact below
  the commits that carry this fork's changes.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork)** —
  the upstream tracking branch; key performance and descriptor patches come
  from its `main` and active PRs.
- **[Steenax6/VIctory-NVID](https://github.com/Steenax6/VIctory-NVID)** —
  tested on Ryzen 7 9800X3D + NVIDIA Ampere; `CardState` subresource sync
  (PR #1934 by AndyLatimer/zondy1i) and NVIDIA wave32 tuning.
- **[codec-d/cool_fox](https://git.lbxmb.fr/moddingarchive/codec-d-cool_fox)** —
  pareto residency manager (`residency.rs`) protecting 32 GB RAM systems
  against Windows 11 commit exhaustion, and fast `WaitOnAddress` fence wakes.
- **[Belobug/VIctory-Belobug-AMD](https://github.com/Belobug/VIctory-Belobug-AMD)** —
  pixel quad reads reaching helper lanes (`03629f3`), sequential clamp barrier
  mechanisms, and wave32 volumetrics (#823).
- **Other key contributors**:
  - Page-view guards by **myokex** (victory-amd `c6dc4d8`, #115 by Spexivus, joroyt, VIctory #1122 by tightfall).
  - Read-beside kernel reads by **Joshua Odom** (`10ea38e`).
  - NVIDIA wave32 host fixes by **Massive-Adrian** (#799).
  - Relay write-back fixes following **AscilCH** (#786 / upstream #2009) and missing-skin measurements by **Drdg263** (#89).
  - Upscaler integration by **ZigZagBoi1/ScarlettPort**.
  - Meshdump tooling and low-address guards from **DexNom** and **ZSlayer**.
- This fork's engineering and synchronization are developed with AI agentic supervision.

---

## Status of this Fork (2026-11-26)

**Breakthrough: Opening Cutscene & Story Prologue Reached and Stable!**  
The game progresses past all introductory titles, survives fullscreen zoom/blur transitions, and enters the narrative garage prologue.

| Stage | State in this Fork | Details |
|---|---|---|
| **Boot, Menus, Start Screen** | **Working, 30 FPS** | Flawless menu navigation, audio playback, and options. |
| **Crash 0x80070057 (Second 54)** | **FIXED (`6b6d3826`)** | Render Target format mismatch during zoom/blur solved dynamically via `VICTORY_TARGET_VARIANTS`. |
| **World & Mesh Flickering** | **FIXED** | Sequential UAV barriers restored (`VICTORY_INDIRECT_GATHER_ELIDE=0`); stale CPU zero reads prevented (`VICTORY_COPY_BACK_ONLY_READBACK=0`). |
| **Opening Cutscenes & Prologue** | **Working, ~2.5–5 FPS** | Full 3D scene rendered: Lucia (denim shorts with stitching), Jason, red sports sedan ("INJECTION") with illuminated taillights, dialogue subtitles (*"You good over there?"*), 1938 draws/frame. |
| **Water Reflections** | **FIXED (`33e456b6`)** | RDNA 2 legacy Fused MAD with denormal flush and zero-factor rule eliminates black core artifacts. |
| **Memory Safety (32 GB RAM)** | **FIXED (`06fe62f6`)** | CoolFox `residency.rs` commit charge monitor with 4/2 GiB reserve and 500ms backoff prevents `E_OUTOFMEMORY`. |
| **Descriptor Lookup Speed** | **ACCELERATED (`596fa099`)** | Thread-local `LOOKUP_CACHE` with `TABLES_CHANGED` epoch counter saves ~14 ms/frame on submission prep. |
| **Shadows & Garage Ambience** | Work in progress | Title relies on Ray-Traced Bounce Light (GI) which is not yet emulated; compensated via `VICTORY_AMBIENT=1.2` and `VICTORY_CARD_WINDOWS_MB=1024`. |

---

## Storage Architecture: Fast SSD vs Mechanical HDD

> [!NOTE]
> **This title targets the Xbox Velocity Architecture (NVMe direct streaming at 2.4–4.8 GB/s).**

Running large-scale open-world assets from a mechanical HDD causes noticeable streaming latency:
- **Mechanical HDD:** Random 4K read throughput (~1 MB/s) with 12–15 ms mechanical seek latency starves DirectStorage texture streams, causing prolonged scene transitions and delayed mipmap streaming.
- **SATA or NVMe SSD:** Sub-millisecond seek times and higher random 4K IOPS allow asset queues to drain smoothly, cutting initial load times dramatically and preventing in-game streaming hitches. An SSD installation is strongly recommended.

---

## A/B Results on NVIDIA RTX 3060 Ti 8GB

| Component / Switch | Mechanism & Measurement | Verdict |
|---|---|---|
| **`VICTORY_TARGET_VARIANTS`** (`6b6d3826`) | Port of upstream `a35c557`. Dynamically matches and creates PSO variants when bound RTV format differs from PSO declaration (e.g. `R10G10B10A2` vs `R11G11B10_FLOAT`). | **In fork, active** (eliminates fatal D3D12 crash `0x80070057`). |
| **`VICTORY_LOOKUP_CACHE`** (`596fa099`) | Port of upstream `32be36d` + `bc6ee68`. Thread-local weak cache answering ~89% of host buffer queries without locking descriptor tables. | **In fork, ON by default** (~14 ms/frame saved). |
| **RDNA 2 Fused MAD** (`33e456b6`) | Port of upstream `f6a09d3`. Exact AMD legacy MAD behavior (`0 * Inf = 0`, denormal flushing). | **In fork, active** (eliminates black water dots). |
| **CoolFox Residency Guard** (`06fe62f6`) | Port of `ab75ac3e`. Monitors `GlobalMemoryStatusEx` commit charge under Windows 11; throttles speculative staging and adds 500ms backoff on `E_OUTOFMEMORY`. | **In fork, active** (prevents memory exhaustion). |
| **`VICTORY_CARD_WINDOWS_MB=1024`** | Drops host-copy layers from video memory to system RAM, reclaiming ~1.5–2 GB VRAM on 8GB cards so world textures (walls, ground) allocate without hitting the 90% budget cap. | **Recommended runtime switch**. |
| **`VICTORY_AMBIENT=1.2`** | Boosts global ambient lighting intensity to compensate for the unbuilt Ray Tracing bounce light in interior/garage scenes. | **Recommended runtime switch**. |
| **`VICTORY_INDIRECT_GATHER_ELIDE=0`** | Retains global UAV barriers between indirect gather passes. | **Default OFF** (eliminates mesh/world flickering). |
| **`VICTORY_COPY_BACK_ONLY_READBACK=0`** | Retains full buffer copy-backs for CPU visibility. | **Default OFF** (prevents stale zero reads). |
| **NVIDIA Wave32 Shaders** (`99ac16e`, `fc09e2a3`) | Volumetrics and SCC lowering for 32-lane Ampere hosts (PR #823). | **In fork, active**. |
| **`VICTORY_DRIVER_WATCH=0`** | Prevents Windows 11 informational Auto HDR events from triggering driver loss watchdogs. | **Recommended switch**. |

---

## Recommended Launch Configuration

To run with optimal performance and visuals on Ryzen 7 9800X3D + RTX 3060 Ti:

### Windows Batch Script (`run_game.bat`)
```bat
@echo off
title VIctory Launcher
cd /d "%~dp0"

:: Pass the game executable as an argument, drag-and-drop it onto this script, or enter path below:
set "GAME_EXE=%~1"
if "%GAME_EXE%"=="" (
    set /p GAME_EXE="Enter full path to game_scarlett_master.exe: "
)

if not exist "%GAME_EXE%" (
    echo Error: Game executable not found at "%GAME_EXE%"
    pause
    exit /b 1
)

set VICTORY_LOG=info
set VICTORY_AMBIENT=1.2
set VICTORY_TEXTURE_DROP_MIPS=1
set VICTORY_CARD_WINDOWS_MB=1024
set VICTORY_LOOKUP_CACHE=1
set VICTORY_COPY_BACK_ONLY_READBACK=0
set VICTORY_INDIRECT_GATHER_ELIDE=0
set VICTORY_RELAY=2
set VICTORY_LATE_WAIT=3
set VICTORY_WIDE_BACK=1
set VICTORY_INDIRECT_WINDOW=1
set VICTORY_INDIRECT_GATHER=1
set VICTORY_INDIRECT_CARD_BUDGET=1
set VICTORY_NO_WATCHDOG=1
set VICTORY_DRIVER_WATCH=0
set VICTORY_SHADER_LOOP_LIMIT=1024

.\target\diag\victory.exe run "%GAME_EXE%" --execute
pause
```

---

## Building from Source

### Requirements
- Windows 10/11 x64 (22H2 or newer)
- AVX2 compatible CPU (Ryzen 7 9800X3D recommended)
- Direct3D 12 compatible GPU with Shader Model 6.6 and Mesh Shaders
- Rust toolchain pinned via `rust-toolchain.toml`

### Build Commands
```powershell
# Install git policy hooks
cargo xtask install-hooks

# Run test suite (requires no game assets)
cargo test --workspace

# Build optimized diagnostics binary (recommended for development)
cargo build --profile diag -p victory --features diagnostics
# Target path: target\diag\victory.exe

# Build release binary
cargo build --release -p victory
# Target path: target\release\victory.exe
```

---

## Keyboard Controls (Pad Emulation)

When running without an XInput controller, keyboard emulation is active while the window has focus:

| Keys | Gamepad Mapping |
|---|---|
| **W, A, S, D** / **I, J, K, L** | Left Stick / Right Stick |
| **Arrow Keys** | D-Pad |
| **Enter**, **Backspace**, **X**, **Y** | A, B, X, Y buttons |
| **Q** / **E**, **Z** / **C** | Left / Right Shoulders, Triggers |
| **F** / **H** | Left / Right Stick Clicks (L3 / R3) |
| **Esc**, **Tab** | Menu (Start), View (Back) |

---

## Diagnostic & Debugging Levers

| Variable | Description |
|---|---|
| `VICTORY_LOG=<target>=debug,...` | Target-specific trace and debug logging. |
| `VICTORY_STACKS=20,60` | Dumps all thread callstacks at specified timestamps. |
| `VICTORY_CAPTURE=<dir>` | Captures presented frame screenshots (powers of 2). |
| `VICTORY_DUMP_TARGETS=1` | Saves all render target attachments with frame captures. |
| `VICTORY_D3D12_DEBUG=1` | Enables Direct3D 12 API validation layers. |
| `VICTORY_D3D12_GBV=1` | Enables GPU-Based Validation (slow; detects descriptor leaks). |
| `VICTORY_D3D12_DRED=1` | Activates D3D12 Device Removed Extended Data for crash dumps. |
| `VICTORY_SHADER_LOOP_LIMIT=<n>` | Limits translated shader loop iterations to `n` (default 1024 for testing). |

---

## License

This project is licensed under the **GNU General Public License v3.0 or later** ([GPL-3.0-or-later](LICENSE)).
