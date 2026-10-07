import { MotionMixer } from "./motionMixer";

export class IdleController {
  private active=false;
  private nextGesture=0;
  constructor(private readonly mixer:MotionMixer){}
  start(){this.active=true;this.nextGesture=performance.now()+5000;}
  stop(){this.active=false;this.mixer.clear();}
  update(now=performance.now()){
    if(!this.active)return false;
    if(now>=this.nextGesture){
      const start=now;
      this.mixer.add({id:"idle-breathe",weight:1,expiresAt:now+1800,update:()=>{}});
      this.nextGesture=start+5000+Math.random()*5000;
    }
    return this.mixer.tick(now);
  }
}
