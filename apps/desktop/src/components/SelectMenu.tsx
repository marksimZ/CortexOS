import { ChevronDown } from "lucide-react";
import type { SelectHTMLAttributes } from "react";

export function SelectMenu({ children, className = "", multiple, size, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  if (multiple || (size ?? 1) > 1) {
    return <select {...props} multiple={multiple} size={size} className={className}>{children}</select>;
  }
  return <span className="select-field">
    <select {...props} size={size} className={`select-control ${className}`}>{children}</select>
    <ChevronDown className="select-chevron" size={14} aria-hidden="true" />
  </span>;
}
