import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";

type Settings = {
  character: {
    scale: "small" | "medium" | "large";
    currentId: string;
    alwaysOnTop: boolean;
    clickThrough: boolean;
  };
  performance: { lowPower: boolean };
};

const canvas = document.querySelector<HTMLCanvasElement>("#character-canvas")!;
const status = document.querySelector<HTMLDivElement>("#status")!;
const windowHandle = getCurrentWebviewWindow();

let renderer: THREE.WebGLRenderer | null = null;
let scene: THREE.Scene | null = null;
let camera: THREE.PerspectiveCamera | null = null;
let model: THREE.Object3D | null = null;
let renderFrame = 0;
let rotating = false;
let disposed = false;
let dragging = false;
let clickThrough = true;
let contextRecoveryAttempts = 0;
let recoveringContext = false;
let pendingAlphaProbe: { x: number; y: number; width: number; height: number } | null = null;
let activeRenderCallback: ((now: number) => boolean) | null = null;
const raycaster = new THREE.Raycaster();
const pointer = new THREE.Vector2();

function setStatus(message: string) {
  status.textContent = message;
  status.hidden = !message;
}

function disposeObject(root: THREE.Object3D) {
  root.traverse((object) => {
    const mesh = object as THREE.Mesh;
    if (mesh.geometry) mesh.geometry.dispose();
    const material = (mesh as THREE.Mesh).material;
    if (Array.isArray(material)) material.forEach(disposeMaterial);
    else if (material) disposeMaterial(material);
  });
}

function disposeMaterial(material: THREE.Material) {
  material.dispose();
  for (const key of ["map", "normalMap", "roughnessMap", "metalnessMap", "emissiveMap", "aoMap", "alphaMap", "bumpMap"] as const) {
    const value = (material as unknown as Record<string, unknown>)[key];
    if (value && typeof value === "object" && "dispose" in value) (value as THREE.Texture).dispose();
  }
}

function disposeRenderer() {
  activeRenderCallback = null;
  pendingAlphaProbe = null;
  if (model) {
    disposeObject(model);
    scene?.remove(model);
    model = null;
  }
  if (renderer) {
    renderer.dispose();
    renderer.forceContextLoss();
    renderer.domElement.width = 1;
    renderer.domElement.height = 1;
    renderer = null;
  }
  if (renderFrame) cancelAnimationFrame(renderFrame);
  renderFrame = 0;
}

function requestRender(active = false) {
  if (disposed || !renderer || !scene || !camera || renderFrame) return;
  const started = performance.now();
  const tick = (now: number) => {
    renderFrame = 0;
    if (disposed || !renderer || !scene || !camera) return;
    const keepActive = activeRenderCallback ? activeRenderCallback(now) : active && now - started < 2000;
    renderer.render(scene, camera);
    if (pendingAlphaProbe) samplePendingAlphaProbe();
    if (keepActive) renderFrame = requestAnimationFrame(tick);
    else activeRenderCallback = null;
  };
  renderFrame = requestAnimationFrame(tick);
}

function samplePendingAlphaProbe() {
  if (!pendingAlphaProbe || !renderer) return;
  const sample = pendingAlphaProbe;
  pendingAlphaProbe = null;
  const gl = renderer.getContext();
  if (gl.isContextLost()) return;
  const drawing = renderer.getDrawingBufferSize(new THREE.Vector2());
  if (!sample.width || !sample.height || drawing.x < 1 || drawing.y < 1) return;
  const px = Math.min(drawing.x - 1, Math.max(0, Math.floor((sample.x / sample.width) * drawing.x)));
  const py = Math.min(drawing.y - 1, Math.max(0, Math.floor(((sample.height - sample.y) / sample.height) * drawing.y)));
  const pixel = new Uint8Array(4);
  gl.readPixels(px, py, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixel);
  void windowHandle.setIgnoreCursorEvents(pixel[3] <= 8).catch(() => {});
}

function fitModel() {
  if (!model || !camera || !renderer) return;
  const box = new THREE.Box3().setFromObject(model);
  if (box.isEmpty()) return;
  const size = box.getSize(new THREE.Vector3());
  const center = box.getCenter(new THREE.Vector3());
  model.position.sub(center);
  const max = Math.max(size.x, size.y, size.z);
  const fov = THREE.MathUtils.degToRad(camera.fov);
  const distance = (max / 2) / Math.tan(fov / 2) * 1.25;
  camera.position.set(0, 0, Math.max(distance, 1));
  camera.near = Math.max(0.01, camera.position.z / 100);
  camera.far = camera.position.z * 100;
  camera.updateProjectionMatrix();
  requestRender();
}

