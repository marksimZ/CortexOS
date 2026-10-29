export type Provider = "openai" | "anthropic" | "gemini";
export interface Connection {
  state: string;
  provider: Provider | null;
  model: string | null;
  checkedAt: string | null;
  allowed: boolean;
  models: { id: string; generation: "listed" | "unknown" }[];
}
export const providers: Record<Provider, string> = { openai: "OpenAI", anthropic: "Anthropic", gemini: "Google Gemini" };
export const statuses: Record<string, string> = {
  unconfigured: "尚未连接", verifying: "正在验证连接…", model_required: "密钥验证通过，请选择模型",
  verified: "连接已验证", invalid_credential: "密钥无效或已过期，请更换密钥",
  model_denied: "没有所选模型或目录的访问权限，请检查账号权限或更换模型",
  quota_exceeded: "服务商报告额度不足，请检查账户额度",
  rate_limited: "服务商限制了请求频率，请稍后重试；无法据此判断余额",
  network_unavailable: "暂时无法连接服务商，请检查网络后重试；密钥已保留",
  service_error: "服务商响应异常，请稍后重试",
  revalidation_required: "连接需要重新验证", cancelled: "验证已取消，可重新验证或解除绑定",
};
