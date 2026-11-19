import { Component, Suspense, lazy, useState, type ReactNode } from "react";

class ViewBoundary extends Component<{
  retry: () => void;
  children: ReactNode;
}, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    if (this.state.failed) {
      return (
        <div className="error-banner" role="alert">
          <span>这部分内容暂时无法打开。</span>
          <button className="button text-button" onClick={this.props.retry}>重试打开</button>
        </div>
      );
    }
    return this.props.children;
  }
}

export function lazyView<Module, Props extends object>(
  load: () => Promise<Module>,
  select: (module: Module) => (props: Props) => ReactNode,
) {
  const defer = () => lazy(async () => ({ default: select(await load()) }));
  let View = defer();
  return function DeferredView(props: Props) {
    const [attempt, setAttempt] = useState(0);
    return (
      <ViewBoundary key={attempt} retry={() => {
        View = defer();
        setAttempt((value) => value + 1);
      }}>
        <Suspense fallback={<p className="inline-loading" role="status">正在打开…</p>}>
          <View {...props} />
        </Suspense>
      </ViewBoundary>
    );
  };
}
