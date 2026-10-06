# Saeed: Resource Lifecycle

**Rule: if it is not used, destroy it. Do not hide it.**

## State table

| User state | Running | Destroyed |
|---|---|---|
| Everything closed, character hidden, mic OFF | Core + tray only | Character window, Renderer, Chat, Brain, Voice, TTS, STT, plugins |
| Character visible, chat closed, mic OFF | Core, Character window | Brain, Voice, STT, TTS |
| Chat open | Core, Character, Chat, Brain (while active) | STT (if mic OFF) |
| Mic ON | Core, Character, Voice capture + VAD, STT, Brain on demand | none of the above |
| Muted | everything as above | TTS pipeline |
| Character hidden, Chat open | Core, Chat, Brain while active | Character window, Three.js, renderer, Mic, VAD, STT, TTS |\n| Character hidden, Chat closed | Core + tray only | Character, Three.js, renderer, Mic, VAD, STT, TTS, Brain, Chat |

## Mechanics

- **Core/Tray** owns application lifecycle. No window is the application owner. It creates/destroys feature resources and is the only permanent runtime.\n- **Character** owns only character resources and character-local animation/input. Destroying Character destroys its renderer, GLB, and voice resources tied to its presence.\n- A future Service Manager may implement resource helpers, but it must not become a second lifecycle owner.
- **Brain** is created on first message, destroyed after an idle timeout (default 2 minutes).
- **STT / VAD** exist only while the mic is ON.
- **TTS** is created on demand for a spoken reply, destroyed after idle (default 30 s) and while muted.
- **Plugins** stop after their `idle_timeout_s`.
- **Character window**: Hide = close the window → WebView2 process exits → GPU and JS memory released. Show = create again, reload model and profile.
- **Renderer**: on destroy, dispose scene, geometries, materials, textures, then renderer, and force context loss.
- **Render loop**: runs only while animations or interaction are active.

## State persistence across destroy

Everything needed to rebuild lives on disk or in the Core: conversation, settings, character profile, last window position. Destroying a service must never lose user data.

## Budgets (targets, to verify)

| Situation | Target |
|---|---|
| Tray only | Core process under ~30 MB |
| Character idle visible | Total under ~200 MB including WebView2 |
| Hidden | Back to the tray-only figure |

Add a Diagnostics tab showing running services, their memory, and last activity.
