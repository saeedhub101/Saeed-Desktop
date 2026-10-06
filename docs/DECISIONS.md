# Decisions

## 2026-10-06 — Static MSVC runtime
- Use Tauri 2 supported build.windows.staticVCRuntime: true setting for Windows builds.
- Do not set or export the deprecated STATIC_VCRUNTIME environment variable.
- Keep the static runtime enabled so the main Windows executable does not require the VC++ Redistributable for the MSVC runtime.
- CI must verify the built executable dependencies and fail if vcruntime140.dll or msvcp140.dll are listed.
