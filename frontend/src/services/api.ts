import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

export const API_BASE = 'http://127.0.0.1:8080';

export const isTauri =
  typeof window !== 'undefined' &&
  ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);

export interface MediaItem {
  id: string;
  name: string;
  path: string;
  ext: string;
  is_video: boolean;
  size: number;
  timestamp: string;
  date_str: string;
  month_str: string;
  is_undated: boolean;
  has_sidecar: boolean;
  sidecar_path: string | null;
  has_gps: boolean;
  gps: [number, number] | null;
  category: 'DOCUMENT' | 'PHOTO';
  tier: 'OBVIOUS' | 'MIXED';
  reason: string;
  caption: string;
  is_cached?: boolean;
}

export interface Cluster {
  id: string;
  date_str: string;
  folder_name: string;
  is_holiday: boolean;
  is_event: boolean;
  items: MediaItem[];
}

export interface SystemStatus {
  status: string;
  ollama: {
    connected: boolean;
    url: string;
    models: string[];
    has_moondream: boolean;
  };
}

export interface AppSettings {
  source_dir: string;
  pictures_dir: string | null;
  videos_dir: string | null;
}

export interface InventoryScanSummary {
  scan_id: string;
  root_kind: 'pictures' | 'videos';
  root_path: string;
  status: string;
  discovered: number;
  indexed: number;
  skipped: number;
  completed_at: string;
}

export interface InventoryStats {
  pictures_count: number;
  pictures_bytes: number;
  videos_count: number;
  videos_bytes: number;
  stale_count: number;
  last_scan: InventoryScanSummary | null;
}

export async function scanInventory(root: string, rootKind: 'pictures' | 'videos'): Promise<InventoryScanSummary> {
  if (isTauri) return await invoke<InventoryScanSummary>('scan_inventory', { root, rootKind });
  const res = await fetch(`${API_BASE}/api/inventory/scan`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ root, root_kind: rootKind }),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.detail || 'Inventory scan failed');
  }
  return res.json();
}

export async function fetchInventoryStats(): Promise<InventoryStats> {
  if (isTauri) return await invoke<InventoryStats>('get_inventory_stats');
  const res = await fetch(`${API_BASE}/api/inventory/stats`);
  if (!res.ok) throw new Error('Failed to load inventory statistics');
  return res.json();
}

export async function fetchSettings(): Promise<AppSettings> {
  if (isTauri) return await invoke<AppSettings>('get_settings');

  const res = await fetch(`${API_BASE}/api/settings`);
  if (!res.ok) throw new Error('Failed to load settings');
  return res.json();
}

export async function saveSettings(settings: AppSettings): Promise<AppSettings> {
  if (isTauri) return await invoke<AppSettings>('save_settings', { settings });

  const res = await fetch(`${API_BASE}/api/settings`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(settings),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.detail || 'Failed to save settings');
  }
  return res.json();
}

export async function pickFolder(title: string): Promise<string | null> {
  if (!isTauri) return window.prompt(`${title}: enter an absolute folder path`);
  const selected = await open({ directory: true, multiple: false, title });
  return typeof selected === 'string' ? selected : null;
}

export async function fetchSystemStatus(): Promise<SystemStatus> {
  if (isTauri) {
    return await invoke<SystemStatus>('get_system_status', {
      ollamaUrl: 'http://127.0.0.1:11434',
    });
  }

  const res = await fetch(`${API_BASE}/api/system/status`);
  if (!res.ok) throw new Error('Failed to reach backend');
  return res.json();
}

export interface ScanResult {
  source_dir: string;
  month_filter: string | null;
  total_discovered: number;
  total_scanned: number;
  obvious_docs: MediaItem[];
  obvious_photos: MediaItem[];
  mixed_items: MediaItem[];
}

export async function scanFolder(
  sourceDir: string,
  backend: 'ollama' | 'gemini' = 'ollama',
  ollamaModel: string = 'moondream',
  monthFilter?: string | null,
  runAiOnAmbiguous: boolean = false
): Promise<ScanResult> {
  if (isTauri) {
    return await invoke<ScanResult>('scan_folder', {
      sourceDir,
      monthFilter: monthFilter || null,
      runAiOnAmbiguous,
      backend,
      ollamaModel,
      ollamaUrl: 'http://127.0.0.1:11434',
    });
  }

  const res = await fetch(`${API_BASE}/api/scan`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      source_dir: sourceDir,
      backend,
      ollama_model: ollamaModel,
      month_filter: monthFilter || null,
      run_ai_on_ambiguous: runAiOnAmbiguous,
    }),
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.detail || 'Scan failed');
  }
  return res.json();
}

