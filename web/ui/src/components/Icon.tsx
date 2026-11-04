import type { ReactNode } from 'react';

export interface IconProps {
  name: string;
  size?: number;
  className?: string;
  title?: string;
}

const paths: Record<string, ReactNode> = {
  agents: <><rect x="4" y="6" width="16" height="14" rx="4" /><path d="M12 3v3M8 11v2m8-2v2m-7 3h6M1 11v4m22-4v4" /></>,
  memory: <><path d="M9 4a3 3 0 0 0-5 2 4 4 0 0 0-1 7 4 4 0 0 0 3 6 3 3 0 0 0 6 0V5a3 3 0 0 0-3-3v2Zm6 0a3 3 0 0 1 5 2 4 4 0 0 1 1 7 4 4 0 0 1-3 6 3 3 0 0 1-6 0V5a3 3 0 0 1 3-3v2Z" /><path d="M6 8c3 0 3 4 0 4m12-4c-3 0-3 4 0 4M8 17h4m4-3h-4" /></>,
  mcp: <><path d="M8 3v4m8-4v4M7 7h10v4a5 5 0 0 1-10 0V7Zm5 9v5m-3 0h6" /></>,
  vault: <><rect x="3" y="3" width="18" height="18" rx="3" /><circle cx="12" cy="12" r="4" /><path d="M12 8v2m0 4v2m-4-4h2m4 0h2M5 7H3m2 10H3" /></>,
  updates: <><path d="M20 8a8 8 0 0 0-14-3L3 8m0-5v5h5M4 16a8 8 0 0 0 14 3l3-3m-5 0h5v5" /></>,
  'chevron-left': <path d="m14 6-6 6 6 6" />,
  'chevron-right': <path d="m10 6 6 6-6 6" />,
  'chevron-down': <path d="m6 9 6 6 6-6" />,
  sun: <><circle cx="12" cy="12" r="4" /><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5" /></>,
  moon: <path d="M20.9 13A9 9 0 0 1 11 3.1 9 9 0 1 0 20.9 13Z" />,
  monitor: <><rect x="3" y="3" width="18" height="14" rx="2" /><path d="M8 21h8m-4-4v4" /></>,
  refresh: <><path d="M20 7v5h-5M4 17v-5h5" /><path d="M6 6a8 8 0 0 1 13 2l1 4M4 12l1 4a8 8 0 0 0 13 2" /></>,
  search: <><circle cx="10.5" cy="10.5" r="6.5" /><path d="m16 16 5 5" /></>,
  plus: <path d="M12 5v14M5 12h14" />,
  check: <path d="m5 12 4 4L19 6" />,
  alert: <><path d="m10.3 3.9-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.7-3.1l-8-14a2 2 0 0 0-3.4 0Z" /><path d="M12 9v4m0 4h.01" /></>,
  download: <><path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5" /></>,
  trash: <><path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7" /></>,
  copy: <><rect x="8" y="8" width="12" height="13" rx="2" /><path d="M16 8V3H3v13h5" /></>,
  key: <><circle cx="8" cy="9" r="5" /><path d="m12 12 8 8m-3-3 3-3m-6 0 3-3M7 8h.01" /></>,
  external: <><path d="M14 3h7v7m0-7L10 14M10 3H3v18h18v-7" /></>,
  info: <><circle cx="12" cy="12" r="9" /><path d="M12 11v6m0-10h.01" /></>,
  x: <path d="m6 6 12 12M6 18 18 6" />,
  settings: <><path d="m9 3-1 3-3 1-2 3 2 2-1 3 3 3 3-1 2 2 3-1 1-3 3-1 1-3-2-2 1-3-3-3-3 1-2-2Z" /><circle cx="11" cy="11" r="3" /></>,
};

export function Icon({ name, size = 18, className, title }: IconProps) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden={title ? undefined : true}
      role={title ? 'img' : undefined}
      aria-label={title}
      focusable="false"
    >
      {title && <title>{title}</title>}
      {paths[name] ?? paths.info}
    </svg>
  );
}
