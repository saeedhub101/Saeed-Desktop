import type { CharacterIntent, IntentEnvelope } from "./intent";

export class BehaviorLayer {
  private cooldownUntil = new Map<string,number>();

  accept(envelope:IntentEnvelope):boolean {
    const key = envelope.intent.type === "gesture"
      ? `gesture:${envelope.intent.name}`
      : envelope.intent.type;
    const now=Date.now();
    if ((this.cooldownUntil.get(key) ?? 0) > now && envelope.priority !== "high") return false;
    this.cooldownUntil.set(key, now + (envelope.intent.type === "gesture" ? 250 : 80));
    return true;
  }

  normalize(intent:CharacterIntent):CharacterIntent {
    if (intent.type === "gesture") return {...intent,intensity:Math.max(0,Math.min(1,intent.intensity ?? 1))};
    if (intent.type === "lookAt") return {...intent,x:Math.max(-1,Math.min(1,intent.x)),y:Math.max(-1,Math.min(1,intent.y)),z:Math.max(-1,Math.min(1,intent.z))};
    return intent;
  }
}
