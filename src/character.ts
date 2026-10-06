import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { invoke } from "@tauri-apps/api/core";
import { RenderScheduler } from "./render_scheduler";

export type Scale = "small" | "medium" | "large";

export type Settings = {
  schemaVersion: number;
  character: {
    visible: boolean;
    scale: Scale;
    position: { x: number; y: number };
    alwaysOnTop: boolean;
    currentId: string;
  };
  performance: { lowPower: boolean };
};

export const sizes: Record<Scale, number> = {
  small: 280,
  medium: 360,
  large: 460,
};

const MASK_SIZE = 64;
const HIT_ALPHA = 12;

export class CharacterScene {
  scene = new THREE.Scene();
  camera = new THREE.PerspectiveCamera(32, 1, 0.01, 100);
  renderer: THREE.WebGLRenderer;
  scheduler: RenderScheduler;
  model: THREE.Object3D | null = null;

  private loader = new GLTFLoader();
  private alphaMask = new Uint8Array(MASK_SIZE * MASK_SIZE * 4);
  private maskTarget: THREE.WebGLRenderTarget;
  private lowPower: boolean;
  private disposed = false;

  /** Called after every mask refresh so hit-testing can be re-run. */
  onMaskUpdated: (() => void) | null = null;

  constructor(canvas: HTMLCanvasElement, lowPower: boolean) {
    this.lowPower = lowPower;
    this.renderer = this.createRenderer(canvas, lowPower);
    this.maskTarget = this.createMaskTarget();

    this.scene.add(
      new THREE.HemisphereLight(0xffffff, 0x444450, 2.2),
    );

    const key = new THREE.DirectionalLight(0xffffff, 2.5);
    key.position.set(2, 4, 5);
    this.scene.add(key);

    this.scheduler = new RenderScheduler(() => this.renderFrame());
  }

  private createRenderer(
    canvas: HTMLCanvasElement,
    lowPower: boolean,
  ): THREE.WebGLRenderer {
    const renderer = new THREE.WebGLRenderer({
      canvas,
      alpha: true,
      antialias: !lowPower,
      powerPreference: lowPower ? "low-power" : "high-performance",
      preserveDrawingBuffer: false,
    });

    renderer.setPixelRatio(
      Math.min(devicePixelRatio, lowPower ? 1 : 2),
    );
    renderer.setClearColor(0, 0);
    return renderer;
  }

  private createMaskTarget(): THREE.WebGLRenderTarget {
    return new THREE.WebGLRenderTarget(MASK_SIZE, MASK_SIZE, {
      format: THREE.RGBAFormat,
      type: THREE.UnsignedByteType,
      depthBuffer: true,
      stencilBuffer: false,
    });
  }

  private renderFrame(): void {
    if (this.disposed) {
      return;
    }

    this.renderer.setRenderTarget(this.maskTarget);
    this.renderer.render(this.scene, this.camera);
    this.renderer.readRenderTargetPixels(
      this.maskTarget,
      0,
      0,
      MASK_SIZE,
      MASK_SIZE,
      this.alphaMask,
    );

    this.onMaskUpdated?.();

    this.renderer.setRenderTarget(null);
    this.renderer.render(this.scene, this.camera);
  }

  hitTest(
    x: number,
    y: number,
    width: number,
    height: number,
  ): boolean {
    if (
      x < 0 ||
      y < 0 ||
      x >= width ||
      y >= height ||
      width <= 0 ||
      height <= 0
    ) {
      return false;
    }

    const maskX = Math.min(
      MASK_SIZE - 1,
      Math.floor((x / width) * MASK_SIZE),
    );
    const maskY = Math.min(
      MASK_SIZE - 1,
      Math.floor(((height - y) / height) * MASK_SIZE),
    );

    return (
      this.alphaMask[
        (maskY * MASK_SIZE + maskX) * 4 + 3
      ] >= HIT_ALPHA
    );
  }

  recreateRenderer(): void {
    const canvas = this.renderer.domElement;
    this.renderer.dispose();
    this.maskTarget.dispose();

    this.renderer = this.createRenderer(
      canvas,
      this.lowPower,
    );
    this.maskTarget = this.createMaskTarget();
    this.scheduler.requestRender();
  }

