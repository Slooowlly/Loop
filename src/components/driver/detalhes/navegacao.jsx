export function NavChevron({ direction }) {
  const path = direction === "up" ? "M3 7.5 6 4.5l3 3" : "m3 4.5 3 3 3-3";

  return (
    <svg
      viewBox="0 0 12 12"
      aria-hidden="true"
      className="h-3.5 w-3.5 flex-shrink-0"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d={path} />
    </svg>
  );
}
