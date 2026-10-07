export type MessageSource="chat"|"voice";

export interface ConversationMessage {
  id:number;
  sessionId:string;
  role:"user"|"assistant"|"system";
  content:string;
  source:MessageSource;
  createdAt:number;
}

export interface ConversationSession {
  id:string;
  title:string;
  createdAt:number;
  updatedAt:number;
}
