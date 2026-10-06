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
        void scene?.load().catch((error) => {
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

  resizeScene(
    sizes[settings.character.scale],
  );

  installContextRecovery();

  await listen<{
    x: number;
    y: number;
    width: number;
    height: number;
  }>("cursor-probe", (event) => {
    if (!scene) {
      return;
    }

    const hit = scene.hitTest(
      event.payload.x,
      event.payload.y,
      event.payload.width,
      event.payload.height,
    );

    void windowHandle.setIgnoreCursorEvents(!hit);
  });

  canvas.addEventListener(
    "pointerdown",
    (event) => {
      if (
        scene?.hitTest(
          event.clientX,
          event.clientY,
          canvas.clientWidth,
          canvas.clientHeight,
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
      resizeScene(
        sizes[settings.character.scale],
      );
    },
  );

  await listen(
    "debug-rotate-once",
    () => scene?.rotateOnce(),
  );

  await listen(
    "character-reload",
    () =>
      scene?.load().catch((error) => {
        console.error(error);
        message(
          "Saeed could not load the character model.",
        );
      }),
  );

  try {
    await scene.load();
    scene.scheduler.requestRender();
  } catch (error) {
    console.error(error);
    message(
      "Saeed could not load the character model.",
    );
  }
}

window.addEventListener(
  "beforeunload",
  () => scene?.dispose(),
);

window.addEventListener("resize", () => {
  scene?.scheduler.requestRender();
});

void init().catch((error) => {
  console.error(error);
  message(
    "Saeed could not initialize the character window.",
  );
});