  setLowPower(lowPower: boolean): void {
    this.lowPower = lowPower;
    this.recreateRenderer();
  }

  /** Idempotent: safe to call from the cleanup handshake and beforeunload. */
  dispose(): void {
    if (this.disposed) {
      return;
    }

    this.disposed = true;
    this.scheduler.dispose();
    this.onMaskUpdated = null;

    if (this.model) {
      this.disposeObject(this.model);
      this.scene.remove(this.model);
      this.model = null;
    }

    this.maskTarget.dispose();
    this.renderer.dispose();
    this.renderer.forceContextLoss();
  }

  private disposeObject(root: THREE.Object3D): void {
    root.traverse((node) => {
      const mesh = node as THREE.Mesh;
      if (!mesh.isMesh) {
        return;
      }

      mesh.geometry.dispose();

      const skinned = node as THREE.SkinnedMesh;
      if (skinned.isSkinnedMesh) {
        skinned.skeleton.dispose();
      }

      for (const material of Array.isArray(mesh.material)
        ? mesh.material
        : [mesh.material]) {
        for (const value of Object.values(material)) {
          if (value instanceof THREE.Texture) {
            value.dispose();
          }
        }
        material.dispose();
      }
    });
  }

  private fit(root: THREE.Object3D): void {
    const box = new THREE.Box3().setFromObject(root);
    const size = box.getSize(new THREE.Vector3());

    root.position.sub(box.getCenter(new THREE.Vector3()));
    root.scale.setScalar(
      2.8 / Math.max(size.x, size.y, size.z, 0.001),
    );

    const bounds = new THREE.Box3().setFromObject(root);
    root.position.sub(bounds.getCenter(new THREE.Vector3()));

    const scaledSize = bounds.getSize(new THREE.Vector3());

    this.camera.position.set(
      0,
      scaledSize.y * 0.45,
      Math.max(scaledSize.y * 1.45, 3.4),
    );
    this.camera.lookAt(0, scaledSize.y * 0.05, 0);
    this.camera.updateProjectionMatrix();
  }

  placeholder(): void {
    const group = new THREE.Group();

    group.add(
      new THREE.Mesh(
        new THREE.CapsuleGeometry(0.72, 1.45, 8, 16),
        new THREE.MeshStandardMaterial({
          color: 0x4b4b55,
          roughness: 0.8,
        }),
      ),
    );

    const head = new THREE.Mesh(
      new THREE.SphereGeometry(0.58, 24, 16),
      new THREE.MeshStandardMaterial({
        color: 0x777783,
        roughness: 0.75,
      }),
    );

    head.position.y = 1.2;
    group.add(head);
    group.userData.placeholder = true;

    this.scene.add(group);
    this.model = group;
    this.fit(group);
  }

  async load(): Promise<void> {
    if (this.model) {
      this.disposeObject(this.model);
      this.scene.remove(this.model);
      this.model = null;
    }

    const bytes = await invoke<ArrayBuffer>("get_character_model");

    if (this.disposed) {
      return;
    }

    if (!bytes || bytes.byteLength === 0) {
      this.placeholder();
      this.scheduler.requestRender();
      return;
    }

    await new Promise<void>((resolve, reject) => {
      this.loader.parse(
        bytes,
        "",
        (gltf) => {
          if (this.disposed) {
            this.disposeObject(gltf.scene);
            resolve();
            return;
          }

          this.model = gltf.scene;
          this.scene.add(this.model);
          this.fit(this.model);
          this.scheduler.requestRender();
          resolve();
        },
        (error) => {
          reject(
            new Error("GLB load failed: " + String(error)),
          );
        },
      );
    });
  }

  rotateOnce(): void {
    if (!this.model || this.model.userData.placeholder) {
      return;
    }

    const start = performance.now();
    const base = this.model.rotation.y;

    this.scheduler.setActive(true);

    const tick = (now: number) => {
      if (!this.model) {
        return;
      }

      const progress = Math.min(
        (now - start) / 2000,
        1,
      );

      this.model.rotation.y =
        base + Math.PI * 2 * progress;
      this.scheduler.requestRender();

      if (progress < 1) {
        requestAnimationFrame(tick);
      } else {
        this.scheduler.setActive(false);
      }
    };

    requestAnimationFrame(tick);
  }
}