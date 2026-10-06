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
