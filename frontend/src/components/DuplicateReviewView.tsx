import { useEffect, useState } from 'react';
import { Check, RefreshCw, ShieldCheck, X } from 'lucide-react';
import { analyzeDuplicates, excludeDuplicateMember, fetchDuplicateGroups, type DuplicateGroup } from '../services/api';

interface Props { onClose: () => void; }

function bytes(value: number) { return value < 1024 ? `${value} B` : `${(value / 1024 / 1024).toFixed(1)} MB`; }

export function DuplicateReviewView({ onClose }: Props) {
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = () => {
    fetchDuplicateGroups().then((result) => setGroups(result.groups)).catch((err) => setError(err instanceof Error ? err.message : 'Could not load duplicates')).finally(() => setLoading(false));
  };

  useEffect(() => { load(); }, []);

  const analyze = async () => {
    setAnalyzing(true);
    setError(null);
    try { await analyzeDuplicates(); load(); } catch (err) { setError(err instanceof Error ? err.message : 'Duplicate analysis failed'); } finally { setAnalyzing(false); }
  };

  const toggleExcluded = async (group: DuplicateGroup, identity: string, excluded: boolean) => {
    await excludeDuplicateMember(group.group_id, identity, excluded);
    setGroups((current) => current.map((item) => item.group_id === group.group_id ? { ...item, members: item.members.map((member) => member.identity === identity ? { ...member, excluded } : member) } : item));
  };

  return <section className="max-w-5xl mx-auto w-full space-y-5">
    <div className="flex items-start justify-between"><div><p className="text-xs uppercase tracking-widest text-amber-400 font-semibold">Review only</p><h2 className="text-3xl font-extrabold text-white mt-1">Duplicate Candidates</h2><p className="text-sm text-zinc-400 mt-2">Exact matches are grouped for review. Nothing is deleted automatically.</p></div><button type="button" onClick={onClose}><X className="w-5 h-5 text-zinc-400" /></button></div>
    <div className="flex items-center justify-between bg-zinc-900/60 border border-zinc-800 rounded-2xl p-4"><div className="flex items-center gap-3 text-sm text-zinc-300"><ShieldCheck className="w-5 h-5 text-emerald-400" />Exact-content matching with reviewable exclusions</div><button type="button" onClick={analyze} disabled={analyzing} className="flex items-center gap-2 px-4 py-2 rounded-xl bg-amber-600 hover:bg-amber-500 disabled:opacity-50 text-sm font-semibold"><RefreshCw className={`w-4 h-4 ${analyzing ? 'animate-spin' : ''}`} />{analyzing ? 'Analyzing…' : 'Analyze library'}</button></div>
    {error && <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/30 text-sm text-rose-300">{error}</div>}
    {loading ? <div className="p-12 text-center text-sm text-zinc-500">Loading duplicate groups…</div> : groups.length === 0 ? <div className="p-12 text-center border border-dashed border-zinc-800 rounded-2xl text-sm text-zinc-500">No duplicate groups found. Run an analysis to check the indexed library.</div> : <div className="space-y-4">{groups.map((group) => <div key={group.group_id} className="bg-zinc-900/60 border border-zinc-800 rounded-2xl p-4 space-y-3"><div className="flex justify-between text-sm"><span className="font-semibold text-zinc-200">Exact match · {group.members.length} files</span><span className="text-zinc-500">{group.fingerprint.slice(0, 12)}…</span></div>{group.members.map((member) => <div key={member.identity} className={`flex items-center gap-3 p-3 rounded-xl border ${member.excluded ? 'border-zinc-800 opacity-50' : 'border-zinc-700'}`}><span className="flex-1 min-w-0"><span className="block truncate text-sm text-zinc-200">{member.path}</span><span className="text-xs text-zinc-500">{bytes(member.size)} · {new Date(member.timestamp).toLocaleString()}</span></span><button type="button" onClick={() => toggleExcluded(group, member.identity, !member.excluded)} className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg border border-zinc-700 text-xs text-zinc-300 hover:text-white">{member.excluded ? <><Check className="w-3 h-3" />Excluded</> : 'Exclude'}</button></div>)}</div>)}</div>}
  </section>;
}
