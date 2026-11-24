import { expect, test } from "@playwright/test";
import { createServer } from "node:http";
import { createWorkspace, launchNative, nativeInvoke } from "./native";

test("web search sends only the query and supplies returned sources to chat", async ({}, info) => {
  test.setTimeout(75_000);
  const query = "今天有哪些行业消息";
  const requests: URL[] = [];
  const newsRequests: {query:string|null;token:unknown}[] = [];
  const messages: {role:string; content:string}[][] = [];
  const tokens: (string | string[] | undefined)[] = [];
  const fixture = createServer(async (req, res) => {
    const url = new URL(req.url ?? "/", "http://127.0.0.1");
    res.setHeader("content-type", "application/json");
    if (url.pathname === "/news/search") {
      newsRequests.push({query:url.searchParams.get("q"), token:req.headers["x-subscription-token"]});
      res.setHeader("content-type","application/rss+xml");
      res.end("<rss><channel><item><title>研发 &amp; 进展</title><link>http://www.bing.com/news/apiclick.aspx?url=https%3A%2F%2Fexample.org%2Fstory</link><description><![CDATA[公开新闻摘要]]></description></item></channel></rss>");
    } else if (url.pathname === "/res/v1/web/search") {
      requests.push(url);
      tokens.push(req.headers["x-subscription-token"]);
      if (url.searchParams.get("q") === "限流") {
        res.writeHead(429); res.end(); return;
      }
      const item = {title:"示例行业消息",url:"https://example.com/news",description:"固定搜索摘要，仅用于验收。"};
      res.end(JSON.stringify({web:{results:[item, item, {...item, url:"file:///sample"},
        {title:"无摘要结果",url:"https://example.org/updates",description:null}]}}));
    } else if (url.pathname === "/api/chat") {
      const chunks: Buffer[] = [];
      for await (const chunk of req) chunks.push(chunk);
      messages.push(JSON.parse(Buffer.concat(chunks).toString("utf8")).messages);
      res.end(JSON.stringify({message:{role:"assistant",content:"已整理示例资讯。"},done:true}) + "\n");
    } else if (url.pathname === "/api/show") res.end('{"details":{"format":"gguf"}}');
    else if (url.pathname === "/api/tags") res.end('{"models":[{"name":"qwen3:4b-instruct","size":1}]}');
    else if (url.pathname === "/api/version") res.end('{"version":"fixture"}');
    else res.end("{}");
  });
  await new Promise<void>(done => fixture.listen(0, "127.0.0.1", done));
  const addr = fixture.address();
  if (!addr || typeof addr === "string") throw Error("fixture unavailable");
  const endpoint = "http://127.0.0.1:" + addr.port;
  const previous = process.env.CORTEXOS_TEST_BRAVE_URL;
  process.env.CORTEXOS_TEST_BRAVE_URL = endpoint;
  let app: Awaited<ReturnType<typeof launchNative>> | undefined;
  let workspaceId = "";
  try {
    app = await launchNative(info, {endpoint});
    const {page} = app;
    await createWorkspace(page, "网页搜索验收");
    workspaceId = await page.getByLabel("当前工作区", {exact:true}).inputValue();
    const publicNews = await nativeInvoke<{results:unknown[]}>(page,"search_web",{workspaceId,query});
    expect(publicNews.results).toEqual([{title:"研发 & 进展",url:"https://example.org/story",snippet:"公开新闻摘要"}]);
    expect(newsRequests).toEqual([{query,token:undefined}]);
    await page.getByRole("link", {name:"设置",exact:true}).click();
    await page.getByRole("button", {name:"我的助手",exact:true}).click();
    await page.getByLabel("Brave Search API 密钥").fill("cortexos-brave-fixture");
    await page.getByRole("button", {name:"保存密钥",exact:true}).click();
    await expect(page.getByText("已配置 Brave Search",{exact:true})).toBeVisible();
    const other = await nativeInvoke<{id:string}>(page, "create_workspace", {name:"隔离工作区"});
    expect(await nativeInvoke(page, "read_web_search_status", {workspaceId:other.id})).toEqual({configured:false});
    await expect(nativeInvoke(page,"search_web",{workspaceId:other.id,query,provider:"brave"})).rejects.toThrow();
    expect(requests).toHaveLength(0);
    await page.getByRole("link", {name:"对话",exact:true}).click();
    await page.getByRole("textbox",{name:"你的问题"}).fill(query);
    await page.getByRole("button", {name:"联网查询",exact:true}).click();
    await expect(page.getByText("示例行业消息",{exact:true})).toHaveCount(1);
    expect(requests).toHaveLength(1);
    expect(requests[0].searchParams.get("q")).toBe(query);
    expect(requests[0].searchParams.get("freshness")).toBe("pd");
    expect(tokens).toEqual(["cortexos-brave-fixture"]);
    expect(messages).toHaveLength(0);
    await page.screenshot({path:info.outputPath("news-results.png"),animations:"disabled"});
    await page.getByRole("button",{name:"依据来源回答",exact:true}).click();
    await expect(page.getByText("已整理示例资讯。",{exact:true})).toBeVisible();
    expect(messages).toHaveLength(1);
    expect(messages[0].at(-1)?.content).toContain("https://example.com/news");
    expect(messages[0].at(-1)?.content).toContain("固定搜索摘要");
    expect(messages[0][0].content).not.toContain("固定搜索摘要");
    expect(JSON.stringify(messages)).not.toContain("cortexos-brave-fixture");
    expect(JSON.stringify(messages)).not.toContain("file:///");
    const [run] = await nativeInvoke<{conversationId:string}[]>(page,"list_runs",{workspaceId});
    const history = await nativeInvoke<{role:string;content:string}[]>(page,"load_messages",
      {workspaceId,conversationId:run.conversationId});
    expect(history.find(row => row.role === "user")?.content).toBe(messages[0].at(-1)?.content);
    await expect(nativeInvoke(page,"search_web",{workspaceId,query:"限流",provider:"brave"})).rejects.toThrow(/限制/);
  } finally {
    try {
      if (app && workspaceId) await nativeInvoke(app.page,"save_web_search_key",{workspaceId,key:""});
    } finally {
      if (previous === undefined) delete process.env.CORTEXOS_TEST_BRAVE_URL;
      else process.env.CORTEXOS_TEST_BRAVE_URL = previous;
      await app?.stop();
      fixture.closeAllConnections(); fixture.close();
    }
  }
});

test("optional live public news without credentials", async ({}, info) => {
  test.skip(process.env.CORTEXOS_PUBLIC_NEWS_LIVE !== "1", "Opt in to a public news request");
  const app = await launchNative(info, {endpoint:"http://127.0.0.1:1"});
  try {
    const {page} = app;
    await createWorkspace(page,"免密钥公开新闻");
    await page.setViewportSize({width:1366,height:768});
    await page.getByRole("textbox",{name:"你的问题"}).fill("人工智能");
    await page.getByRole("button",{name:"联网查询",exact:true}).click();
    const panel = page.getByRole("region",{name:"联网查询来源"});
    const source = panel.getByRole("link").first();
    await expect(source).toBeVisible({timeout:20_000});
    await expect(source).toHaveAttribute("href", /^https?:\/\//);
    await page.screenshot({path:info.outputPath("public-news.png"),animations:"disabled"});
    await page.setViewportSize({width:850,height:640});
    await page.getByRole("combobox",{name:"外观",exact:true}).selectOption("dark");
    await expect(source).toBeVisible();
    await page.screenshot({path:info.outputPath("public-news-narrow-dark.png"),animations:"disabled"});
  } finally {
    await app.stop();
  }
});
