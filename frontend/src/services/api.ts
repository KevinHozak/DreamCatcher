export const API_BASE = 'http://127.0.0.1:8080';

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

export async function fetchSystemStatus(): Promise<SystemStatus> {
  const res = await fetch(`${API_BASE}/api/system/status`);
  if (!res.ok) throw new Error('Failed to reach backend');
  return res.json();
}

export async function scanFolder(
  sourceDir: string,
  backend: 'ollama' | 'gemini' = 'ollama',
  ollamaModel: string = 'moondream'
): Promise<{
  source_dir: string;
  total_scanned: number;
  obvious_docs: MediaItem[];
  obvious_photos: MediaItem[];
  mixed_items: MediaItem[];
}> {
  const res = await fetch(`${API_BASE}/api/scan`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      source_dir: sourceDir,
      backend,
      ollama_model: ollamaModel,
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
  const res = await fetch(`${API_BASE}/api/scan/classify_single`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path, backend, ollama_model: ollamaModel }),
  });
  if (!res.ok) throw new Error('Classification failed');
  return res.json();
}

export async function clusterMedia(items: MediaItem[]): Promise<{ clusters: Cluster[] }> {
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
  const res = await fetch(`${API_BASE}/api/system/execute`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ source_dir: sourceDir, decisions, action }),
  });
  if (!res.ok) throw new Error('Execution failed');
  return res.json();
}

export function getThumbnailUrl(path: string, maxDim: number = 320): string {
  return `${API_BASE}/api/media/thumbnail?path=${encodeURIComponent(path)}&max_dim=${maxDim}`;
}

export function getFullFileUrl(path: string): string {
  return `${API_BASE}/api/media/full?path=${encodeURIComponent(path)}`;
}
