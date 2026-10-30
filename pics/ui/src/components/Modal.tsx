import { useEffect, useRef, type ReactNode } from "react";

interface Props {
  title: string;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
}

/** Boîte de dialogue accessible : focus à l'ouverture, Échap pour fermer, focus rendu à la fermeture. */
export function Modal({ title, onClose, children, wide }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const first = ref.current?.querySelector<HTMLElement>("input, select, button, [tabindex]");
    first?.focus();
    return () => previous?.focus?.();
  }, []);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== "Tab" || !ref.current) return;
    // Piège à focus : Tab reste dans la boîte.
    const items = [...ref.current.querySelectorAll<HTMLElement>("input, select, button, [tabindex]:not([tabindex='-1'])")].filter((el) => !el.hasAttribute("disabled"));
    const firstEl = items[0];
    const lastEl = items[items.length - 1];
    if (!firstEl || !lastEl) return;
    if (e.shiftKey && document.activeElement === firstEl) {
      e.preventDefault();
      lastEl.focus();
    } else if (!e.shiftKey && document.activeElement === lastEl) {
      e.preventDefault();
      firstEl.focus();
    }
  };

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className={wide ? "modal wide" : "modal"} role="dialog" aria-modal="true" aria-label={title} onKeyDown={onKeyDown}>
        <h2>{title}</h2>
        {children}
      </div>
    </div>
  );
}
