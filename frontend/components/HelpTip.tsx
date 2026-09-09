import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { HelpCircle } from 'lucide-react';

interface Props {
  /** Section name — used for the accessible label. */
  label: string;
  /** Brief help content shown in the popover. */
  children: React.ReactNode;
}

const POPOVER_WIDTH = 260;
const GAP = 6;

/** Small "?" affordance that reveals a brief explanation on hover or click. */
function HelpTip({ label, children }: Props) {
  const btnRef = useRef<HTMLButtonElement>(null);
  const [pinned, setPinned] = useState(false);
  const [hovered, setHovered] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  const open = pinned || hovered;

  // Anchor the fixed popover below-left of the icon, clamped to the viewport.
  function updatePosition() {
    const el = btnRef.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const left = Math.min(r.left, window.innerWidth - POPOVER_WIDTH - GAP);
    setPos({ top: r.bottom + GAP, left: Math.max(GAP, left) });
  }

  useEffect(() => {
    if (open) updatePosition();
  }, [open]);

  // Dismiss a pinned popover on Escape or an outside click.
  useEffect(() => {
    if (!pinned) return;
    function handleKey(e: KeyboardEvent) {
      if (e.key === 'Escape') setPinned(false);
    }
    function handleClickOutside(e: MouseEvent) {
      if (!btnRef.current?.contains(e.target as Node)) setPinned(false);
    }
    document.addEventListener('keydown', handleKey);
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('keydown', handleKey);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [pinned]);

  function handleClick(e: React.MouseEvent) {
    // Stop clicks from reaching a parent collapse toggle.
    e.stopPropagation();
    setPinned((p) => !p);
  }

  return (
    <span
      className="help-tip"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <button
        ref={btnRef}
        type="button"
        className="help-tip-btn"
        aria-label={`About ${label}`}
        aria-expanded={open}
        onClick={handleClick}
      >
        <HelpCircle size={14} />
      </button>
      {open &&
        pos &&
        createPortal(
          <div className="help-tip-pop" role="tooltip" style={{ top: pos.top, left: pos.left }}>
            {children}
          </div>,
          document.body
        )}
    </span>
  );
}

export default HelpTip;
