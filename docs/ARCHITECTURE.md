# Saeed: Architecture

## 1. Overview

```
                        ┌──────────────────────────────┐
                        │   Core (Rust, always on)     │
                        │  Tray · Event Bus · Settings │
                        │  Service Manager · Permissions│
                        │  Conversation Store (SQLite) │
                        └──────────────┬───────────────┘
        ┌──────────────┬───────────────┼───────────────┬───────────────┐
        ▼              ▼               ▼               ▼               ▼
  Character Window  Chat Window   Control Center   Voice Service    Brain Service
  (Three.js, 3D)    (tabs UI)     (settings, tabs) (mic, VAD,       (router, agent,
  on demand         on demand     on demand         STT, TTS)        tools)
                                                    on demand        on demand
```

Only the Core/Tray is permanent. It is the program's lifecycle owner. It creates and destroys Character, Chat, Brain, Voice, and Control Center as needed. Character owns only the character runtime: Three.js, renderer, GLB, rig, pose, animation, and character-local input. Nothing outside Character may own character motion.

## 2. Core (Rust)

| Module | Responsibility |
|---|---|
| `tray` | Tray icon and menu. The only way to quit. |
| `bus` | Typed internal event bus. All services talk through events, never direct calls across layers. |
| `services` | Service Manager: `start(name)`, `stop(name)`, idle timers, dependency rules. |
| `settings` | JSON settings with schema versioning and migration. |
| `secrets` | API keys via Windows Credential Manager / DPAPI. |
| `permissions` | Permission Manager: category and tool policy `allow` / `deny` / `ask`. |
| `store` | SQLite: conversations, messages, memory, learned skills. |
| `windows_mgr` | Not a lifecycle owner. Window creation/destruction is orchestrated by Core; window-specific code stays local to each feature. |
| `plugins` | Discover, install, start, stop, and call plugins. |

## 3. Event bus (contract between layers)

Examples of events:

| Event | Producer | Consumer |
|---|---|---|
| `input.user_message {session, source: voice\|chat, text}` | Voice / Chat | Brain |
| `brain.reply {session, text, emotion?}` | Brain | Chat, Bubble, TTS, Character |
| `character.intent {name, args}` | Brain, Voice, Core | Character Engine |
| `voice.state {listening, speaking}` | Voice | Character, UI |
| `tool.request` / `tool.result` | Brain / Tools | Permission Manager, UI |
| `service.started` / `service.stopped` | Service Manager | UI diagnostics |

## 4. Unified conversation

- A **session** has one ordered message list in SQLite.
- Microphone transcripts and chat text are both stored as `user` messages with `source = voice | chat`.
- Switching input method never creates a new session. A new session is created only by the user ("New chat").
- The Brain always receives history from the store, so both inputs see the same context.

## 5. Brain

```
User message
   │
   ▼
Brain Router
   ├─ 1. Local intent matcher  (open app, time, system info, tasks, memory)  → execute locally
   ├─ 2. Learned skill matcher (user-taught workflows)                       → run workflow
   └─ 3. API Brain (LLM + tools)  when not understood or needs reasoning
            │
            ▼
        Tool Registry → Permission Manager → Executor
```

- The router decides locally first to save cost and latency.
- The API Brain uses tool calling. Tool results are verified before the final answer.
- The Brain's only way to affect the character is `character.intent`. Example intents: `happy`, `sad`, `think`, `greet`, `celebrate`, `dance`, `nod`, `shake_head`, `lean_in`, `sway`, `jump`, `cry`, `talk_start`, `talk_end`.

## 6. Tools

Built-in tool groups: **system**, **apps/windows**, **files**, **documents**, **web**, **screen**, **input** (mouse/keyboard), **memory/tasks**.
Heavy document tools (Word, Excel, PDF, OCR) come from plugins.

Each tool declares: name, JSON schema, permission category, `destructive: bool`, timeout.

## 7. Permission Manager

Categories: Files, Applications, System, Network, Screen, Input, Microphone, Memory/Tasks, Credentials, Destructive.
Policy per category and per tool: `allow`, `deny`, `ask`. `ask` shows a confirmation dialog with the exact action. Destructive tools always ask unless explicitly set to `allow` by the user.

## 8. Voice service

```
Mic (Rust, cpal) → VAD → STT provider → text → Brain
Brain reply → TTS provider → audio out
Realtime mode: Mic ⇄ Realtime provider (audio in/out) → transcript events to the same session
```

- Voice is tied to Character presence. When Character is destroyed, microphone capture, VAD, STT, and TTS resources are stopped/destroyed. Brain may remain alive only when Chat is still open.
- Open-mic VAD; no push-to-talk.
- Mute destroys the TTS pipeline only; replies still appear in the speech bubble.
- Lip sync: TTS provider emits visemes or the engine derives a simple amplitude-based mouth movement; sent to the Character Engine as `character.speech_frame`.

## 9. Character window

- Transparent, frameless, always-on-top window.
- Hosts Three.js scene and the Character Engine (see `CHARACTER_ENGINE.md`).
- Click-through outside the character silhouette (toggle ignore-cursor-events from a pixel alpha test).
- Speech bubble is an HTML overlay above the head, anchored to the head bone's screen position.
- Character owns all character-local movement and animation. Brain emits intents only; it never manipulates bones, rigs, Three.js, or renderer state.

## 10. Windows

| Window | Created when | Destroyed when |
|---|---|---|
| Character | Show Saeed | Hide Saeed |
| Chat | User opens chat (tray or character click) | User closes it |
| Control Center | User opens settings or tools | User closes it |
| Rig Lab (inside Control Center, tab) | User opens it | Closed with Control Center |

Control Center uses a top tab bar, not side panels. English only.

## 11. Data locations

```
%APPDATA%/Saeed/
  settings.json
  saeed.db                  conversations, memory, skills
  characters/<id>/model.glb
  characters/<id>/profile.json   bone map, rest pose, axis calibration, motion tuning
  plugins/<plugin-id>/
  logs/
```

## 12. Security

- API keys never leave the secrets store; never logged.
- Plugins run as separate processes with a declared permission list in the manifest.
- Input control tools rate-limited; an emergency "Stop all actions" hotkey cancels running tool chains.
