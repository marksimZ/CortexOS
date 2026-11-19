import { useState } from 'react';
import { FitStatus } from './FitStatus';
import { errorText } from '../personalized';

export interface PendingJob {
  id: string;
  status: 'waiting' | 'starting';
}

export function JobRequest({ job, cancel, onError }: {
  job: PendingJob;
  cancel: () => Promise<unknown>;
  onError: (error: string) => void;
}) {
  const [cancelling, setCancelling] = useState(false);
  async function stopWaiting() {
    setCancelling(true);
    try { await cancel(); }
    catch (error) { onError(errorText(error)); }
    finally { setCancelling(false); }
  }
  return <div className="cloud-actions">
    <span role="status"><FitStatus>{job.status === 'waiting' ? '正在等待运行资源' : '正在检查启动条件'}</FitStatus></span>
    {job.status === 'waiting' && <button className="button secondary" disabled={cancelling} onClick={() => void stopWaiting()}>
      {cancelling ? '正在取消…' : '取消等待'}
    </button>}
  </div>;
}
