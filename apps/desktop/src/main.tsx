import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { QuickBar } from "./QuickBar";
import { restorePortableUi } from "./portable-ui";
import { installSlidingPills } from "./sliding-pill";

const isQuickBarWindow = window.location.hash.startsWith("#/quickbar");
if (!isQuickBarWindow) installSlidingPills();

void restorePortableUi().then(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement, { onCaughtError: () => {} }).render(
    <React.StrictMode>
      <ErrorBoundary>{isQuickBarWindow ? <QuickBar /> : <App />}</ErrorBoundary>
    </React.StrictMode>,
  );
});
