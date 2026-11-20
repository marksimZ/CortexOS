import type {
  Batch,
  History,
  Job,
  ProfileVersion,
} from "../benchmark/personalized";
import type { Connection } from "../benchmark/cloud";
import type { ResultFilter } from "../benchmark/ReportOptions";
import type {
  ReportScope,
  ResultDetail,
  ResultRow,
} from "../benchmark/ResultDrawer";
import { createClient, type Command, type WorkspaceArgs } from "./request";

type ReportArgs = WorkspaceArgs & { batchId: string; scope: ReportScope };
type PageArgs = { pageSize: number; filter: ResultFilter };
export type ResultLocation = {
  ordinal: number;
  modelIndex: number;
  page: number | null;
  matchesFilter: boolean;
};
export type ResultPage = {
  batchId: string;
  modelIndex: number;
  page: number;
  pageSize: number;
  total: number;
  plannedTotal: number;
  rows: ResultRow[];
};

export const benchmark = createClient<{
  personalized_access: Command<undefined, Connection>;
  list_standard_benchmarks: Command<WorkspaceArgs, History[]>;
  list_personalized_batches: Command<WorkspaceArgs, History[]>;
  get_benchmark_profile: Command<WorkspaceArgs, ProfileVersion | null>;
  list_benchmark_generations: Command<WorkspaceArgs, History[]>;
  get_benchmark_generation: Command<WorkspaceArgs & { jobId: string }, Job>;
  get_benchmark_overview: Command<ReportArgs, Batch>;
  get_benchmark_result_page: Command<
    ReportArgs & PageArgs & { modelIndex: number; page: number },
    ResultPage
  >;
  get_benchmark_result_detail: Command<
    ReportArgs & { ordinal: number },
    ResultDetail
  >;
  get_benchmark_result_location: Command<
    ReportArgs &
      PageArgs & { ordinal: number | null; executionRunId: string | null },
    ResultLocation
  >;
}>();
