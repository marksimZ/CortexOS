import { AppDialog, openDialog } from "./components/AppDialog";
import { SelectMenu } from "./components/SelectMenu";
import { SourceBrowser, type SourceRef } from "./SourceBrowser";
import { knowledge } from "./api/knowledge";
import { notify } from "./components/Toast";
import { StatusLabel } from "./components/StatusLabel";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { BookOpen, Check, Pencil, Plus, Search, X, ListTree, History } from "lucide-react";
import { navigate, useNavigation } from "./navigation";
import { errorText, type Studio } from "./studio";
import { kinds, type EditKnowledge, type KnowledgeDetail, type KnowledgeNode } from "./knowledge-types";

const status = (node: KnowledgeNode) =>
  node.latestVersion === node.effectiveVersion
    ? "已确认"
    : node.effectiveVersion
      ? "有新草稿"
      : "待确认";
const date = (value: string) => new Date(value).toLocaleString("zh-CN");

export function KnowledgePage({
  studio,
  selected,
  onSelect,
  revision,
  onEdit,
  requestedVersion,
}: {
  studio: Studio;
  selected: string | null;
  requestedVersion?: number;
  onSelect: (id: string | null) => void;
  revision: number;
  onEdit: (request: EditKnowledge) => void;
}) {
  const workspace = studio.workspaceId;
  const [listOpen, setListOpen] = useState(true);
  const [sourcesOpen, setSourcesOpen] = useState(false);
  const [items, setItems] = useState<KnowledgeNode[]>([]);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [refresh, setRefresh] = useState(0);
  useEffect(() => {
    let current = true;
    setItems([]);
    setError(null);
    setLoading(false);
    if (!studio.desktop || !workspace) return;
    setLoading(true);
    void knowledge("list_knowledge", { workspaceId: workspace })
      .then((data) => {
        if (current) setItems(data);
      })
      .catch((e) => {
        if (current) setError(errorText(e));
      })
      .finally(() => {
        if (current) setLoading(false);
      });
    return () => {
      current = false;
    };
  }, [workspace, studio.desktop, revision, refresh]);
  const shown = items.filter(
    (item) =>
      item.title.toLocaleLowerCase().includes(query.toLocaleLowerCase()) &&
      (filter === "all" ||
        (filter === "pending"
          ? item.latestVersion !== item.effectiveVersion
          : item.latestVersion === item.effectiveVersion)),
  );
  return (
    <section
      className="standard-page knowledge-page"
      aria-labelledby="page-title"
    >
      {sourcesOpen && workspace && (
        <SourceBrowser
          workspace={workspace}
          onClose={() => setSourcesOpen(false)}
          onDraft={(request) => {
            setSourcesOpen(false);
            onEdit(request);
          }}
        />
      )}
      <header className="page-heading">
        <div className="knowledge-page-location">
          <button
            className="button secondary"
            aria-label={listOpen ? "收起知识列表" : "展开知识列表"}
            title={listOpen ? "收起知识列表" : "展开知识列表"}
            aria-expanded={listOpen}
            aria-controls="knowledge-list-sidebar"
            onClick={() => setListOpen(!listOpen)}
          >
            <ListTree size={16} aria-hidden="true" />
            知识列表
          </button>
          <h1 id="page-title" className="sr-only">知识</h1>
        </div>
        <div className="source-actions">
          <button
            className="button secondary"
            disabled={!studio.desktop || !workspace}
            onClick={() => setSourcesOpen(true)}
          >
            资料文件
          </button>
          <button
            className="button primary"
            disabled={!studio.desktop || !workspace}
            onClick={() => onEdit({ workspaceId: workspace! })}
          >
            <Plus size={16} />
            新建知识
          </button>
        </div>
      </header>
      {error && (
        <div className="error-banner" role="alert">
          {error}
          <button
            className="button text-button"
            onClick={() => setRefresh((n) => n + 1)}
          >
            重新读取
          </button>
        </div>
      )}
      <div
        className={`knowledge-workbench ${selected ? "has-selection" : ""} ${listOpen ? "" : "list-collapsed"}`}
      >
        <aside id="knowledge-list-sidebar" className="knowledge-sidebar">
          <section className="knowledge-list-pane" aria-label="知识列表">
            <div className="knowledge-filters">
              <label className="search-field">
                <Search size={16} />
                <input
                  aria-label="搜索知识标题"
                  placeholder="搜索标题"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                />
              </label>
              <SelectMenu
                aria-label="知识状态筛选"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
              >
                <option value="all">全部状态</option>
                <option value="pending">待确认 / 新草稿</option>
                <option value="confirmed">已确认</option>
              </SelectMenu>
            </div>
            {loading ? (
              <p className="inline-loading" role="status">
                正在读取知识…
              </p>
            ) : shown.length ? (
              <ul className="knowledge-list">
                {shown.map((item) => (
                  <li key={item.id}>
                    <button
                      className="knowledge-row"
                      aria-current={selected === item.id ? "true" : undefined}
                      onClick={() => onSelect(item.id)}
                    >
                      <span className="knowledge-row-title">{item.title}</span>
                      <span className="knowledge-row-meta">
                        <span>{kinds[item.kind]}</span>
                        <StatusLabel tone={item.latestVersion === item.effectiveVersion ? "success" : "warning"}>
                          {status(item)}
                        </StatusLabel>
                        <span>v{item.latestVersion}</span>
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <div className="knowledge-empty">
                <BookOpen size={28} />
                <h2>
                  {query || filter !== "all" ? "没有匹配的知识" : "还没有知识"}
                </h2>
                <p>
                  {!studio.desktop
                    ? "在桌面程序中管理知识。"
                    : !workspace
                      ? "先创建或选择工作区。"
                      : "手动记录，或从回答保存草稿。"}
                </p>
                {(query || filter !== "all") && (
                  <button
                    className="button text-button"
                    onClick={() => {
                      setQuery("");
                      setFilter("all");
                    }}
                  >
                    清除筛选
                  </button>
                )}
              </div>
            )}
          </section>
        </aside>
        <KnowledgeInspector
          studio={studio}
          workspace={workspace}
          selected={selected}
          requestedVersion={requestedVersion}
          revision={revision}
          onBack={() => {
            setListOpen(true);
            onSelect(null);
          }}
          onEdit={onEdit}
          onChanged={() => setRefresh((n) => n + 1)}
        />
      </div>
    </section>
  );
}

export function KnowledgeInspector({
  studio,
  workspace,
  selected,
  revision,
  onBack,
  onEdit,
  onChanged,
  requestedVersion,
}: {
  studio: Pick<Studio, "openConversation" | "reportError">;
  workspace: string | null;
  selected: string | null;
  revision: number;
  onBack: () => void;
  onEdit: (request: EditKnowledge) => void;
  onChanged?: () => void;
  requestedVersion?: number;
}) {
  const route = useNavigation();
  const [sourceFocus, setSourceFocus] = useState<SourceRef | null>(null);
  const [detail, setDetail] = useState<KnowledgeDetail | null>(null);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [version, setVersion] = useState<number | null>(null);
  const [approving, setApproving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const historyDialog = useRef<HTMLDialogElement>(null);
  const historyButton = useRef<HTMLButtonElement>(null);
  const closeHistory = () => {
    historyDialog.current?.close();
    historyButton.current?.focus();
  };
  useEffect(() => {
    historyDialog.current?.close();
    setSourceFocus(null);
  }, [workspace, selected]);
  const approval = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    let current = true;
    setDetail(null);
    setError(null);
    setLoadingDetail(false);
    if (!workspace || !selected) return;
    setLoadingDetail(true);
    void knowledge("get_knowledge", { workspaceId: workspace, nodeId: selected })
      .then((data) => {
        if (current) {
          setDetail(data);
        }
      })
      .catch((e) => {
        if (current) setError(errorText(e));
      })
      .finally(() => {
        if (current) setLoadingDetail(false);
      });
    return () => {
      current = false;
    };
  }, [workspace, selected, revision, refresh]);
  useEffect(() => {
    setVersion(requestedVersion ?? null);
    if (detail) {
      setError(
        requestedVersion &&
          !detail.versions.some((v) => v.version === requestedVersion)
          ? "指定知识版本不存在，请从版本历史中选择已有版本。"
          : null,
      );
    }
  }, [requestedVersion, detail]);

  const currentVersion = detail?.versions.find(
    (item) => item.version === (version ?? detail.node.latestVersion),
  );
  const effective = detail?.versions.find(
    (item) => item.version === detail.node.effectiveVersion,
  );
  const history =
    detail && currentVersion ? (
      <details
        className="knowledge-disclosure version-history"
        open={route.page === "knowledge" ? true : undefined}
      >
        <summary>
          {route.page === "knowledge"
            ? `${detail.versions.length} 个版本`
            : `版本历史（${detail.versions.length}）`}
        </summary>
        <div className="version-picker">
          <label htmlFor="knowledge-version">查看版本</label>
          <SelectMenu
            id="knowledge-version"
            value={currentVersion.version}
            onChange={(e) => {
              setVersion(Number(e.target.value));
              navigate(route.page, {
                workspace,
                node: selected,
                version: Number(e.target.value),
                mode: route.page === "graph" ? "knowledge" : undefined,
                back: route.back,
              });
            }}
          >
            {detail.versions.map((item) => (
              <option key={item.version} value={item.version}>
                v{item.version}，{item.approvedAt ? "已确认过" : "草稿"}
              </option>
            ))}
          </SelectMenu>
        </div>
        <ol className="knowledge-history">
          {detail.versions.map((item) => (
            <li key={item.version}>
              <button
                className="version-entry"
                aria-current={
                  currentVersion.version === item.version ? "true" : undefined
                }
                onClick={() => {
                  setVersion(item.version);
                  navigate(route.page, {
                    workspace,
                    node: selected,
                    version: item.version,
                    mode: route.page === "graph" ? "knowledge" : undefined,
                    back: route.back,
                  });
                }}
              >
                <strong>{item.changeReason}</strong>
                <span>
                  v{item.version}，{item.approvedAt ? "已确认过" : "草稿"}
                </span>
              </button>
              <span>保存于 {date(item.createdAt)}</span>
              {item.approvedAt && <span>确认于 {date(item.approvedAt)}</span>}
            </li>
          ))}
        </ol>
      </details>
    ) : (
      <p className="history-empty">选择知识后查看版本历史。</p>
    );
  return (
    <>
      {sourceFocus && workspace && (
        <SourceBrowser
          key={`${workspace}:${selected}`}
          workspace={workspace}
          initial={sourceFocus}
          onClose={() => setSourceFocus(null)}
          onDraft={(request) => {
            setSourceFocus(null);
            onEdit(request);
          }}
        />
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
          <button
            className="button text-button"
            onClick={() => setRefresh((n) => n + 1)}
          >
            重新读取
          </button>
        </p>
      )}{" "}
      <section className="knowledge-detail" aria-label="知识详情">
        {selected && (
          <button
            className="button text-button knowledge-back"
            onClick={() => onBack()}
          >
            返回知识列表
          </button>
        )}
        {loadingDetail ? (
          <p role="status">正在读取版本…</p>
        ) : detail && currentVersion ? (
          <>
            <header className="knowledge-detail-heading">
              <div>
                <span className="knowledge-kind">
                  {kinds[currentVersion.kind]} / v{currentVersion.version}
                </span>
                <h2>{currentVersion.title}</h2>
                <p className="knowledge-state">
                  <StatusLabel tone={detail.node.latestVersion === detail.node.effectiveVersion ? "success" : "warning"}>
                    {status(detail.node)}
                  </StatusLabel>
                  <span>
                    {detail.node.effectiveVersion
                      ? `当前确认 v${detail.node.effectiveVersion}`
                      : "确认后用于回答"}
                  </span>
                  {currentVersion.validity?.validFrom && <span>{currentVersion.validity.validFrom} 起生效</span>}
                  {currentVersion.validity?.validTo && <span>{currentVersion.validity.validTo} 起失效</span>}
                  {!!currentVersion.lineage?.replacesVersion && <span>接替 v{currentVersion.lineage.replacesVersion}</span>}
                  {!!currentVersion.lineage?.replacedByVersion && <span>后由 v{currentVersion.lineage.replacedByVersion} 接替</span>}
                </p>
              </div>
              <div className="knowledge-detail-actions">
                {route.page === "knowledge" && (
                  <button
                    ref={historyButton}
                    className="button secondary"
                    onClick={() => openDialog(historyDialog.current)}
                    aria-haspopup="dialog"
                  >
                    <History size={16} />
                    版本历史
                  </button>
                )}
                <button
                  className="button secondary"
                  onClick={() => onEdit({ workspaceId: workspace!, detail })}
                >
                  <Pencil size={15} />
                  修改草稿
                </button>
              </div>
            </header>
            <div className="knowledge-prose">{currentVersion.content}</div>
            {route.page === "knowledge" && (
              <button
                className="button text-button"
                onClick={() =>
                  navigate("graph", {
                    workspace,
                    mode: "knowledge",
                    node: selected,
                    version: currentVersion.version,
                    back: route.back,
                  })
                }
              >
                在知识图谱定位
              </button>
            )}
            {detail.node.latestVersion !== detail.node.effectiveVersion && (
              <div className="knowledge-review">
                <span>最新草稿 v{detail.node.latestVersion}</span>
                <button
                  className="button primary"
                  onClick={() => openDialog(approval.current)}
                >
                  <Check size={16} />
                  检查并确认
                </button>
              </div>
            )}
            {effective && effective.version !== currentVersion.version && (
              <details className="knowledge-disclosure">
                <summary>对照回答使用的版本 v{effective.version}</summary>
                <h3>{effective.title}</h3>
                <p>{kinds[effective.kind]}</p>
                <div className="knowledge-prose">{effective.content}</div>
              </details>
            )}
            {route.page === "knowledge" ? (
              createPortal(
              <AppDialog
                ref={historyDialog}
                className="knowledge-history-drawer panel-dialog"
                aria-labelledby="knowledge-history-title"
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    event.preventDefault();
                    closeHistory();
                  }
                }}
              >
                <header className="history-drawer-heading panel-header">
                  <h2 id="knowledge-history-title">版本历史</h2>
                  <button
                    className="icon-button"
                    aria-label="关闭版本历史"
                    onClick={closeHistory}
                  >
                    <X size={20} />
                  </button>
                </header>
                <div className="panel-body">
                  <p className="history-document-title">{detail.node.title}</p>
                  {history}
                </div>
              </AppDialog>, document.body)
            ) : (
              history
            )}
            {(detail.sourceEvidence ?? [])
              .filter((e) => e.knowledgeVersion === currentVersion.version)
              .map((evidence) => (
                <details
                  key={`${evidence.sourceId}:${evidence.version}`}
                  className="knowledge-disclosure"
                >
                  <summary>
                    所用文件：{evidence.name}
                    {!evidence.enabled && "（已停用）"}
                  </summary>
                  <p>
                    文件版本 v{evidence.version}，第 {evidence.startLine}–
                    {evidence.endLine} 行
                  </p>
                  <blockquote>{evidence.excerpt}</blockquote>
                  <button
                    className="button secondary"
                    onClick={() => setSourceFocus(evidence)}
                  >
                    查看文件内容
                  </button>
                </details>
              ))}
            <details className="knowledge-disclosure">
              <summary>
                {detail.sourceEvidence?.length
                  ? "这条知识是怎么来的"
                  : detail.sourceExcerpt
                    ? "回答来源"
                    : "来源：手动记录"}
              </summary>
              {detail.sourceExcerpt ? (
                <>
                  <blockquote>{detail.sourceExcerpt}</blockquote>
                  <button
                    className="button text-button"
                    onClick={() => {
                      void studio
                        .openConversation(
                          workspace!,
                          detail.sourceConversationId!,
                        )
                        .then(() => {
                          navigate("chat", {
                            workspace,
                            conversation: detail.sourceConversationId,
                            message: detail.node.sourceMessageId,
                            back: route.back,
                          });
                        })
                        .catch(studio.reportError);
                    }}
                  >
                    打开原会话
                  </button>
                  <p>
                    这里保留最初引用的回答文字。修改后的知识仍需要你检查并确认。
                  </p>
                  <code className="source-hash">
                    SHA-256 {detail.sourceHash}
                  </code>
                </>
              ) : (
                <p>
                  {detail.sourceEvidence?.length
                    ? "这条知识最初根据文件中的文字整理而来。修改后仍保留这份原始记录，内容是否准确需要你另行检查。"
                    : "由用户手动记录，没有引用模型回答。"}
                </p>
              )}
            </details>
            <AppDialog
              ref={approval}
              className="form-dialog approval-dialog"
              aria-labelledby="approval-title"
              onCancel={(e) => {
                if (approving) e.preventDefault();
              }}
            >
              <h2 id="approval-title">确认知识版本</h2>
              <p>
                将“{detail.versions[0].title}”的 v{detail.node.latestVersion}{" "}
                设为当前确认版本。回答查找资料时会遵循已填写的生效和失效日期。
              </p>
              <div className="approval-content">
                {detail.versions[0].content}
              </div>
              <div className="dialog-actions">
                <button
                  className="button secondary"
                  disabled={approving}
                  onClick={() => approval.current?.close()}
                >
                  返回检查
                </button>
                <button
                  className="button primary"
                  disabled={approving}
                  onClick={() => {
                    if (!workspace) return;
                    setApproving(true);
                    void knowledge("approve_knowledge", {
                      workspaceId: workspace,
                      nodeId: detail.node.id,
                      expectedRevision: detail.node.revision,
                    })
                      .then(() => {
                        approval.current?.close();
                        setRefresh((n) => n + 1);
                        notify("知识版本已确认");
                        onChanged?.();
                      })
                      .catch((e) => {
                        approval.current?.close();
                        setError(errorText(e));
                      })
                      .finally(() => setApproving(false));
                  }}
                >
                  {approving ? "正在确认…" : "确认此版本"}
                </button>
              </div>
            </AppDialog>
          </>
        ) : (
          !loadingDetail && (
            <div className="knowledge-empty detail-empty">
              <h2>选择一条知识</h2>
            </div>
          )
        )}
      </section>
    </>
  );
}
