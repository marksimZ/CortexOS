import { StatusLabel } from "../components/StatusLabel";
import { stateName } from "./personalized";

export function ResultStatus({ state }: { state: string }) {
  const tone = state === "Passed" ? "success"
    : state === "Failed" || state === "Execution Error" || state === "Scoring Error" ? "attention"
    : state === "Needs Review" ? "warning"
    : "neutral";
  return <StatusLabel tone={tone}>{stateName[state] ?? state}</StatusLabel>;
}
