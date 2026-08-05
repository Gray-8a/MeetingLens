import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { AppLink, meetingPath, useNavigation } from "../app/navigation";
import { useServices } from "../app/services";
import { ErrorNotice } from "../components/UiState";
import { meetingErrorMessage, uiError } from "../lib/presentation";

export function NewMeetingPage() {
  const { t } = useTranslation();
  const { meetingApi } = useServices();
  const { navigate } = useNavigation();
  const [title, setTitle] = useState("");
  const [titleMissing, setTitleMissing] = useState(false);
  const [error, setError] = useState<ReturnType<typeof uiError> | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submitting) return;

    const normalizedTitle = title.trim();
    if (normalizedTitle.length === 0) {
      setTitleMissing(true);
      return;
    }

    setTitleMissing(false);
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
        <p className="eyebrow">{t("newMeeting.eyebrow")}</p>
        <h1 id="new-meeting-title">{t("newMeeting.title")}</h1>
        <p>{t("newMeeting.description")}</p>
      </div>

      {error ? (
        <ErrorNotice
          code={error.code}
          message={meetingErrorMessage(error.code)}
        />
      ) : null}

      <form onSubmit={handleSubmit}>
        <div className="field-group">
          <label htmlFor="meeting-title">{t("newMeeting.meetingTitle")}</label>
          <input
            data-acceptance="new-title"
            aria-describedby="meeting-title-help"
            aria-invalid={titleMissing}
            autoComplete="off"
            disabled={submitting}
            id="meeting-title"
            maxLength={200}
            onChange={(event) => {
              setTitle(event.target.value);
              if (titleMissing) setTitleMissing(false);
            }}
            placeholder={t("newMeeting.placeholder")}
            value={title}
          />
          <p
            className={titleMissing ? "field-error" : "field-help"}
            id="meeting-title-help"
          >
            {titleMissing
              ? t("newMeeting.titleRequired")
              : t("newMeeting.titleHelp")}
          </p>
        </div>
        <div className="form-actions">
          <button
            className="button button--primary"
            data-acceptance="create-meeting"
            disabled={submitting}
            type="submit"
          >
            {submitting ? t("common.creating") : t("newMeeting.create")}
          </button>
          <AppLink
            aria-disabled={submitting}
            className="button button--ghost"
            to="/"
          >
            {t("common.cancel")}
          </AppLink>
        </div>
      </form>
    </section>
  );
}
