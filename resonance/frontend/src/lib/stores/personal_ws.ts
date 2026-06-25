// ============================================================================
// resonance-frontend/src/lib/stores/personal_ws.ts
// A Svelte store that maintains a personal WebSocket connection to receive
// real-time notifications, DMs, jury summons, and presence pulses addressed
// specifically to this user.
//
// The backend publishes events to the Redis channel `user:{user_id}`, and
// the WS endpoint `/ws/personal?user_id=...` forwards them to the browser.
// ============================================================================

import { writable } from 'svelte/store';
import { session } from './session';

export interface PersonalEvent {
  type:
    | 'dm:new'
    | 'pulse:interaction'
    | 'presence:pulse'
    | 'resonance:threshold'
    | 'jury:concluded'
    | 'jury:summoned'
    | 'goal:lit';
  [key: string]: any;
}

export const personalEvents = writable<PersonalEvent | null>(null);
export const wsConnected = writable(false);

let ws: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

const WS_URL =
  (import.meta.env.VITE_PUBLIC_WS_URL as string | undefined)?.replace('/ws', '/ws/personal') ??
  'ws://localhost:8080/ws/personal';

export function connectPersonalWS(userId: string) {
  if (ws) ws.close();
  const url = `${WS_URL}?user_id=${encodeURIComponent(userId)}`;
  ws = new WebSocket(url);

  ws.onopen = () => {
    wsConnected.set(true);
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
  };

  ws.onclose = () => {
    wsConnected.set(false);
    // Reconnect after 3s if session still active.
    if (session.unlocked) {
      reconnectTimer = setTimeout(() => connectPersonalWS(userId), 3000);
    }
  };

  ws.onmessage = (e) => {
    try {
      const evt = JSON.parse(e.data) as PersonalEvent;
      personalEvents.set(evt);
    } catch {
      // ignore non-JSON frames
    }
  };

  ws.onerror = () => {
    ws?.close();
  };
}

export function disconnectPersonalWS() {
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  ws?.close();
  ws = null;
  wsConnected.set(false);
}
