import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import dialogSource from "./DeepSeekConsentDialog.tsx?raw";
import i18n from "../i18n";
import { syntheticExternalDisclosure } from "../test/fakeSummaryApi";
import { DeepSeekConsentDialog } from "./DeepSeekConsentDialog";

beforeEach(async () => {
  await i18n.changeLanguage("en-US");
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("DeepSeekConsentDialog", () => {
  it("shows only the approved disclosure metadata and data categories", () => {
    render(
      <DeepSeekConsentDialog
        disclosure={syntheticExternalDisclosure()}
        onCancel={() => undefined}
        onConfirm={async () => undefined}
      />,
    );

    const dialog = screen.getByRole("dialog", {
      name: "Send this frozen transcript to DeepSeek?",
    });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(screen.getByText("api.deepseek.com")).toBeInTheDocument();
    expect(screen.getByText("deepseek-v4-pro")).toBeInTheDocument();
    expect(screen.getByText("Frozen transcript text")).toBeInTheDocument();
    expect(
      screen.getByText("Speaker labels and speaker names"),
    ).toBeInTheDocument();
    expect(dialog).toHaveTextContent("MeetingLens cannot recall");
    expect(dialog).not.toHaveTextContent("Synthetic transcript body");
    expect(dialog).not.toHaveTextContent("Authorization");
  });

  it("submits a one-time choice or a remembered choice explicitly", async () => {
    const user = userEvent.setup();
    const confirmOnce = vi.fn(async () => undefined);
    const first = render(
      <DeepSeekConsentDialog
        disclosure={syntheticExternalDisclosure()}
        onCancel={() => undefined}
        onConfirm={confirmOnce}
      />,
    );
    await user.click(
      screen.getByRole("button", { name: "Consent this time and continue" }),
    );
    expect(confirmOnce).toHaveBeenCalledWith(false);
    first.unmount();

    const confirmRemembered = vi.fn(async () => undefined);
    render(
      <DeepSeekConsentDialog
        disclosure={syntheticExternalDisclosure()}
        onCancel={() => undefined}
        onConfirm={confirmRemembered}
      />,
    );
    await user.click(
      screen.getByRole("checkbox", { name: /Remember this choice/ }),
    );
    await user.click(
      screen.getByRole("button", { name: "Remember choice and continue" }),
    );
    expect(confirmRemembered).toHaveBeenCalledWith(true);
  });

  it("treats Escape, close, backdrop and Cancel as cancellation", async () => {
    const user = userEvent.setup();
    const cancel = vi.fn();
    const view = render(
      <DeepSeekConsentDialog
        disclosure={syntheticExternalDisclosure()}
        onCancel={cancel}
        onConfirm={async () => undefined}
      />,
    );

    fireEvent.keyDown(document, { key: "Escape" });
    await user.click(
      screen.getByRole("button", { name: "Close without sending" }),
    );
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(view.container.querySelector(".consent-dialog-backdrop")!);
    expect(cancel).toHaveBeenCalledTimes(4);
  });

  it("blocks double submission while confirmation is pending", async () => {
    const pending = deferred<void>();
    const confirm = vi.fn(() => pending.promise);
    render(
      <DeepSeekConsentDialog
        disclosure={syntheticExternalDisclosure()}
        onCancel={() => undefined}
        onConfirm={confirm}
      />,
    );

    const button = screen.getByRole("button", {
      name: "Consent this time and continue",
    });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(confirm).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Confirming…" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();

    await act(async () => pending.resolve());
  });

  it("keeps focus inside the modal while every action is disabled", async () => {
    const pending = deferred<void>();
    render(
      <>
        <button type="button">{i18n.t("app.nav.help")}</button>
        <DeepSeekConsentDialog
          disclosure={syntheticExternalDisclosure()}
          onCancel={() => undefined}
          onConfirm={() => pending.promise}
        />
      </>,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Consent this time and continue" }),
    );
    const heading = screen.getByRole("heading", {
      name: "Send this frozen transcript to DeepSeek?",
    });
    expect(heading).toHaveFocus();

    fireEvent.keyDown(heading, { key: "Tab" });
    expect(heading).toHaveFocus();

    screen.getByRole("button", { name: "Help" }).focus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(heading).toHaveFocus();

    await act(async () => pending.resolve());
  });

  it("allows a safe retry after confirmation rejects", async () => {
    const user = userEvent.setup();
    const confirm = vi
      .fn<(remember: boolean) => Promise<void>>()
      .mockRejectedValueOnce(new Error("private backend wording"))
      .mockResolvedValueOnce(undefined);
    render(
      <DeepSeekConsentDialog
        disclosure={syntheticExternalDisclosure()}
        errorCode="deepseek_network_failed"
        onCancel={() => undefined}
        onConfirm={confirm}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "MeetingLens could not reach DeepSeek.",
    );
    expect(document.body).not.toHaveTextContent("private backend wording");
    await user.click(
      screen.getByRole("button", { name: "Consent this time and continue" }),
    );
    expect(
      screen.getByRole("button", { name: "Consent this time and continue" }),
    ).toBeEnabled();
    await user.click(
      screen.getByRole("button", { name: "Consent this time and continue" }),
    );
    expect(confirm).toHaveBeenCalledTimes(2);
  });

  it("moves focus into the dialog and restores it after cancellation", async () => {
    const user = userEvent.setup();
    render(<FocusHarness />);
    const trigger = screen.getByRole("button", {
      name: "External transcript sharing",
    });
    trigger.focus();
    await user.click(trigger);
    expect(
      screen.getByRole("heading", {
        name: "Send this frozen transcript to DeepSeek?",
      }),
    ).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(trigger).toHaveFocus();
  });

  it("does not reset focus when a polling parent supplies a new callback", () => {
    const view = render(
      <DeepSeekConsentDialog
        disclosure={syntheticExternalDisclosure()}
        onCancel={() => undefined}
        onConfirm={async () => undefined}
      />,
    );
    const remember = screen.getByRole("checkbox", {
      name: /Remember this choice/,
    });
    remember.focus();

    view.rerender(
      <DeepSeekConsentDialog
        disclosure={syntheticExternalDisclosure()}
        onCancel={() => undefined}
        onConfirm={async () => undefined}
      />,
    );

    expect(remember).toHaveFocus();
  });

  it("keeps reverse tab navigation inside the dialog from initial heading focus", () => {
    render(
      <DeepSeekConsentDialog
        disclosure={syntheticExternalDisclosure()}
        onCancel={() => undefined}
        onConfirm={async () => undefined}
      />,
    );
    const heading = screen.getByRole("heading", {
      name: "Send this frozen transcript to DeepSeek?",
    });
    expect(heading).toHaveFocus();
    fireEvent.keyDown(heading, { key: "Tab", shiftKey: true });

    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
  });

  it("renders equivalent Simplified Chinese controls", async () => {
    await act(async () => i18n.changeLanguage("zh-CN"));
    render(
      <DeepSeekConsentDialog
        disclosure={syntheticExternalDisclosure()}
        onCancel={() => undefined}
        onConfirm={async () => undefined}
      />,
    );
    expect(
      screen.getByRole("heading", {
        name: "将这份冻结转录发送给 DeepSeek？",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "本次同意并继续" }),
    ).toBeEnabled();
    expect(screen.getByRole("button", { name: "取消" })).toBeEnabled();
  });

  it("contains no direct transport, browser storage or logging surface", () => {
    expect(dialogSource).not.toMatch(
      /@tauri-apps|\binvoke\s*\(|\bfetch\s*\(|XMLHttpRequest|WebSocket/,
    );
    expect(dialogSource).not.toMatch(
      /localStorage|sessionStorage|indexedDB|console\./,
    );
  });
});

function FocusHarness() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button onClick={() => setOpen(true)} type="button">
        {i18n.t("settings.deepseek.externalSharing")}
      </button>
      {open ? (
        <DeepSeekConsentDialog
          disclosure={syntheticExternalDisclosure()}
          onCancel={() => setOpen(false)}
          onConfirm={async () => undefined}
        />
      ) : null}
    </>
  );
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((promiseResolve) => {
    resolve = promiseResolve;
  });
  return { promise, resolve };
}
