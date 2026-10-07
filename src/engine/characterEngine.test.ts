import { describe, expect, it } from "vitest";
import { CharacterEngine } from "./characterEngine";
import type { IntentEnvelope } from "./intent";

describe("CharacterEngine",()=>{
  it("normalizes and dispatches canonical intents",()=>{
    const seen:IntentEnvelope[]=[];
    const engine=new CharacterEngine({execute:i=>seen.push(i)});
    expect(engine.dispatch({intent:{type:"gesture",name:"wave",intensity:4},priority:"normal",source:"brain",createdAt:Date.now()})).toBe(true);
    expect(seen[0].intent.type==="gesture" && seen[0].intent.intensity).toBe(1);
  });
  it("blocks repeated non-high gestures during cooldown",()=>{
    const seen:IntentEnvelope[]=[];
    const engine=new CharacterEngine({execute:i=>seen.push(i)});
    const e={intent:{type:"gesture",name:"nod" as const},priority:"normal" as const,source:"brain" as const,createdAt:Date.now()};
    expect(engine.dispatch(e)).toBe(true);
    expect(engine.dispatch(e)).toBe(false);
    expect(seen).toHaveLength(1);
  });
});
