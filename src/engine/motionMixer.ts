export type MotionLayer={id:string;weight:number;expiresAt:number;update:(now:number)=>void};

export class MotionMixer {
  private layers=new Map<string,MotionLayer>();
  add(layer:MotionLayer){this.layers.set(layer.id,layer);}
  remove(id:string){this.layers.delete(id);}
  tick(now=performance.now()){
    for(const [id,layer] of this.layers){
      if(now>=layer.expiresAt){this.layers.delete(id);continue;}
      layer.update(now);
    }
    return this.layers.size>0;
  }
  clear(){this.layers.clear();}
}
