import type { Value, Identity } from "./value.js";
import type { Message } from "./world.js";
import { World } from "./world.js";

/** A single dispatched message and its results. */
export interface Event {
  target: Identity;
  message: Message;
  sender: Identity | undefined;
  replies: Value[];
}

/** A sequence of events, serializable for persistence alongside world snapshots. */
export interface EventLog {
  events: Event[];
}

/** Create a new empty event log. */
export function createEventLog(): EventLog {
  return { events: [] };
}

/** Returns a new log containing events 0..index (the prefix up to the branch point). */
export function branchAt(log: EventLog, index: number): EventLog {
  return { events: log.events.slice(0, index) };
}

/** Clone the world, replay the log on it, return the new world and all replies. */
export function replayFrom(world: World, log: EventLog): [World, Value[]] {
  const w = world.clone();
  const replies = w.replay(log);
  return [w, replies];
}

/** Given the original world (pre-log) and a log, replay up to index,
 *  return the new world state and the truncated log. */
export function forkAt(world: World, log: EventLog, index: number): [World, EventLog] {
  const truncated = branchAt(log, index);
  const [w] = replayFrom(world, truncated);
  return [w, truncated];
}
