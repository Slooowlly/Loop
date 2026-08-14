import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import FlagIcon from "./FlagIcon";
import { extractNationalityLabel } from "../../utils/formatters";

// Dropdown customizado para nacionalidade: o <select> nativo não renderiza
// imagens nas <option>, então as bandeiras (FlagIcon = PNG) só aparecem aqui.
// A lista é renderizada num portal (document.body) com posição fixa porque os
// ancestrais do wizard usam overflow:hidden e cortariam um dropdown absoluto.
function NationalitySelect({ value, onChange, options = [], className = "" }) {
  const [open, setOpen] = useState(false);
  const [rect, setRect] = useState(null);
  const triggerRef = useRef(null);
  const listRef = useRef(null);

  const selected = options.find((option) => option.id === value) ?? options[0];

  function updateRect() {
    if (triggerRef.current) {
      setRect(triggerRef.current.getBoundingClientRect());
    }
  }

  useLayoutEffect(() => {
    if (open) updateRect();
  }, [open]);

  useEffect(() => {
    if (!open) return undefined;

    function handlePointerDown(event) {
      const insideTrigger = triggerRef.current?.contains(event.target);
      const insideList = listRef.current?.contains(event.target);
      if (!insideTrigger && !insideList) setOpen(false);
    }
    function handleKeydown(event) {
      if (event.key === "Escape") setOpen(false);
    }
    function handleReposition() {
      updateRect();
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeydown);
    window.addEventListener("resize", handleReposition);
    // captura para pegar scroll de qualquer container ancestral
    window.addEventListener("scroll", handleReposition, true);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeydown);
      window.removeEventListener("resize", handleReposition);
      window.removeEventListener("scroll", handleReposition, true);
    };
  }, [open]);

  function handleSelect(id) {
    onChange?.({ target: { value: id } });
    setOpen(false);
  }

  return (
    <div className={["relative", className].join(" ")}>
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        aria-haspopup="listbox"
        aria-expanded={open}
        className={[
          "glass-light flex min-h-12 w-full items-center gap-3 rounded-2xl border border-white/10 px-4 py-3",
          "text-sm text-text-primary outline-none transition-glass",
          "focus:border-accent-primary",
          "focus:shadow-[0_0_0_1px_rgba(88,166,255,0.5),0_0_20px_rgba(88,166,255,0.12)]",
        ].join(" ")}
      >
        {selected && <FlagIcon nacionalidade={selected.label} />}
        <span className="flex-1 text-left">
          {selected ? extractNationalityLabel(selected.label) || selected.label : ""}
        </span>
        <svg
          className={["h-4 w-4 text-text-secondary transition-transform", open ? "rotate-180" : ""].join(" ")}
          viewBox="0 0 20 20"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
        >
          <path d="M5 8l5 5 5-5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {open && rect &&
        createPortal(
          <ul
            ref={listRef}
            role="listbox"
            style={{
              position: "fixed",
              top: rect.bottom + 8,
              left: rect.left,
              width: rect.width,
              backgroundColor: "rgb(13, 19, 33)",
            }}
            className={[
              "z-[1000] max-h-64 overflow-y-auto rounded-2xl border border-white/[0.12] p-1 backdrop-blur-xl",
              "shadow-[0_16px_40px_rgba(0,0,0,0.55)]",
            ].join(" ")}
          >
            {options.map((option) => {
              const isSelected = option.id === value;
              return (
                <li key={option.id}>
                  <button
                    type="button"
                    role="option"
                    aria-selected={isSelected}
                    onClick={() => handleSelect(option.id)}
                    className={[
                      "flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left text-sm transition-glass",
                      isSelected
                        ? "bg-accent-primary/15 text-text-primary"
                        : "text-text-secondary hover:bg-white/5 hover:text-text-primary",
                    ].join(" ")}
                  >
                    <FlagIcon nacionalidade={option.label} />
                    <span className="flex-1">
                      {extractNationalityLabel(option.label) || option.label}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>,
          document.body,
        )}
    </div>
  );
}

export default NationalitySelect;
