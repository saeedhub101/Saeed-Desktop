import type { CharacterIntent, IntentEnvelope } from "../engine/intent";

export type LocalIntentResult = {handled:boolean; intent?:CharacterIntent};

export interface BrainProvider {
  complete(input:string, sessionId:string):Promise<string>;
}

export function localIntent(input:string):LocalIntentResult {
  const text=input.trim().toLowerCase();
  if (/^(hi|hello|مرحبا|اهلا|أهلا)/.test(text)) return {handled:true,intent:{type:"gesture",name:"wave",intensity:1}};
  if (/(nod|yes|نعم|اهز رأس)/.test(text)) return {handled:true,intent:{type:"gesture",name:"nod",intensity:1}};
  if (/(wave|bye|وداع|مع السلامة)/.test(text)) return {handled:true,intent:{type:"gesture",name:"wave",intensity:1}};
  return {handled:false};
}

export async function routeBrain(input:string,sessionId:string,provider:BrainProvider):Promise<{reply?:string;intent?:CharacterIntent}> {
  const local=localIntent(input);
  if (local.handled) return {intent:local.intent};
  return {reply:await provider.complete(input,sessionId)};
}

export function envelope(intent:CharacterIntent,source:"brain"|"chat"|"voice"|"system"="brain"):IntentEnvelope {
  return {intent,priority:"normal",source,createdAt:Date.now()};
}
