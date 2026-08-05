import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./i18n";

if (import.meta.env.DEV && import.meta.env.VITE_MEETING_COMMAND_SMOKE === "1") {
  void import("./dev/meetingCommandSmoke").then(({ runMeetingCommandSmoke }) =>
    runMeetingCommandSmoke(),
  );
}

if (import.meta.env.DEV && import.meta.env.VITE_MEETING_UI_SMOKE === "1") {
  void import("./dev/meetingUiSmoke").then(({ runMeetingUiSmoke }) =>
    runMeetingUiSmoke(),
  );
}

if (import.meta.env.DEV && import.meta.env.VITE_PROVIDER_UI_SMOKE === "1") {
  void import("./dev/providerUiSmoke").then(({ runProviderUiSmoke }) =>
    runProviderUiSmoke(),
  );
}

if (import.meta.env.DEV && import.meta.env.VITE_SIDECAR_AUTH_SMOKE === "1") {
  void import("./dev/sidecarAuthSmoke").then(({ runSidecarAuthSmoke }) =>
    runSidecarAuthSmoke(),
  );
}

if (import.meta.env.DEV && import.meta.env.VITE_SECRET_STORE_SMOKE === "1") {
  void import("./dev/secretStoreSmoke").then(({ runSecretStoreSmoke }) =>
    runSecretStoreSmoke(),
  );
}

if (import.meta.env.DEV && import.meta.env.VITE_DOCUMENT_UI_SMOKE === "1") {
  void import("./dev/documentUiSmoke").then(({ runDocumentUiSmoke }) =>
    runDocumentUiSmoke(),
  );
}

if (import.meta.env.DEV && import.meta.env.VITE_I18N_UI_SMOKE === "1") {
  void import("./dev/i18nUiSmoke").then(({ runI18nUiSmoke }) =>
    runI18nUiSmoke(),
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
