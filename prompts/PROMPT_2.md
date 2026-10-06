# PROMPT 2: Phase 2: Rig Tools (Bone Mapper, Rest Pose, Axis Calibration)

Phase 1 is complete and verified. Read `README.md`, `AGENTS.md`, and all files in `docs/`, especially `CHARACTER_ENGINE.md`. Follow them strictly.

## Goal
Make **any** uploaded GLB usable by Saeed: even if it is in a T-pose, has non-standard bone names, wrong bone axes, or no standard skeleton. The user must be able to fix everything inside Saeed's own UI. This phase builds the **Rig Lab** and the per-character profile. It does **not** build the full animation engine, the Brain, or the speech bubble.

## Scope
Build:
1. The Control Center window (tabbed shell) with a **Rig Lab** tab.
2. The skeleton inspection, Bone Mapper, Rest Pose Editor, Axis Wizard, and a minimal Motion Tester.
3. The `profile.json` system and the **canonical rig layer** that the later Character Engine will rely on.

Do not build chat, voice, providers, plugins, the Behavior Layer, idle life cycle, face/visemes, or desktop walking.

## Technology
Same as Phase 1 (Tauri 2, Rust, TypeScript, Three.js).

## Requirements

### 1. Control Center window
- Separate window, **top tab bar** (no side panels), professional dark theme, English only.
- Created from the tray (**Control Center...**) and destroyed when closed.
- Tabs for now: **Rig Lab** (implemented), **Character** (list/select/delete characters, basic info). Other tabs may appear disabled as placeholders.
- Rig Lab uses its own 3D preview viewport inside the Control Center window with orbit/zoom/pan controls (OrbitControls) and a toggle to show the skeleton overlay. It edits the same character loaded in the character window; changes apply live to the character window via the Core event bus.

### 2. Canonical rig layer (`src/rig/`)
- Define the canonical bone list from `CHARACTER_ENGINE.md` (all optional).
- `Rig` class: holds `boneMap`, resolves canonical name → Three.js `Bone`, stores the **original bind quaternions**, and exposes `getBone(name)`, `has(name)`, `applyDelta(name, pitch, yaw, roll)`.
- `applyDelta` composes `restPose × axisCalibration × delta` as described in `CHARACTER_ENGINE.md`. Missing bones are skipped silently.
- Rig must work with models that have **zero** standard bones (everything unmapped) and with models that have **no skeleton** (report "No skeleton", and provide whole-model transform mode only).

### 3. Skeleton viewer
- Tree and flat searchable list of all bones in the file with names and hierarchy.
- Selecting a bone highlights it in the 3D preview (a visible marker) and in the tree.
- Show skeleton overlay lines and joint markers (toggle).

### 4. Bone Mapper
- **Auto-detect** by name patterns: Mixamo (`mixamorig:*`), VRM humanoid names, Blender/Rigify-style (`upper_arm.L`, `.R`), generic (`LeftUpperArm`, `Left_Arm`, `arm_l`, `Bip01`, etc.). Normalize names (case, separators, prefixes, left/right tokens) before matching. Show a confidence per match.
- **Manual mapping UI**: a list of canonical slots; each slot has a dropdown/picker to choose a bone from the file, a "pick from 3D" button (click a joint marker), and a clear button. Show unmapped slots clearly. Allow mirrored assign (copy L to R by name symmetry).
- Never block the user: partial mapping is valid. Required bones: none.
- Validation panel: warnings for suspicious mappings (e.g. same bone mapped twice, hierarchy order inconsistent), but allow saving anyway.

### 5. Rest Pose Editor
- Select a mapped bone; edit rotation with X/Y/Z sliders and numeric fields (degrees), live preview.
- Buttons: **Reset bone**, **Reset all** (to original bind pose), **Apply**, **Save to profile**.
- **T-pose helper:** "Relax arms" with an adjustable angle slider that rotates both upper arms (and optionally forearms) downward using the mapped arm bones; a visible "before/after" toggle. This is a helper only; the user can fine-tune every bone afterward.
- Symmetry toggle: editing a left bone mirrors to the right bone.
- The saved rest pose is stored as quaternions per canonical bone in `profile.json` and is re-applied every time the character loads.

