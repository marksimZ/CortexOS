import { createClient, type Command, type WorkspaceArgs } from "./request";

export type WebSearchProvider = "news" | "brave";
export const webSearchProviderLabel = {
  news: "公开新闻（Bing News）",
  brave: "Brave Search",
} satisfies Record<WebSearchProvider, string>;

export type WebSearchResult = {
  title: string;
  url: string;
  snippet: string;
};

export type WebSearchResponse = {
  results: WebSearchResult[];
  searchedAt: string;
};

export const webSearch = createClient<{
  read_web_search_status: Command<WorkspaceArgs, { configured: boolean }>;
  save_web_search_key: Command<WorkspaceArgs & { key: string }, void>;
  search_web: Command<
    WorkspaceArgs & { query: string; provider: WebSearchProvider },
    WebSearchResponse
  >;
}>();
