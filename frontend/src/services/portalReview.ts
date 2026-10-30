import { invoke } from '@tauri-apps/api/core';
export type PortalSource = 'generated' | 'selection';
export interface PortalPreview {
  session_id: string; fingerprint: string; app_id: string; action: string; model: string;
  endpoint: string; policy: string; image: string | null; text: string; max_tokens: number;
  redact: boolean; can_send: boolean; can_confirm_offline: boolean; message: string;
}
function nativeOnly() {
  if (!('__TAURI_INTERNALS__' in window)) throw new Error('NATIVE_REVIEW_REQUIRED');
}
export async function preparePortalReview(source: PortalSource, text: string, redact: boolean): Promise<PortalPreview> {
  nativeOnly();
  return invoke('portal_prepare_review', { args: { source, text, redact } });
}
export async function finishPortalReview(sessionId: string, fingerprint: string, send: boolean): Promise<string> {
  nativeOnly();
  return invoke('portal_finish_review', { sessionId, fingerprint, send });
}
export async function cancelPortalReview(sessionId: string): Promise<void> {
  try { nativeOnly(); await invoke('portal_cancel_review', { sessionId }); } catch { /* No retry or fallback. */ }
}