export async function classifySingle(
  path: string,
  backend: 'ollama' | 'gemini' = 'ollama',
  ollamaModel: string = 'moondream'
): Promise<{ category: string; reason: string; caption: string }> {
  if (isTauri) {
    return await invoke<{ category: string; reason: string; caption: string }>(
      'classify_single',
      {
        path,
        backend,
        ollamaModel,
        ollamaUrl: 'http://127.0.0.1:11434',
      }
    );
  }

  const res = await fetch(`${API_BASE}/api/scan/classify_single`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path, backend, ollama_model: ollamaModel }),
  });
  if (!res.ok) throw new Error('Classification failed');
  return res.json();
}

export async function classifyBatch(
  paths: string[],
  backend: 'ollama' | 'gemini' = 'ollama',
  ollamaModel: string = 'moondream'
): Promise<{
  results: Array<{
    path: string;
    category: string;
    tier: string;
    reason: string;
    caption: string;
  }>;
}> {
  if (isTauri) {
    const results = await invoke<
      Array<{
        path: string;
        category: string;
        tier: string;
        reason: string;
        caption: string;
      }>
    >('classify_batch', {
      paths,
      backend,
      ollamaModel,
      ollamaUrl: 'http://127.0.0.1:11434',
    });
    return { results };
  }

  const res = await fetch(`${API_BASE}/api/scan/classify_batch`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ paths, backend, ollama_model: ollamaModel }),
  });
  if (!res.ok) throw new Error('Batch classification failed');
  return res.json();
}

export async function streamClassify(
  paths: string[],
  onItem: (item: {
    path: string;
    category: string;
    tier: string;
    reason: string;
    caption: string;
  }) => void,
  backend: 'ollama' | 'gemini' = 'ollama',
  ollamaModel: string = 'moondream',
  signal?: AbortSignal
): Promise<void> {
  if (isTauri) {
    for (const path of paths) {
      if (signal?.aborted) break;
      try {
        const res = await invoke<{
          category: string;
          tier: string;
          reason: string;
          caption: string;
        }>('classify_single', {
          path,
          backend,
          ollamaModel,
          ollamaUrl: 'http://127.0.0.1:11434',
        });
        onItem({ path, ...res });
      } catch (e) {
        console.error('Error during desktop classification:', e);
      }
    }
    return;
  }

  const res = await fetch(`${API_BASE}/api/scan/classify_stream`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ paths, backend, ollama_model: ollamaModel }),
    signal,
  });
  if (!res.ok || !res.body) throw new Error('Streaming classification failed');

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';
    for (const line of lines) {
      if (line.trim()) {
        try {
          const data = JSON.parse(line);
          onItem(data);
        } catch (e) {
          console.error('Error parsing stream line:', e);
        }
      }
    }
  }
  if (buffer.trim()) {
    try {
      const data = JSON.parse(buffer);
      onItem(data);
    } catch (e) {
      console.error('Error parsing final stream chunk:', e);
    }
  }
}

export async function clusterMedia(
  items: MediaItem[]
): Promise<{ clusters: Cluster[] }> {
  if (isTauri) {
    const clusters = await invoke<Cluster[]>('cluster_media', {
      items,
      clusterHours: 4.0,
      minClusterSize: 5,
    });
    return { clusters };
  }

  const res = await fetch(`${API_BASE}/api/scan/cluster`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ items, cluster_hours: 4.0, min_cluster_size: 5 }),
  });
  if (!res.ok) throw new Error('Clustering failed');
  return res.json();
}

export async function executeTriage(
  sourceDir: string,
  decisions: Record<string, any>,
  action: 'move' | 'copy' = 'move'
): Promise<any> {
  if (isTauri) {
    return await invoke('execute_triage', {
      sourceDir,
      decisions,
      action,
    });
  }

  const res = await fetch(`${API_BASE}/api/system/execute`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ source_dir: sourceDir, decisions, action }),
  });
  if (!res.ok) throw new Error('Execution failed');
  return res.json();
}

export interface RollbackResult {
  success: boolean;
  restored_items: number;
  restored_sidecars: number;
  errors: number;
  ledger_backup?: string;
}

export async function rollbackTriage(
  sourceDir: string
): Promise<RollbackResult> {
  if (isTauri) {
    return await invoke<RollbackResult>('rollback_triage', {
      sourceDir,
    });
  }

  const res = await fetch(`${API_BASE}/api/system/rollback`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ source_dir: sourceDir }),
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.detail || 'Rollback failed');
  }
  return res.json();
}

export function getThumbnailUrl(path: string, maxDim: number = 320): string {
  if (isTauri) {
    return `http://dc-media.localhost/thumbnail?path=${encodeURIComponent(path)}&max_dim=${maxDim}`;
  }
  return `${API_BASE}/api/media/thumbnail?path=${encodeURIComponent(path)}&max_dim=${maxDim}`;
}

export function getFullFileUrl(path: string): string {
  if (isTauri) {
    return `http://dc-media.localhost/full?path=${encodeURIComponent(path)}`;
  }
  return `${API_BASE}/api/media/full?path=${encodeURIComponent(path)}`;
}
