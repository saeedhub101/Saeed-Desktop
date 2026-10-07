import * as THREE from "three";
import { CANONICAL_BONES, type CanonicalBone } from "./canonical";

export type Axis = "+x"|"-x"|"+y"|"-y"|"+z"|"-z";
export type AxisMap = Partial<Record<CanonicalBone,{pitch:Axis;yaw:Axis;roll:Axis}>>;
export type Limits = Partial<Record<CanonicalBone,{pitch:[number,number];yaw:[number,number];roll:[number,number]}>>;

export interface RigProfile {
  schemaVersion:number;
  boneMap: Partial<Record<CanonicalBone,string>>;
  restPose: Partial<Record<CanonicalBone,[number,number,number,number]>>;
  axisMap: AxisMap;
  limits: Limits;
  motionTune: Record<string,{intensity:number}>;
  scale:number;
}

const identity = new THREE.Quaternion();

export function defaultProfile(): RigProfile {
  const boneMap: Partial<Record<CanonicalBone,string>> = {};
  const restPose: Partial<Record<CanonicalBone,[number,number,number,number]>> = {};
  const limits: Limits = {};
  for (const bone of CANONICAL_BONES) {
    restPose[bone] = [0,0,0,1];
    limits[bone] = {pitch:[-90,90],yaw:[-90,90],roll:[-90,90]};
  }
  return {schemaVersion:1,boneMap,restPose,axisMap:{},limits,motionTune:{},scale:1};
}

export function clamp(value:number, range:[number,number]):number {
  return Math.min(range[1], Math.max(range[0], value));
}

function axisQuaternion(axis:Axis, angle:number):THREE.Quaternion {
  const v = new THREE.Vector3();
  const sign = axis.startsWith("-") ? -1 : 1;
  const a = axis[1];
  if (a === "x") v.x = sign; else if (a === "y") v.y = sign; else v.z = sign;
  return new THREE.Quaternion().setFromAxisAngle(v, THREE.MathUtils.degToRad(angle));
}

export class Rig {
  readonly root:THREE.Object3D;
  readonly bones = new Map<CanonicalBone,THREE.Bone>();
  private readonly originals = new Map<CanonicalBone,THREE.Quaternion>();
  profile:RigProfile;

  constructor(root:THREE.Object3D, profile:RigProfile=defaultProfile()) {
    this.root=root;
    this.profile=profile;
    this.resolve();
  }

  resolve() {
    this.bones.clear();
    for (const [canonical, fileName] of Object.entries(this.profile.boneMap) as [CanonicalBone,string][]) {
      const found=this.findBone(fileName);
      if (found) {
        this.bones.set(canonical,found);
        this.originals.set(canonical,found.quaternion.clone());
        if (this.profile.restPose[canonical]) {
          found.quaternion.fromArray(this.profile.restPose[canonical]!);
        }
      }
    }
  }

  private findBone(name:string):THREE.Bone|null {
    let result:THREE.Bone|null=null;
    this.root.traverse(o=>{ if (!result && o instanceof THREE.Bone && o.name===name) result=o; });
    return result;
  }

  getBone(name:CanonicalBone){ return this.bones.get(name); }
  has(name:CanonicalBone){ return this.bones.has(name); }

  reset(name?:CanonicalBone) {
    if (name) {
      const b=this.bones.get(name); const q=this.originals.get(name);
      if (b && q) b.quaternion.copy(q);
      return;
    }
    for (const bone of CANONICAL_BONES) this.reset(bone);
  }

  applyDelta(name:CanonicalBone,pitch:number,yaw:number,roll:number) {
    const bone=this.bones.get(name); if (!bone) return;
    const limit=this.profile.limits[name] ?? {pitch:[-90,90],yaw:[-90,90],roll:[-90,90]};
    pitch=clamp(pitch,limit.pitch); yaw=clamp(yaw,limit.yaw); roll=clamp(roll,limit.roll);
    const axes=this.profile.axisMap[name];
    const qp=axisQuaternion(axes?.pitch ?? "+x",pitch);
    const qy=axisQuaternion(axes?.yaw ?? "+y",yaw);
    const qr=axisQuaternion(axes?.roll ?? "+z",roll);
    const delta=identity.clone().multiply(qp).multiply(qy).multiply(qr);
    const rest=this.profile.restPose[name];
    const base=rest ? new THREE.Quaternion().fromArray(rest) : (this.originals.get(name)?.clone() ?? identity.clone());
    bone.quaternion.copy(base).multiply(delta);
  }

  serializeRestPose() {
    const out:RigProfile["restPose"]={};
    for (const [name,bone] of this.bones) out[name]=bone.quaternion.toArray() as [number,number,number,number];
    return out;
  }
}