### 6. Axis Wizard
- Purpose: tell Saeed what each bone's local axes mean, so motions rotate in the right direction on any model.
- For each major bone (Head, Neck, Spine, Chest, UpperArm L/R, ForeArm L/R, Thigh L/R): the wizard rotates the bone along one local axis as a preview and asks, via buttons, which real-world direction it moved (Up / Down / Forward / Back / Left / Right / Tilt left / Tilt right). From the answers it derives the `axisMap` (pitch/yaw/roll → local axis and sign).
- Provide an **Auto guess** option based on geometry (bone direction toward its child) and let the user override any value in a table.
- Save to `profile.json`. Include a **Test** button that plays a small, safe pitch/yaw/roll nudge to verify.

### 7. Minimal Motion Tester
- Implement only the **calibration-level test motions**: `nod`, `shake`, `wave`, `lean`, `arms_up`, `jump_whole_body`. They are simple delta definitions in `src/rig/testMotions.ts` using `applyDelta` only, played with an easing helper and the RenderScheduler.
- Per-bone **limit sliders** (min/max for pitch/yaw/roll) and a global **intensity** slider; store in `profile.json`. Apply limits (clamp) inside `applyDelta`.
- **Mirror fix** toggle per side (flip sign) to correct a mirrored arm.
- A **Reset to auto-detected profile** button.

### 8. Profile system
- `%APPDATA%/Saeed/characters/<id>/profile.json` as specified in `CHARACTER_ENGINE.md`, with `schemaVersion` and migration support.
- Autosave with debounce plus explicit Save; **Export profile** and **Import profile** (JSON file).
- If the user replaces the model file of the same character, keep the profile and re-validate the mapping (report missing bones).

### 9. Performance and lifecycle
- The Control Center window and its Three.js preview are destroyed on close and fully disposed (same disposal rules as Phase 1).
- Render on demand only; the preview renders while the user interacts or a test motion plays.
- Handling of large models: do not freeze the UI while parsing; show a loading state.

### 10. Quality
- Include unit tests for: name normalization and auto-mapping, `applyDelta` math (rest × axis × delta, clamp), profile migration, and axis derivation. Provide at least three test GLBs in a `test-assets/` folder or document how to obtain them (a Mixamo-style T-pose rig, a non-standard-named rig, and a model without a skeleton).

## Out of scope
Behavior Layer, autonomous motions, speech/face visemes, chat, voice, plugins, outfits.

## Acceptance checklist (report each as PASS/FAIL with notes)
- [ ] Control Center opens from the tray, uses top tabs, English only, and is destroyed on close (memory returns to baseline).
- [ ] Loading a Mixamo-style GLB auto-maps most bones with confidence values.
- [ ] Loading a GLB with unusual bone names allows complete manual mapping through the UI.
- [ ] A GLB with no skeleton shows "No skeleton" and offers whole-model mode, no crash.
- [ ] Rest Pose Editor changes the pose live in both the preview and the character window; Save persists; restart restores it.
- [ ] "Relax arms" converts a T-pose into a natural standing pose and can be fine-tuned.
- [ ] Axis Wizard produces an `axisMap`; after calibration, `nod` moves the head up/down, `shake` left/right, `wave` raises the correct arm on at least two different models.
- [ ] Limits and Mirror fix correct a wrong result without code changes.
- [ ] Export/Import profile round-trips exactly.
- [ ] Replacing the model keeps the profile and reports missing bones.
- [ ] Unit tests pass.
- [ ] No permanent render loop; Task Manager shows near-zero GPU/CPU when idle.

## Deliverables
- Source code, updated `docs/DECISIONS.md`, a short `docs/RIG_LAB_USER_GUIDE.md` (English) explaining each tool for the end user.
- A final report with checklist results, models tested, known issues, and measured memory figures.
