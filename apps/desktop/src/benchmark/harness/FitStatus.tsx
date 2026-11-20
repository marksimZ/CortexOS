import type { ReactNode } from 'react';
import { StatusLabel } from "../../components/StatusLabel";

export function FitStatus({children,tone='neutral'}:{children:ReactNode;tone?:'neutral'|'attention'|'success'}){
 return <StatusLabel className="fit-status" tone={tone}>{children}</StatusLabel>;
}
