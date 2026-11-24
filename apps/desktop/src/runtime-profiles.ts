import type { ContextSelection } from "./studio";
import { cloudPresetForAddress } from "./cloud-presets";

export const runtimeModes = { local: "本地", cloud: "云端", hybrid: "混合" } as const;
export const runtimeRoles = { generation: "生成回答", embedding: "生成向量", reranking: "重排结果" } as const;
export type RuntimeMode = keyof typeof runtimeModes;
export type RuntimeRole = keyof typeof runtimeRoles;
export type Target =
  | { kind: "off" }
  | { kind: "local"; model: string | null }
  | { kind: "cloud"; connectionId: string; model: string };
export type ProfileInput = { name: string; mode: RuntimeMode } & Record<RuntimeRole, Target>;
export type Profile = ProfileInput & { id: string; updatedAt: string };
export type Profiles = { activeId: string | null; profiles: Profile[] };
export type ChatRuntimeRoute = { profile: Profile | null; decision: ModelRoute; localBackup: string | null };
export type ModelRoute = {
  task: "question" | "knowledge"; preference: "preferred" | "lower_cost"; reason: string;
  preferredProfileId: string | null; preferredEstimatedMicros: number | null;
  alternative: { profileId: string; estimatedMicros: number | null } | null; selectedProfileId: string | null;
};
export type RuntimeConnection = {
  id: string;
  label: string;
  provider: string;
  address: string | null;
  probes: { model: string; capability: RuntimeRole; state: string }[];
};
export function cloudConnectionLabel(connection: RuntimeConnection): string {
  const preset = connection.provider === "compatible" && cloudPresetForAddress(connection.address);
  return preset && preset.label !== connection.label
    ? connection.label + "（" + preset.label + "）"
    : connection.label;
}

export type CloudConsent = {
  profileId: string;
  updatedAt: string;
  shareSelection: boolean;
  grantedVersions: ContextSelection[];
  rankingVersions: ContextSelection[];
};
export type CloudKnowledgeGrant = ContextSelection & { title: string; grantedAt: string; available: boolean };
export type CloudDisclosure = Partial<Record<RuntimeRole, { connection: RuntimeConnection; grants: CloudKnowledgeGrant[] }>>;
export type CloudDraft = {
  workspace: string;
  conversation: string | null;
  question: string;
  retry: boolean;
  webSources?: boolean;
  selection: ContextSelection[];
  profile: Profile;
  disclosure: CloudDisclosure;
  knowledgeRoute: boolean;
};
