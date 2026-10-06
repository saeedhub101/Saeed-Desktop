# Saeed: Character Engine

The Character Engine is independent of the Brain. It receives **intents** and decides how to perform them.

## 1. Pipeline

```
Intent API ─► Behavior Layer ─► Motion Generator ─► Pose Mixer ─► Rig ─► Three.js skeleton
(from Brain)   (decides)        (builds motion)     (blends layers)  (canonical bones)
```

### Intent API
`happy()`, `sad()`, `cry()`, `greet()`, `celebrate()`, `dance()`, `think()`, `nod()`, `shake_head()`, `lean_in()`, `sway()`, `jump()`, `walk(to)`, `yawn()`, `sleep()`, `wake()`, `talk_start()`, `talk_end()`, `look_at(x,y)`.
Intents are requests, not commands. The engine may ignore, modify, or queue them.

### Behavior Layer
- Applies user settings (e.g. greetings off, celebrations off, victory off, all animation off, speech off).
- Applies priorities (speaking > reacting > idle) and cooldowns (no repeating the same gesture twice in a row).
- Runs the autonomous life cycle: idle variations, random gestures, walking on the desktop, yawning after inactivity, sleeping after longer inactivity, waking on interaction.
- Maps one intent to one of several variants (e.g. `celebrate` → clap, jump, or arms up).

### Motion Generator
Procedural motions defined as **keyframes of deltas on canonical bones** with easing, not clips.

```
motion "wave": duration 1.6s
  RightUpperArm: pitch  -150°  (ease in-out)
  RightForeArm : roll   ±25°   oscillate 3x
  Head         : yaw     10°
```

- Angles are expressed in canonical space (pitch / yaw / roll), then converted by the bone's calibration (see 3).
- Each motion declares per-bone limits; the **Motion Safety** layer clamps results to avoid broken poses.
- Motion library (initial): nod, shake, wave, think, jump, clap, dance, cry, lean_in, sway, look left/right, stretch, yawn, sit, stand, walk, sleep, wake.
- A motion can run on **layers** (body, head, face) so talking and gestures can overlap.

### Pose Mixer
Layers: `rest pose` (base) + `idle breathing` + `gesture` + `talk` + `look_at` + `face`. Weighted blend, applied once per frame.

## 2. Canonical rig

Canonical bones (all optional; missing bones are skipped, never an error):

`Hips, Spine, Chest, UpperChest, Neck, Head, Jaw, LeftEye, RightEye, Left/Right: Shoulder, UpperArm, ForeArm, Hand, Thigh, Shin, Foot, Toes, and finger bones`.

A model does not have to be a standard humanoid. Whatever bones the user maps are used.

## 3. Per-character profile (`profile.json`)

```
{
  "boneMap":   { "Head": "mixamorig:Head", ... },
  "restPose":  { "<canonicalBone>": [qx,qy,qz,qw], ... },
  "axisMap":   { "<canonicalBone>": { "pitch": "+x", "yaw": "+y", "roll": "-z" } },
  "limits":    { "<canonicalBone>": { "pitch": [-90,90], ... } },
  "motionTune":{ "wave": { "intensity": 0.8 }, ... },
  "scale": 1.0
}
```

Motions are always applied as `restPose × delta`, so fixing the rest pose fixes all motions.

## 4. Rig Lab (inside Control Center)

| Tool | Purpose |
|---|---|
| **Skeleton viewer** | List/tree of all bones in the loaded file, highlight selected bone in 3D |
| **Bone Mapper** | Auto-detect by name (Mixamo, VRM, Blender, custom). Manual assign: choose a canonical slot, pick a file bone. Works for files with unusual or missing standard names |
| **Rest Pose Editor** | Select bone, edit X/Y/Z rotation, apply, reset, save. T-pose to relaxed-pose helper (lower arms by an adjustable angle) |
| **Axis Wizard** | For each major bone: rotate test, user confirms the direction ("this is forward/up"); saves `axisMap` |
| **Motion Tester** | Buttons to play every motion; per-motion intensity and per-bone limit sliders; mirror left/right fix |
| **Profile** | Save/load/export profile per character; reset to auto-detected |

If the model has **no skeleton** at all: the app shows a clear message and offers a limited mode (whole-model transforms: bob, tilt, scale, jump) until the user supplies a rigged model.

## 5. Face

Optional, only if morph targets exist: expressions (neutral, happy, sad, angry, surprised, confused, sleepy, thinking), blink, visemes (aa, ee, oo, oh, fv, mbp), look-at with eyes/head. Missing morphs are skipped silently.

## 6. Performance rules

- Render on demand: render only while a motion, blink, speech, or interaction is active; otherwise stop the loop.
- Idle breathing may run at a reduced frame rate (e.g. 15 fps).
- Cap device pixel ratio; low-power mode available in settings.
- Dispose geometries, textures, materials, and the renderer when the window is destroyed.

## 7. Customization (later phases)

Outfit and accessory plugins attach GLB parts to canonical bones (e.g. glasses to `Head`, flag to `RightHand`) or swap material sets. The engine exposes `attach(partId, bone, offset)` and `detach(partId)`.
