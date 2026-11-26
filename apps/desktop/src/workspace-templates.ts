export type WorkspaceTemplateId = "blank" | "study" | "work" | "writing" | "life";

export interface WorkspaceTemplate {
  id: WorkspaceTemplateId;
  label: string;
  tagline: string;
  placeholder: string;
  assistantLabel: string | null;
}

export const WORKSPACE_TEMPLATES: WorkspaceTemplate[] = [
  {
    id: "study",
    label: "学习备考",
    tagline: "PDF/PPT 课件梳理 · 费曼讲解 · 考前自测",
    placeholder: "例如：操作系统期末 / 考研政治",
    assistantLabel: "学习备考教练",
  },
  {
    id: "work",
    label: "职场项目",
    tagline: "Excel/方案提炼 · 结论先行 · 风险与周报",
    placeholder: "例如：Q4 产品发布 / 客户方案",
    assistantLabel: "职场项目搭档",
  },
  {
    id: "writing",
    label: "阅读写作",
    tagline: "EPUB 电子书拆解 · 素材串联 · 保留文风",
    placeholder: "例如：读书笔记与专栏写作",
    assistantLabel: "阅读写作编辑",
  },
  {
    id: "life",
    label: "生活管家",
    tagline: "体检/保单大白话解读 · 本地隐私 · 提醒",
    placeholder: "例如：个人与家庭事务",
    assistantLabel: "私人生活管家",
  },
  {
    id: "blank",
    label: "空白空间",
    tagline: "不预置场景分身，从零自由定制",
    placeholder: "例如：产品设计",
    assistantLabel: null,
  },
];
