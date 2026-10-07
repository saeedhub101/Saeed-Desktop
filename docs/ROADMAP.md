# Saeed: Roadmap

The roadmap covers the full build, not a repair-only cycle. Phases 1–4 form the first complete product milestone; fixes are only one part of Phase 1.

## Phase 1–4 product milestone

| Phase | Name | Outcome | Status |
|---|---|---|---|
| 1 | Shell and 3D window | Permanent Core/Tray, transparent GLB character window, drag/click-through, Change Character, destroy-on-hide, lifecycle and packaging acceptance | **In progress / acceptance hardening** |
| 2 | Rig Lab and canonical character rig | Control Center, skeleton viewer, Bone Mapper, Rest Pose Editor, Axis Wizard, profile persistence, calibration motions and tests | **Next implementation** |
| 3 | Character Engine | Intent API, Behavior Layer, procedural motion system, pose mixer, idle life cycle, face/look-at, speech bubble and render-on-demand integration | **Planned implementation** |
| 4 | Conversation and Brain | SQLite conversation sessions, Chat window, shared chat/voice-ready history contract, local intent router and one configurable LLM provider | **Planned implementation** |

Phases 5–10 extend the product after the first milestone:

| Phase | Name | Outcome |
|---|---|---|
| 5 | Voice | Mic with VAD, STT/TTS slots, mute, lip sync, unified history |
| 6 | Service Manager | Full create/destroy lifecycle, idle timers, Diagnostics tab |
| 7 | Tools and Permissions | System/files/input/screen tools, Permission Manager, Stop-all hotkey |
| 8 | Plugins | Manifest, process host, Office/PDF/OCR, Whisper plugins |
| 9 | Realtime | Realtime provider adapters, shared transcript |
| 10 | Customization | Outfits, accessories, event props, animation packs, character store |

## Phase gates

### Phase 1 — Shell and 3D window
Definition of done: installed Windows EXE smoke passes, not merely compilation.
- Tray: right-click opens the real menu; left-click toggles Show/Hide.
- Closing the character window never exits Core.
- Hide destroys the character WebView/renderer; Show creates it again.
- Character is placed at the bottom-right of the monitor work area on a fresh process launch.
- GLB loading, replacement, persistence, drag, alpha click-through, scale, always-on-top and low-power behavior pass.
- RenderScheduler has no permanent animation loop.
- Invalid GLB and WebGL recovery are handled without terminating Core.
- Cargo check/clippy, TypeScript, installer and Windows runtime smoke all pass.

### Phase 2 — Rig Lab
Definition of done: any GLB can be inspected and calibrated without code changes.
- Control Center is a separate destroyed-on-close window with top tabs.
- Canonical rig is independent of Brain and character UI.
- Skeleton viewer and automatic/manual mapping work with standard and unusual names.
- Rest pose and Relax Arms are live and persisted in profile.json.
- Axis calibration produces signed pitch/yaw/roll mappings.
- Calibration motions use only canonical deltas and safety limits.
- Profiles migrate, export/import and survive model replacement.
- Unit tests cover normalization, mapping, delta math, limits, migration and axis derivation.

### Phase 3 — Character Engine
Definition of done: Brain never manipulates Three.js, bones or rigs directly.
- Intent API is the only Brain-to-character contract.
- Behavior Layer applies settings, priority and cooldown rules.
- Procedural motions run through canonical bones, safety limits and Pose Mixer layers.
- Idle breathing/gestures/sleep/wake are lifecycle-aware and render only while active.
- Face/blink/look-at/visemes are optional and skip missing morphs safely.
- Speech bubble is independent of TTS and persists according to settings.
- Character resources are fully disposed when the character window is destroyed.

### Phase 4 — Conversation and Brain
Definition of done: chat and Brain use the same durable session contract that Voice will later consume.
- SQLite stores sessions and ordered messages with source = chat|voice.
- Chat opens in a separate window and is destroyed on close.
- Local intents are evaluated before an API request.
- One LLM provider adapter is implemented behind a provider interface.
- Provider failures are visible and never silently switch providers.
- API credentials never appear in JSON settings or logs.
- Brain emits only character intents; it never accesses rig/Three.js state.
- Chat replies, speech-bubble events and future voice transcripts share the same session history.

## Working rule
Each phase is implemented as product development. Phase 1 may contain fixes because its shell is already under construction; Phases 2–4 are new capabilities, not a sequence of repair-only phases.

No phase is declared complete until its acceptance checklist and CI evidence are green.