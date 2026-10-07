export interface LookAtDriver { setLookAt(x:number,y:number,z:number):void; }

export class LookAtController {
  constructor(private readonly driver:LookAtDriver){}
  set(x:number,y:number,z:number){
    const length=Math.hypot(x,y,z)||1;
    this.driver.setLookAt(x/length,y/length,z/length);
  }
}
