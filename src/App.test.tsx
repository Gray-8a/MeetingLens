import { render, screen } from "@testing-library/react";
import App from "./App";

describe("MeetingLens application shell", () => {
  it("explains the local-first privacy boundary", () => {
    render(<App />);

    expect(
      screen.getByRole("heading", {
        name: /turn meetings into evidence-backed action/i,
      }),
    ).toBeInTheDocument();
    expect(screen.getByText(/raw meeting data stays local/i)).toBeInTheDocument();
    expect(screen.getByText(/credentials never enter/i)).toBeInTheDocument();
  });

  it("shows every planned product module", () => {
    render(<App />);

    const expectedModules = [
      "Audio capture",
      "Document context",
      "DeepSeek summary",
      "Live translation",
      "Speaker diarization",
      "Screen context",
    ];

    for (const moduleName of expectedModules) {
      expect(screen.getByRole("heading", { name: moduleName })).toBeInTheDocument();
    }
  });

  it("publishes the sidecar health contract", () => {
    render(<App />);

    expect(screen.getByText("GET /health")).toBeInTheDocument();
    expect(screen.getByText("127.0.0.1:8765")).toBeInTheDocument();
  });
});
