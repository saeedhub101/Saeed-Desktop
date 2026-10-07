export type CharacterIntent =
  | { type:"idle"; intensity?:number }
  | { type:"gesture"; name:"nod"|"shake"|"wave"|"lean"|"arms_up"; intensity?:number }
  | { type:"lookAt"; x:number; y:number; z:number }
  | { type:"blink"; eye:"left"|"right"|"both" }
  | { type:"speech"; text:string };

export type IntentPriority = "background"|"normal"|"high";

export interface IntentEnvelope {
  intent:CharacterIntent;
  priority:IntentPriority;
  source:"brain"|"chat"|"voice"|"system";
  createdAt:number;
  ttlMs?:number;
}
