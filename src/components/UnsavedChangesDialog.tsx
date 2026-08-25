import { useEffect, useRef, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

export function UnsavedChangesDialog({
  onCancel,
  onDiscard,
}: {
  onCancel: () => void;
  onDiscard: () => void;
}) {
  const { t } = useTranslation();
  const dialogRef = useRef<HTMLDivElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const cancelRef = useRef(onCancel);

  useEffect(() => {
    cancelRef.current = onCancel;
  }, [onCancel]);

  useEffect(() => {
    const previousFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    headingRef.current?.focus();
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        cancelRef.current();
      } else if (
        event.key === "Tab" &&
        dialogRef.current !== null &&
        !dialogRef.current.contains(document.activeElement)
      ) {
        event.preventDefault();
        headingRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      previousFocus?.focus();
    };
  }, []);

  function trapFocus(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== "Tab" || dialogRef.current === null) return;
    const buttons = [
      ...dialogRef.current.querySelectorAll<HTMLButtonElement>(
        "button:not(:disabled)",
      ),
    ];
    const first = buttons[0];
    const last = buttons[buttons.length - 1];
    const active =
      document.activeElement instanceof HTMLButtonElement
        ? document.activeElement
        : null;
    if (
      event.shiftKey &&
      (active === first || active === null || !buttons.includes(active))
    ) {
      event.preventDefault();
      last?.focus();
    } else if (
      !event.shiftKey &&
      (active === last || active === null || !buttons.includes(active))
    ) {
      event.preventDefault();
      first?.focus();
    }
  }

  return (
    <div className="consent-dialog-backdrop">
      <div
        ref={dialogRef}
        aria-describedby="unsaved-minutes-description"
        aria-labelledby="unsaved-minutes-title"
        aria-modal="true"
        className="consent-dialog"
        onKeyDown={trapFocus}
        role="alertdialog"
      >
        <h2 ref={headingRef} id="unsaved-minutes-title" tabIndex={-1}>
          {t("summaryMinutes.unsaved.title")}
        </h2>
        <p id="unsaved-minutes-description">
          {t("summaryMinutes.unsaved.body")}
        </p>
        <div className="form-actions">
          <button
            className="button button--danger"
            onClick={onDiscard}
            type="button"
          >
            {t("summaryMinutes.unsaved.discard")}
          </button>
          <button
            className="button button--secondary"
            onClick={onCancel}
            type="button"
          >
            {t("summaryMinutes.unsaved.keepEditing")}
          </button>
        </div>
      </div>
    </div>
  );
}
