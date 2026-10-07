import type { CanonicalBone } from "./canonical";
import { Rig } from "./rig";

export type TestMotion = {name:string; duration:number; deltas:Array<[CanonicalBone,number,number,number]>};

export const TEST_MOTIONS:TestMotion[]=[
 {name:"nod",duration:700,deltas:[["Head",-12,0,0]]},
 {name:"shake",duration:700,deltas:[["Head",0,18,0]]},
 {name:"wave",duration:1000,deltas:[["RightUpperArm",0,0,-55],["RightForeArm",0,0,35]]},
 {name:"lean",duration:900,deltas:[["Spine",0,0,12],["Head",0,0,8]]},
 {name:"arms_up",duration:900,deltas:[["LeftUpperArm",0,0,-55],["RightUpperArm",0,0,55]]},
];

export function applyTestMotion(rig:Rig,motion:TestMotion,intensity=1) {
  for (const [bone,p,y,r] of motion.deltas) rig.applyDelta(bone,p*intensity,y*intensity,r*intensity);
}
