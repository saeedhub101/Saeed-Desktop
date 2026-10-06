# PROMPT 1: Phase 1: Shell and 3D Character Window

You are building **Saeed**, a lightweight Windows desktop AI agent whose face is a 3D character. This is a rebuild from scratch. Before writing any code, read `README.md`, `AGENTS.md`, and every file in `docs/`. Follow them strictly.

## Scope of this phase
Only the application shell and the 3D character window. **Do not** implement chat, brain, voice, providers, tools, plugins, or the motion engine. Do not add features beyond this list.

## Technology (fixed)
- Tauri 2 + Rust core
- Frontend: TypeScript + Vite, Three.js for 3D (no heavy UI framework; plain TS or a very small library)
- Target: Windows 10/11 x64
- The installer must be a single self-contained package (NSIS or MSI) that embeds the WebView2 bootstrapper. The user must not need to install anything else.

## Requirements

### 1. Project setup
- Create the repository layout from `README.md` (`src-tauri/`, `src/`).
- Add a `docs/DECISIONS.md` file and log any decision you make.
- Provide `npm run dev` and `npm run build` that work from a clean clone, and document the build prerequisites for the developer (not for end users) in `docs/DEV_SETUP.md`.

### 2. Core (Rust)
- Single-instance app: launching a second time focuses the existing one (shows the character if hidden).
- Settings module: `%APPDATA%/Saeed/settings.json` with `schemaVersion`, default values, atomic writes. Keys for this phase: `character.visible`, `character.scale`, `character.position {x,y}`, `character.alwaysOnTop`, `character.currentId`, `performance.lowPower`.
- Logging to `%APPDATA%/Saeed/logs/` with rotation.
- Window manager module with functions `create_character_window()` and `destroy_character_window()`.

### 3. Tray
- Tray icon with menu: **Show Saeed**, **Hide Saeed**, **Change Character...**, **Character Size** (Small / Medium / Large), **Always on Top** (toggle), **Low Power Mode** (toggle), **Quit**.
- The menu reflects current state (Show disabled when visible, etc.).
- **Quit is only possible from the tray.** Closing any window must never exit the app.
- Left-click on the tray icon toggles the character.

### 4. Character window
- Transparent, frameless, no taskbar button, optional always-on-top, no shadow.
- Hosts a Three.js scene: perspective camera, soft lighting, renderer with alpha and antialiasing, `devicePixelRatio` capped (2, or 1 in low power mode).
- Loads a `.glb` (GLTFLoader). Use a small bundled default model for first run; if none is bundled, show a clear placeholder and the "Change Character" flow.
- **Change Character...**: native file dialog to pick a `.glb`; copy it to `%APPDATA%/Saeed/characters/<id>/model.glb` and load it. Keep previous characters on disk.
- Auto-fit: compute the bounding box, scale and center the model so the full body fits the window at the chosen size.
- Drag the character by clicking and dragging on the character itself. Save position on drop. Keep the window on-screen (clamp to the monitor work area, multi-monitor aware).
- **Click-through outside the character:** transparent pixels must not capture mouse input. Implement it by sampling the alpha under the cursor (read pixels from the canvas, throttled) and toggling Tauri's ignore-cursor-events accordingly. Clicking the character itself must work.
- Basic scale options (Small / Medium / Large) change the window size and re-fit.

### 5. Hide = destroy
- **Hide Saeed** must **destroy** the character window completely, not hide it.
- Before the window closes, the frontend must dispose all geometries, materials, textures, and the renderer, and force context loss.
- **Show Saeed** creates a new window and reloads the current character and settings.
- Verify in Task Manager that the WebView2 child processes disappear on Hide.

### 6. Render on demand
- No permanent `requestAnimationFrame` loop. Provide a small `RenderScheduler` that renders only when requested (`requestRender()`), and keeps rendering while an "active" flag is set (for example during drag or load). When idle, nothing renders.
- Add a temporary debug "Turn" test (tray submenu **Debug > Rotate once**) that rotates the model 360° over 2 seconds using the scheduler, to prove that rendering starts and stops correctly. Remove nothing else; leave it in under a debug flag.

### 7. Error handling
- If the GLB fails to load, show a small English message inside the window and log the error. The app must never crash or exit.
- If WebGL context is lost, recreate the renderer once, then show an error.

### 8. UI language and style
- English only. No other windows are required in this phase.

## Out of scope (do not build)
Chat, settings window, bone tools, animations, speech bubble, microphone, providers, plugins.

## Acceptance checklist (report each item as PASS/FAIL with notes)
- [ ] `npm run build` produces an installer that installs and runs on a clean Windows 10/11 machine with nothing else installed.
- [ ] Tray icon appears; Show/Hide/Size/Always on Top/Low Power/Quit all work; menu state is correct.
- [ ] Closing windows never exits the app; Quit exits cleanly with no leftover processes.
- [ ] Second launch focuses the existing instance.
- [ ] A GLB loads, is auto-fitted, and is draggable; position persists after restart.
- [ ] Transparent areas are click-through; the character itself is clickable and draggable.
- [ ] Change Character loads a different GLB and persists after restart.
- [ ] Hide destroys the window; WebView2 processes are gone; Show restores the same character and position.
- [ ] While idle, the GPU/CPU usage is near zero (no continuous rendering). Rotate-once test works.
- [ ] Memory: tray-only state is low; report the measured numbers for Core only, character visible idle, and after Hide.
- [ ] Corrupt/invalid GLB shows an error and the app keeps running.

## Deliverables
- Working source code, `docs/DECISIONS.md`, `docs/DEV_SETUP.md`.
- A final report: what was built, the checklist results, measured memory figures, known issues. Be honest about anything that does not work.
