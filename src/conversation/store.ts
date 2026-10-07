import type { ConversationMessage, ConversationSession } from "./types";

export interface ConversationStore {
  createSession(title?:string):Promise<ConversationSession>;
  append(message:Omit<ConversationMessage,"id">):Promise<ConversationMessage>;
  list(sessionId:string):Promise<ConversationMessage[]>;
}
