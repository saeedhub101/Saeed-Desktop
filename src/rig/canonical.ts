export const CANONICAL_BONES = [
  "Hips","Spine","Chest","UpperChest","Neck","Head","Jaw","LeftEye","RightEye",
  "LeftShoulder","RightShoulder","LeftUpperArm","RightUpperArm","LeftForeArm","RightForeArm",
  "LeftHand","RightHand","LeftThigh","RightThigh","LeftShin","RightShin","LeftFoot","RightFoot",
  "LeftToes","RightToes"
] as const;

export type CanonicalBone = typeof CANONICAL_BONES[number];

export const MAJOR_BONES: readonly CanonicalBone[] = [
  "Head","Neck","Spine","Chest","LeftUpperArm","RightUpperArm",
  "LeftForeArm","RightForeArm","LeftThigh","RightThigh"
];

export function sideOf(name: CanonicalBone): "left"|"right"|null {
  if (name.startsWith("Left")) return "left";
  if (name.startsWith("Right")) return "right";
  return null;
}
