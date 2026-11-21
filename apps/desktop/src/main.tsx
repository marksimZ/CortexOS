import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { restorePortableUi } from "./portable-ui";

void restorePortableUi().then(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement, { onCaughtError: () => {} }).render(
    <React.StrictMode>
      <ErrorBoundary><App /></ErrorBoundary>
    </React.StrictMode>,
  );
});
