# Saeed: Providers

Four independent slots. Each slot has its own provider, model, key, and settings. They are never coupled.

| Slot | Purpose | Example providers |
|---|---|---|
| **LLM (Brain)** | Reasoning, tool calling | Anthropic Claude, OpenAI GPT, Google Gemini, Groq, OpenAI-compatible endpoints, local endpoints |
| **STT** | Speech to text | OpenAI, Groq, ElevenLabs, local Whisper (plugin) |
| **TTS** | Text to speech | ElevenLabs, OpenAI, local TTS (plugin) |
| **Realtime** | Full-duplex voice | OpenAI Realtime, Gemini Live, others via adapters |

Valid combination example: Brain = Groq, STT = Claude-compatible/other, TTS = ElevenLabs.
Note: availability of each capability depends on what the provider actually offers; the UI only lists providers that support the slot.

## Adapter interfaces (conceptual)

```
LlmProvider      { chat(messages, tools, options) -> stream of text/tool_calls }
SttProvider      { transcribe(audio, options) -> text }   // plus optional streaming
TtsProvider      { speak(text, voice, options) -> audio stream (+ visemes if available) }
RealtimeProvider { connect(options), sendAudio(), onAudio(), onTranscript(), onToolCall(), close() }
```

Adding a provider = adding one adapter file implementing the interface and one entry in the provider registry. No other code changes.

## Control Center → Providers tab

- One section per slot: provider dropdown, model, API key (stored in secrets), base URL for compatible endpoints, **Test** button.
- Status badge per slot: not configured, ok, error (with message).
- Realtime has an enable toggle; when active it replaces the STT → Brain → TTS chain for voice turns, while transcripts are still written to the same conversation.

## Fallback rules

- If the selected LLM fails, show the error in the bubble/chat; do not silently switch providers.
- Optional "fallback provider" per slot, configured by the user.
- Local-first: the Brain Router tries local intents before spending API calls.

## Cost/privacy notes

- Show which provider received each request in diagnostics.
- Audio is sent only when the mic is ON and a cloud STT/Realtime provider is selected.
