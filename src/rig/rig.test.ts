import * as THREE from "three";
import { describe, expect, it } from "vitest";
import { defaultProfile, clamp, Rig } from "./rig";

describe("rig foundation",()=>{
  it("clamps canonical deltas",()=>expect(clamp(120,[-90,90])).toBe(90));
  it("starts with a versioned profile",()=>expect(defaultProfile().schemaVersion).toBe(1));
  it("resolves mapped bones",()=>{
    const root=new THREE.Object3D();
    const head=new THREE.Bone(); head.name="HeadBone"; root.add(head);
    const p=defaultProfile(); p.boneMap.Head="HeadBone";
    const rig=new Rig(root,p);
    expect(rig.getBone("Head")).toBe(head);
  });
});
