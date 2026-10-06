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
