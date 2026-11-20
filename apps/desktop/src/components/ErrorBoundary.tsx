import { Component, Fragment, type ReactNode } from "react";

export class ErrorBoundary extends Component<{ children: ReactNode; scope?: "page" }, { failed: boolean; attempt: number }> {
  state = { failed: false, attempt: 0 };

  static getDerivedStateFromError() { return { failed: true }; }

  render() {
    if (this.state.failed) {
      return <section className="render-error" aria-label="界面恢复">
        <h2 role="alert">{this.props.scope === "page" ? "当前页面无法显示" : "CortexOS 界面无法显示"}</h2>
        <p>重新打开界面前，可先检查正在编辑的内容是否已写入本地草稿。</p>
        <div className="dialog-actions">
          <button className="button primary" autoFocus onClick={() => this.setState(({ attempt }) => ({ failed: false, attempt: attempt + 1 }))}>重试显示</button>
          {!this.props.scope && <button className="button secondary" onClick={() => window.location.reload()}>重新加载应用</button>}
        </div>
      </section>;
    }
    return <Fragment key={this.state.attempt}>{this.props.children}</Fragment>;
  }
}
