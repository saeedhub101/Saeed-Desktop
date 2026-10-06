# Saeed Desktop

Lightweight Rust-based AI desktop companion for Windows.

## Foundation

- Rust application core
- Slint desktop UI
- SQLite for local persistence
- Modular boundaries for core, voice, character, tools and plugins
- Windows-native release target
- No Electron, Node.js runtime, Godot or Qt

## Architecture

The application is intentionally split into independent boundaries:

- **Core** — application state, session and orchestration
- **UI** — Slint interface
- **Voice** — microphone, STT and TTS
- **Character** — 3D runtime, VRM and procedural motion
- **Tools** — computer and external actions
- **Plugins** — extensibility
- **Storage** — SQLite

The Core is the single application orchestrator. There must not be multiple competing Brain/Core runtimes.

## Prototype

The first Windows prototype is packaged by GitHub Actions as a self-contained Saeed.exe inside the workflow artifact.

## Development

This repository is being built from scratch. Features are added incrementally and measured before unnecessary runtime dependencies are introduced.


## Architecture Rules — Development Contract

These rules are mandatory for every future feature, service, fix, and agent contribution.

1. **One owner per responsibility.** Every state, lifecycle, and capability has one clear owner.
2. **Core remains the single application orchestrator.** Do not create parallel Brain, Core, Agent, Runtime, Controller, or lifecycle owners.
3. **Chat and Voice share one Session.** They are input/output surfaces, never separate conversation engines.
4. **No parallel paths.** A new service must connect through the existing architecture instead of creating a side path around it.
5. **No duplicated state.** Do not introduce a second variable, cache, manager, or configuration value when an existing source of truth already exists.
6. **Modify the correct boundary.** Before changing code, identify which module owns the responsibility and make the smallest appropriate change there.
7. **Do not break existing contracts to add features.** Preserve established interfaces and behavior unless a deliberate architectural change is required.
8. **No fake UI.** A control is added only when it is connected to the real capability it represents.
9. **No permanent background loops unless explicitly required.** Work should be on-demand whenever possible.
10. **Keep dependencies minimal.** Add a dependency only when the feature genuinely requires it.
11. **Keep files predictable.** New code belongs in the module that owns its responsibility; do not scatter one feature across unrelated modules.
12. **Document extension points.** A future agent should be able to determine where to add a service, what it owns, and how it connects to Core without guessing.
13. **Inspect before modifying.** Check the existing implementation and contracts before adding a new abstraction, variable, manager, or service.
14. **Prefer the smallest safe change.** Fix the root cause without rewriting unrelated working code.
15. **Architecture changes must be intentional.** Do not silently introduce a second architecture while solving a local problem.
16. **Tests protect boundaries.** Tests should verify shared-session behavior, ownership boundaries, and important contracts as the system grows.
17. **Preserve this contract.** Every future agent contribution must follow these rules and must not add architectural overlap or destroy established structure.

### Where to change things

- **Core / lifecycle / shared conversation:** `src/core/` and `src/session.rs`
- **AI providers:** `src/ai/`
- **Voice, STT, TTS:** `src/voice/`
- **Character and procedural motion:** `src/character/`
- **Tools and plugins:** `src/tools/`
- **Persistence:** `src/storage.rs`
- **Desktop UI:** `ui/`
- **Application wiring:** `src/main.rs`
- **Build/package:** `.github/workflows/` and root build files

When adding a feature, first place its ownership in one of these boundaries. If no existing boundary can own it cleanly, document the architectural reason before creating a new one.
