/** Shrink only the basename; the extension remains visible and copy keeps the full name. */
export function FileName({ name }: { name: string }) {
  const dot = name.lastIndexOf(".");
  const split = dot > 0 && dot < name.length - 1;
  return <span className="file-name" title={name} aria-label={name}>
    <span className="file-name-stem">{split ? name.slice(0, dot) : name}</span>
    {split && <span className="file-name-extension">{name.slice(dot)}</span>}
  </span>;
}
