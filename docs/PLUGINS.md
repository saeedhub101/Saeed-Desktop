# Saeed: Plugins

Plugins add capabilities without bloating the core installer.

## Types

| Type | Examples |
|---|---|
| `tools` | Word, Excel, PDF, OCR, browser, email, calendar |
| `stt` | Local Whisper and others |
| `tts` | Local TTS engines |
| `provider` | Extra LLM/Realtime adapters |
| `animation` | Extra motion packs |
| `outfit` | Clothing, accessories, glasses, hair, event props (flags, flowers) |
| `character` | Packaged character with profile |

## Package layout

```
plugin-id/
  manifest.json
  bin/ or lib/        executable or assets
  README.md
```

### manifest.json

```
{
  "id": "saeed.pdf",
  "name": "PDF Tools",
  "version": "1.0.0",
  "type": "tools",
  "entry": { "exe": "bin/pdf-tool.exe" },
  "permissions": ["files.read", "files.write"],
  "tools": [ { "name": "pdf_extract_text", "schema": { ... } } ],
  "memory_hint_mb": 80,
  "idle_timeout_s": 60
}
```

## Runtime model

- A plugin is a **separate process** speaking JSON-RPC over stdio. A crashing plugin never crashes Saeed.
- Started on first use, stopped after `idle_timeout_s` of inactivity.
- Declared permissions are shown to the user at install and enforced by the Permission Manager.
- Tools exposed by a plugin register into the Tool Registry like built-in tools.

## Installation

- Control Center → Plugins tab: browse a catalog (JSON index), install from URL or local file, enable/disable, remove.
- Downloads are verified (SHA-256 listed in the catalog). Unsigned or unknown plugins show a warning.
- Plugins are installed into `%APPDATA%/Saeed/plugins/`. No system-wide installs, no admin rights.

## Outfit / animation plugins

- `outfit` plugins ship GLB parts plus a small JSON describing attachment bone, offset, scale, and slots (hat, glasses, hair, top, bottom, held item).
- `animation` plugins ship motion definitions in the same delta format as built-in motions (see `CHARACTER_ENGINE.md`).
- Event props can be bound to events (holiday, victory) and are subject to the same user settings toggles.
