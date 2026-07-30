import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

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

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