async function load() {
  disposed = false;
  try {
    const settings = await invoke<Settings>("get_settings");
    clickThrough = settings.character.clickThrough;
    renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true, powerPreference: settings.performance.lowPower ? "low-power" : "high-performance" });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, settings.performance.lowPower ? 1 : 2));
  renderer.setSize(window.innerWidth, window.innerHeight, false);
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  scene = new THREE.Scene();
  camera = new THREE.PerspectiveCamera(28, Math.max(0.1, innerWidth / innerHeight), 0.01, 1000);
  camera.position.z = 3;

  scene.add(new THREE.HemisphereLight(0xffffff, 0x444444, 2));
  const key = new THREE.DirectionalLight(0xffffff, 2);
  key.position.set(2, 4, 3);
  scene.add(key);

  const bytes = await invoke<ArrayBuffer>("get_character_model");
  if (!bytes.byteLength) {
    await windowHandle.setIgnoreCursorEvents(clickThrough).catch(() => {});
    setStatus("No character model. Choose Change Character from the Saeed tray menu.");
    requestRender();
    return;
  }

  try {
    const gltf = await new GLTFLoader().parseAsync(bytes, "");
    model = gltf.scene;
    scene.add(model);
    await windowHandle.setIgnoreCursorEvents(clickThrough).catch(() => {});
    setStatus("");
    fitModel();
    contextRecoveryAttempts = 0;
  } catch (error) {
    setStatus("Unable to load this GLB.");
    await invoke("log_error", { message: `GLB load failed: ${String(error)}` }).catch(() => {});
    requestRender();
  }
  } catch (error) {
    setStatus("Unable to initialize the character.");
    await invoke("log_error", { message: `Character initialization failed: ${String(error)}` }).catch(() => {});
  }
}

async function recreate(_lowPower: boolean) {
  // Rust persists the requested setting before emitting renderer-recreate.
  // Reload settings from the source of truth inside load().
  disposeRenderer();
  await load();
}

canvas.addEventListener("webglcontextlost", (event) => {
  event.preventDefault();
  if (recoveringContext || disposed) return;
  recoveringContext = true;
  contextRecoveryAttempts += 1;
  if (contextRecoveryAttempts > 1) {
    recoveringContext = false;
    setStatus("WebGL context was lost and could not be restored.");
    void invoke("log_error", { message: "WebGL context lost after recovery attempt." }).catch(() => {});
    return;
  }
  setStatus("Recreating the WebGL renderer...");
  disposeRenderer();
  window.setTimeout(() => {
    void load().finally(() => {
      recoveringContext = false;
    });
  }, 50);
});

window.addEventListener("resize", () => {
  if (!renderer || !camera) return;
  renderer.setSize(innerWidth, innerHeight, false);
  camera.aspect = Math.max(0.1, innerWidth / innerHeight);
  camera.updateProjectionMatrix();
  fitModel();
});

window.addEventListener("pointerdown", async (event) => {
  if (!model || event.button !== 0) return;
  dragging = true;
  await invoke("set_character_interaction", { active: true }).catch(() => {});
  await windowHandle.setIgnoreCursorEvents(false).catch(() => {});
  await windowHandle.startDragging().catch(() => {});
});

window.addEventListener("pointerup", async () => {
  if (!dragging) return;
  dragging = false;
  await invoke("set_character_interaction", { active: false }).catch(() => {});
  if (clickThrough) await windowHandle.setIgnoreCursorEvents(true).catch(() => {});
});

window.addEventListener("blur", async () => {
  if (!dragging) return;
  dragging = false;
  await invoke("set_character_interaction", { active: false }).catch(() => {});
  if (clickThrough) await windowHandle.setIgnoreCursorEvents(true).catch(() => {});
});

windowHandle.listen("cursor-probe", async (event) => {
  const p = event.payload as { x: number; y: number; width: number; height: number };
  if (dragging || !clickThrough) {
    await windowHandle.setIgnoreCursorEvents(false).catch(() => {});
    return;
  }
  if (!model || !camera || !renderer) {
    await windowHandle.setIgnoreCursorEvents(true).catch(() => {});
    return;
  }
  pendingAlphaProbe = p;
  requestRender();
});

windowHandle.listen("click-through-changed", async (event) => {
  clickThrough = Boolean(event.payload);
  if (!clickThrough) {
    await windowHandle.setIgnoreCursorEvents(false).catch(() => {});
  }
});

windowHandle.listen("debug-rotate-once", () => {
  if (!model || rotating) return;
  rotating = true;
  const start = performance.now();
  const original = model.rotation.y;
  activeRenderCallback = (now) => {
    if (!model || disposed) {
      rotating = false;
      return false;
    }
    const t = Math.min(1, (now - start) / 2000);
    model.rotation.y = original + Math.PI * 2 * (t * t * (3 - 2 * t));
    if (t >= 1) {
      rotating = false;
      activeRenderCallback = null;
      return false;
    }
    return true;
  };
  requestRender(true);
});

windowHandle.listen("character-reload", async () => {
  disposeRenderer();
  await load();
});

windowHandle.listen("character-refit", () => fitModel());

windowHandle.listen("renderer-recreate", async (event) => {
  await recreate(Boolean(event.payload));
});

windowHandle.listen("prepare-destroy", async () => {
  disposeRenderer();
  disposed = true;
  await invoke("character_cleanup_done").catch(() => {});
});

window.addEventListener("beforeunload", () => {
  disposed = true;
  disposeRenderer();
});

void load();
