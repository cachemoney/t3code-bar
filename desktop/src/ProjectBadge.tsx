import { useState } from 'react';
import { projectBadge } from './project-badge';

export function ProjectBadge({ title, icon, favicon }: { title: string; icon: string | null; favicon?: string | null }) {
  const badge = projectBadge(title, icon);
  const [failedFavicon, setFailedFavicon] = useState<string | null>(null);
  let usesDefault = !icon;
  if (icon) {
    try {
      const payload = JSON.parse(icon) as { kind?: string; emoji?: string; name?: string };
      usesDefault = !['monogram', 'lucide'].includes(payload?.kind ?? '') && !(payload?.kind === 'emoji' && payload.emoji);
    } catch { usesDefault = true; }
  }
  if (badge.kind !== 'no-project' && usesDefault && favicon && favicon !== failedFavicon) return <span className="project-badge" aria-hidden="true"><img src={favicon} alt="" style={{ width: '100%', height: '100%', objectFit: 'contain' }} onError={() => setFailedFavicon(favicon)} /></span>;
  return <span className={`project-badge badge-${badge.kind}`} style={{ color: badge.color, backgroundColor: badge.kind === 'monogram' ? `${badge.color}24` : 'transparent' }} aria-hidden="true">
    {badge.kind === 'folder' ? <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M3 7V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" /></svg>
      : badge.kind === 'no-project' ? <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d="M9 3h1m4 0h1m3 0q3 0 3 3m0 4v1m0 4v1m0 2q0 3-3 3m-4-4h1m-5 0H7l-4 4V6q0-3 3-3" /></svg> : badge.text}
  </span>;
}
