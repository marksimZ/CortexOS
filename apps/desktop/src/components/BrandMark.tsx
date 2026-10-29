export function BrandMark({ className, size = 28 }: { className?: string; size?: number }) {
  return <img className={className} src="/cortex.svg" width={size} height={size} alt="" aria-hidden="true" />;
}
