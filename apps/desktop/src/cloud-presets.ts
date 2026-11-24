export const cloudPresets = [
  {
    id: "deepseek",
    label: "DeepSeek 官方",
    address: "https://api.deepseek.com",
    keyUrl: "https://platform.deepseek.com/api_keys",
    embedding: false,
    reranking: false,
  },
  {
    id: "bailian",
    label: "阿里云百炼（北京）",
    address: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    keyUrl: "https://bailian.console.aliyun.com/cn-beijing/model/settings/api-key",
    embedding: true,
    reranking: false,
  },
  {
    id: "siliconflow",
    label: "硅基流动",
    address: "https://api.siliconflow.cn/v1",
    keyUrl: "https://cloud.siliconflow.cn/account/ak",
    embedding: true,
    reranking: true,
  },
] as const;

export function cloudPresetForAddress(address?: string | null) {
  const base = address?.trim().replace(/\/+$/, "");
  return cloudPresets.find(preset => preset.address === base
    || (preset.id === "deepseek" && base === preset.address + "/v1"));
}
