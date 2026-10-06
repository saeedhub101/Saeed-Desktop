import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import "./styles.css";
import {
  CharacterScene,
  Settings,
  sizes,
} from "./character";

const canvas =
  document.querySelector<HTMLCanvasElement>(
    "#character-canvas",
  )!;
const status =
  document.querySelector<HTMLDivElement>("#status")!;
const windowHandle = getCurrentWindow();

let scene: CharacterScene | null = null;
let statusTimer: number | undefined;
let contextRecoveryAttempts = 0;
let currentScale: keyof typeof sizes = "medium";
let shuttingDown = false;

type Probe = { x: number; y: number; width: number; height: number };

// Matches the window's initial set_ignore_cursor_events(false).
let ignoringCursor = false;
let lastProbe: Probe | null = null;

function applyHitTest(): void {
  if (!scene || !lastProbe) {
    return;
  }

  const { x, y, width, height } = lastProbe;
  const shouldIgnore = !scene.hitTest(x, y, width, height);

  // Only talk to the OS when the state actually changes.
  if (shouldIgnore === ignoringCursor) {
    return;
  }

  ignoringCursor = shouldIgnore;
  windowHandle.setIgnoreCursorEvents(shouldIgnore).catch(() => {
    ignoringCursor = !shouldIgnore;
  });
}

/** Releases all 3D resources; Rust waits for this before destroying the window. */
function shutdownScene(): void {
  shuttingDown = true;

  if (statusTimer !== undefined) {
    clearTimeout(statusTimer);
  }

  scene?.dispose();
  scene = null;
}

function message(text: string): void {
  status.textContent = text;
  status.classList.add("visible");

  if (statusTimer !== undefined) {
    clearTimeout(statusTimer);
  }

  statusTimer = window.setTimeout(() => {
    status.classList.remove("visible");
  }, 5000);
}

function resizeScene(size: number): void {
  if (!scene) {
    return;
  }

  scene.renderer.setSize(size, size, false);
  scene.scheduler.requestRender();
}

function installContextRecovery(): void {
  canvas.addEventListener(
    "webglcontextlost",
    (event) => {
      event.preventDefault();

      // Context loss caused by our own cleanup must not trigger recovery.
      if (shuttingDown) {
        return;
      }

      if (contextRecoveryAttempts >= 1) {
        message(
          "WebGL context could not be recovered.",
        );
        return;
      }

      contextRecoveryAttempts += 1;
      message(
        "WebGL context lost. Recreating the renderer...",
      );

      try {
        scene?.recreateRenderer();
        void scene?.load().then(() => {
          contextRecoveryAttempts = 0;
        }).catch((error) => {
          console.error(error);
          message(
            "Saeed could not load the character model.",
          );
        });
      } catch (error) {
        console.error(error);
        message(
          "Saeed could not recover the WebGL renderer.",
        );
      }
    },
    { passive: false },
  );
}

async function init(): Promise<void> {
  const settings =
    await invoke<Settings>("get_settings");

  scene = new CharacterScene(
    canvas,
    settings.performance.lowPower,
  );

  currentScale = settings.character.scale;
  resizeScene(sizes[currentScale]);

  installContextRecovery();

  scene.onMaskUpdated = applyHitTest;

  await listen<Probe>("cursor-probe", (event) => {
    lastProbe = event.payload;
    applyHitTest();
  });

  await listen("prepare-destroy", async () => {
    try {
      shutdownScene();
    } catch (error) {
      console.error(error);
    } finally {
      await invoke("character_cleanup_done");
    }
  });

  canvas.addEventListener(
    "pointerdown",
    (event) => {
      const rect = canvas.getBoundingClientRect();
      const x = event.clientX - rect.left;
      const y = event.clientY - rect.top;

      if (
        scene?.hitTest(
          x,
          y,
          rect.width,
          rect.height,
        )
      ) {
        void windowHandle.startDragging();
      }
    },
  );

  await listen<boolean>(
    "renderer-recreate",
    (event) => {
      if (!scene) {
        return;
      }

      scene.setLowPower(event.payload);
      resizeScene(sizes[currentScale]);
    },
  );

  await listen(
    "debug-rotate-once",
    () => scene?.rotateOnce(),
  );

  await listen(
    "character-reload",
    () => {
      const size = window.innerWidth;
      currentScale = size <= sizes.small ? "small" : size <= sizes.medium ? "medium" : "large";
      resizeScene(size);
      return scene?.load().catch((error) => {
        console.error(error);
        void invoke("log_error", {
          message: String(error),
        });
        message(
          "Saeed could not load the character model.",
        );
      });
    },
  );

  try {
    await scene.load();
    scene.scheduler.requestRender();
  } catch (error) {
    console.error(error);
    void invoke("log_error", {
      message: String(error),
    });
    message(
      "Saeed could not load the character model.",
    );
  }
}

window.addEventListener("beforeunload", () => shutdownScene());

window.addEventListener("resize", () => {
  const size = window.innerWidth;
  currentScale = size <= sizes.small ? "small" : size <= sizes.medium ? "medium" : "large";
  resizeScene(size);
});

void init().catch((error) => {
  console.error(error);
  void invoke("log_error", {
    message: String(error),
  });
  message(
    "Saeed could not initialize the character window.",
  );
});