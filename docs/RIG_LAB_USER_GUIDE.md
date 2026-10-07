# Rig Lab User Guide

## Purpose

Rig Lab prepares an uploaded GLB for the later Character Engine. A partial mapping is valid; there are no mandatory canonical bones.

## Workflow

1. Open **Control Center** from the Saeed tray.
2. Select **Rig Lab**.
3. Load/select the character.
4. Inspect the skeleton and select bones to identify their hierarchy.
5. Run automatic mapping, then correct any uncertain mappings manually.
6. Use **Rest Pose** to correct the model's starting pose. **Relax Arms** is a helper for T-pose models.
7. Run **Axis Wizard** for major bones when motions move in the wrong direction.
8. Use the calibration Motion Tester to verify nod, shake, wave, lean and arms-up.
9. Save the profile. It is stored with the character and is restored when the model is loaded.

## Safety

- Missing bones are skipped.
- Suspicious duplicate or hierarchy mappings are warnings, not blockers.
- Motion limits are clamped before a delta is applied.
- A model with no skeleton remains usable in whole-model mode.
