# Decisions

## 2026-10-06 — Static MSVC runtime
- Use Tauri 2 supported build.windows.staticVCRuntime: true setting for Windows builds.
- Do not set or export the deprecated STATIC_VCRUNTIME environment variable.
- Keep the static runtime enabled so the main Windows executable does not require the VC++ Redistributable for the MSVC runtime.
- CI must verify the built executable dependencies and fail if vcruntime140.dll or msvcp140.dll are listed.

## 2026-10-06 — Phase 1 acceptance gate
- Phase 1 remains limited to the shell, tray, settings, 3D character window, and its lifecycle/resource behavior.
- Windows CI must build the installed NSIS package and exercise the installed EXE, not only compile it.
- Runtime acceptance uses Windows UI Automation and Win32 input to verify the real tray menu, window lifecycle, dragging, transparent hit-testing, character replacement, persistence, and clean quit.
- The smoke also records process-tree memory and CPU evidence and verifies WebView2 descendants disappear after Hide.
- The CI gate is intentionally fail-fast for Phase 1 acceptance failures; a green compile without a green installed-EXE smoke is not a Phase 1 pass.

## 2026-10-06 — Core/Tray owns lifecycle
- Rebuilt the Phase 1 runtime around a permanent Rust Core/Tray owner.
- Character owns only Three.js, renderer, GLB, rig/pose/animation, and character-local input.
- Character Hide destroys the character WebView/renderer/model resources; it is never the application owner.
- Voice resources will be tied to Character presence in later phases; if Character and Chat are both closed, Brain and voice resources are destroyed and only Tray/Core remains.
- Existing Markdown specifications are retained; obsolete lifecycle-owner modules are removed instead of layering another manager over them.

## 2026-10-07 — Phase 1–4 milestone
- Phase 1–4 are treated as a product-development milestone, not four repair phases.
- Phase 1 may contain shell/runtime fixes because that phase is already under acceptance hardening.
- Phase 2 introduces the canonical rig/profile boundary. The Brain and future Character Engine must consume canonical intents and never reference Three.js or raw bone names.
- Phase 3 owns procedural character behavior and motion. Motion is expressed as canonical deltas and is rendered on demand.
- Phase 4 owns conversation persistence and Brain routing. Chat and future Voice input share one SQLite session model.
