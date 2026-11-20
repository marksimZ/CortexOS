import { useId, useState } from "react";

export function NumberField({ label, min, max, step = 1, value, onChange, readOnly, error }: {
  label: string;
  min: number;
  max: number;
  step?: number | "any";
  value: number | null;
  onChange: (value: number | null) => void;
  readOnly?: boolean;
  error?: string;
}) {
  const id = useId();
  const [checked, setChecked] = useState(false);
  const invalid = error || (checked ? value === null || Number.isNaN(value) ? "请输入数字。" :
    !Number.isFinite(value) || value < min || value > max || (step === 1 && !Number.isInteger(value))
      ? `请输入 ${min} 至 ${max} 之间的${step === 1 ? "整数" : "数字"}。` : undefined : undefined);
  return <div className="field">
    <label htmlFor={id}>{label}</label>
    <input id={id} type="number" required min={min} max={max} step={step} readOnly={readOnly}
      value={value !== null && Number.isFinite(value) ? value : ""}
      aria-invalid={!!invalid || undefined} aria-describedby={invalid ? `${id}-error` : undefined}
      onChange={event => onChange(Number.isFinite(event.currentTarget.valueAsNumber) ? event.currentTarget.valueAsNumber : null)}
      onBlur={() => setChecked(true)} onInvalid={event => {
        setChecked(true);
        const parent = event.currentTarget.closest("details:not([open])");
        if (parent instanceof HTMLDetailsElement) parent.open = true;
      }} />
    {invalid && <small className="form-error" id={`${id}-error`}>{invalid}</small>}
  </div>;
}

export function validModelCapacity(context: number, response: number) {
  return Number.isInteger(context) && Number.isInteger(response) && context > response + 512;
}

export const capacityError = "总容量必须比回答上限多出至少 513 Token。";
