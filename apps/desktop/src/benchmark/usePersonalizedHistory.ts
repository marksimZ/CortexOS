import { useCallback, useEffect, useRef, useState } from "react";
import { benchmark } from "../api/benchmark";
import { errorText } from "../frontend-error";
import type { History, ProfileVersion } from "./personalized";

export function usePersonalizedHistory(workspaceId: string, reportError: (message: string) => void) {
  const [profile, setProfile] = useState<ProfileVersion | null>(null);
  const [loading, setLoading] = useState(true);
  const [jobs, setJobs] = useState<History[]>([]);
  const [batches, setBatches] = useState<History[]>([]);
  const active = useRef<AbortController | null>(null);

  const refresh = useCallback(async () => {
    active.current?.abort();
    const request = new AbortController();
    active.current = request;
    const args = { workspaceId };
    try {
      const [profile, jobs, batches] = await Promise.all([
        benchmark("get_benchmark_profile", args, request.signal),
        benchmark("list_benchmark_generations", args, request.signal),
        benchmark("list_personalized_batches", args, request.signal),
      ]);
      if (request.signal.aborted) return;
      setProfile(profile);
      setJobs(jobs);
      setBatches(batches);
    } catch (error) {
      if (!request.signal.aborted) reportError(errorText(error));
    } finally {
      if (!request.signal.aborted) setLoading(false);
    }
  }, [workspaceId, reportError]);

  useEffect(() => {
    void refresh();
    return () => active.current?.abort();
  }, [refresh]);

  return { profile, setProfile, loading, jobs, batches, refresh };
}
