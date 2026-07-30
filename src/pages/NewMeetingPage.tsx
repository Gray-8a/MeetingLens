import { useState, type FormEvent } from "react";
import { AppLink, meetingPath, useNavigation } from "../app/navigation";
import { useServices } from "../app/services";
import { ErrorNotice } from "../components/UiState";
import { uiError } from "../lib/presentation";

export function NewMeetingPage() {
  const { meetingApi } = useServices();
  const { navigate } = useNavigation();
  const [title, setTitle] = useState("");
  const [validationMessage, setValidationMessage] = useState<string | null>(
    null,
  );
  const [error, setError] = useState<ReturnType<typeof uiError> | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submitting) return;

    const normalizedTitle = title.trim();
    if (normalizedTitle.length === 0) {
      setValidationMessage("Enter a meeting title.");
      return;
    }

    setValidationMessage(null);
    setError(null);
    setSubmitting(true);
    try {
      const meeting = await meetingApi.create({
        title: normalizedTitle,
        primaryLanguage: "en-AU",
      });
      navigate(meetingPath(meeting.id));
    } catch (reason: unknown) {
      setError(uiError(reason));
      setSubmitting(false);
    }
  }

  return (
    <section className="form-panel" aria-labelledby="new-meeting-title">
      <div className="section-heading section-heading--stacked">
        <p className="eyebrow">Create a local draft</p>
        <h1 id="new-meeting-title">New meeting</h1>
        <p>
          This creates metadata only. No microphone, screen, file, or external
          AI access is enabled.
        </p>
      </div>

      {error ? <ErrorNotice code={error.code} message={error.message} /> : null}

      <form onSubmit={handleSubmit}>
        <div className="field-group">
          <label htmlFor="meeting-title">Meeting title</label>
          <input
            data-acceptance="new-title"
            aria-describedby="meeting-title-help"
            aria-invalid={validationMessage !== null}
            autoComplete="off"
            disabled={submitting}
            id="meeting-title"
            maxLength={200}
            onChange={(event) => {
              setTitle(event.target.value);
              if (validationMessage) setValidationMessage(null);
            }}
            placeholder="e.g. Synthetic project review"
            value={title}
          />
          <p
            className={validationMessage ? "field-error" : "field-help"}
            id="meeting-title-help"
          >
            {validationMessage ?? "Use a short title you can recognize later."}
          </p>
        </div>
        <div className="form-actions">
          <button
            className="button button--primary"
            data-acceptance="create-meeting"
            disabled={submitting}
            type="submit"
          >
            {submitting ? "Creating…" : "Create meeting"}
          </button>
          <AppLink
            aria-disabled={submitting}
            className="button button--ghost"
            to="/"
          >
            Cancel
          </AppLink>
        </div>
      </form>
    </section>
  );
}
