// ============================================================================
// resonance-frontend/src/lib/api/notifications.ts
// ============================================================================

import { signedFetch } from './client';

export interface NotificationOut {
  id: string;
  kind: string;
  actor_id: string | null;
  actor_username: string | null;
  target_type: string | null;
  target_id: string | null;
  payload: any;
  read_at: string | null;
  created_at: string;
}

export async function listNotifications(
  userId: string,
  opts: { unreadOnly?: boolean; limit?: number } = {},
): Promise<NotificationOut[]> {
  const params = new URLSearchParams();
  if (opts.unreadOnly) params.set('unread_only', 'true');
  if (opts.limit) params.set('limit', opts.limit.toString());
  const qs = params.toString();
  return signedFetch<NotificationOut[]>(
    userId,
    'GET',
    qs ? `/notifications?${qs}` : '/notifications',
  );
}

export async function unreadCount(userId: string): Promise<number> {
  const res = await signedFetch<{ unread: number }>(userId, 'GET', '/notifications/unread-count');
  return res.unread;
}

export async function markRead(userId: string, notifId: string): Promise<void> {
  await signedFetch(userId, 'POST', `/notifications/${notifId}/read`, {});
}

export async function markAllRead(userId: string): Promise<void> {
  await signedFetch(userId, 'POST', '/notifications/read-all', {});
}
