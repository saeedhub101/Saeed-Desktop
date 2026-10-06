# AGENTS.md: Rules for AI coding agents working on Saeed

Read `README.md` and everything in `docs/` before writing code.

## Hard rules
1. **Scope:** implement only the phase you were given. Do not add features from later phases.
2. **No prerequisites:** the final app must run on a clean Windows 10/11 machine with no installed runtimes. Do not add dependencies that require the user to install anything.
3. **Brain and Character separation:** the Brain/Agent code must never reference bones, rigs, or Three.js. It only emits intents through the Character Intent API.
4. **Destroy, don't hide:** windows and services that are not needed must be destroyed and release memory. See `docs/RESOURCE_LIFECYCLE.md`.
5. **Render on demand:** no permanent `requestAnimationFrame` loop. Render only while something changes.
6. **Quit only from the tray.** Closing any window must never exit the app.
7. **Providers are independent slots:** LLM, STT, TTS, Realtime. Never couple them.
8. **UI:** English only. Settings and tools open in a separate tabbed window, not side panels. Clean, professional look.
9. **Secrets:** API keys are stored with Windows Credential Manager (or DPAPI-encrypted file), never in plain JSON or logs.
10. **Safety:** every computer-control tool goes through the Permission Manager (`allow` / `deny` / `ask`).

## Working method
- Make small commits with clear messages.
- After each task, run the build and the acceptance checklist from the prompt, and report results honestly, including what is not working.
- If a requirement is ambiguous, choose the simplest option that satisfies the docs and note the decision in `docs/DECISIONS.md`.
- Never rewrite working files wholesale to make a small change.
- Write code that is typed and commented where logic is non-obvious.

## Definition of done (every task)
- Builds without warnings treated as errors.
- App starts on a clean profile and on an existing profile.
- Memory behaves as documented (check Task Manager when windows are closed/hidden).
- The acceptance checklist in the prompt is fully ticked, with a short report.
