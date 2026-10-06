# Saeed

**A lightweight Windows desktop AI agent with a 3D character as its face.**

Talk to Saeed by microphone or by chat, in the same conversation. Ask it to operate your computer, build reports, enter data, analyze information, or use applications. The character reacts, gestures, walks around your desktop, and speaks.

---

## Core ideas

1. **The 3D character is the interface.** Everything else (chat, settings) is secondary.
2. **The Brain never moves the character.** It only sends intents such as `emote("happy")`. A separate Character Engine decides what actually happens.
3. **One conversation, many inputs.** Microphone and chat share a single history. Start by voice, continue by text, and vice versa.
4. **Every provider is replaceable and independent.** Brain (LLM), STT, TTS and Realtime are four separate slots. Mix Claude, GPT, Gemini, Groq, ElevenLabs, local Whisper, and others freely.
5. **Lightweight by design.** Anything not in use is destroyed, not hidden.
6. **Plugins add capabilities.** Office, PDF, OCR, local STT/TTS, animation packs, outfits.
7. **No prerequisites.** One installer. The user installs nothing else.

## Feature overview

| Area | Capability |
|---|---|
| Interaction | Open microphone (VAD, not push-to-talk), chat, shared history, realtime voice mode |
| Agent | Local intent matching first, API brain as fallback, tool calling, permission system |
| Computer control | Mouse, keyboard, windows, apps, files, screenshots, documents |
| Character | Any GLB/VRM model, bone mapping, rest-pose fixing, procedural animation, face, lip sync |
| Behavior | Greetings, dance, celebration, crying, leaning in, swaying, jumping, walking on the desktop, yawning, sleeping, waking |
| Speech bubble | Shows replies above the character, works when muted, persists until disabled in settings |
| Customization | Replace the model, outfits, accessories, glasses, hair, event props (flags, flowers) via plugins |
| Settings | Tabbed window: disable greetings, animations, celebrations, victory motions, speech, and more |
| Shell | Tray icon, hide/show character, quit only from the tray |

## Technology

| Layer | Choice |
|---|---|
| App shell and core | Tauri 2 + Rust |
| 3D | Three.js in a WebView2 window (preinstalled on Windows 10/11, bundled bootstrapper as fallback) |
| Storage | SQLite (conversations, memory) + JSON (settings, character profiles) |
| Windows control | `windows` crate: SendInput, UI Automation, screen capture |
| Plugins | Manifest + separate process over JSON-RPC (stdio) |

## Repository layout

```
Saeed/
  README.md
  AGENTS.md                  Rules for AI coding agents
  docs/
    ARCHITECTURE.md
    CHARACTER_ENGINE.md
    PROVIDERS.md
    PLUGINS.md
    RESOURCE_LIFECYCLE.md
    ROADMAP.md
  prompts/
    PROMPT_1.md              Phase 1: shell + 3D window
    PROMPT_2.md              Phase 2: rig tools
  src-tauri/                 Rust core
  src/                       Frontend (character, chat, settings windows)
```

## Status

Rebuilt from scratch. See [docs/ROADMAP.md](docs/ROADMAP.md).

## Principles for contributors

- Build one phase at a time. Do not start a phase until the previous one passes its acceptance checklist.
- No feature may keep a service running when nothing needs it.
- UI language: English only.
