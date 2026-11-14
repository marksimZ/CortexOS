import type { ReactNode } from 'react';

export function FitStatus({children,tone='neutral'}:{children:ReactNode;tone?:'neutral'|'attention'|'success'}){
 return <span className="fit-status" data-tone={tone}>{children}</span>;
}
