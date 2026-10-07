import type { IntentEnvelope } from "./intent";
import { BehaviorLayer } from "./behavior";

export interface CharacterDriver {
  execute(intent:IntentEnvelope):void;
}

export class CharacterEngine {
  private readonly behavior=new BehaviorLayer();
  constructor(private readonly driver:CharacterDriver){}

  dispatch(envelope:IntentEnvelope):boolean {
    const normalized={...envelope,intent:this.behavior.normalize(envelope.intent)};
    if (!this.behavior.accept(normalized)) return false;
    this.driver.execute(normalized);
    return true;
  }
}
