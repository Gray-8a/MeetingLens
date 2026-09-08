import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useServices } from "../app/services";
import {
  toSummaryError,
  type SummaryContent,
  type SummaryActionStatus,
  type SummaryDecisionStatus,
  type SummaryEditableContent,
  type SummaryErrorCode,
  type SummaryEvidenceResponse,
  type SummaryRunResponse,
} from "../api/summary";
import i18n from "../i18n";
import { summaryErrorMessage } from "../i18n/errors";
import { formatLocalDate, formatNumber } from "../i18n/formatters";
import { ErrorNotice, InfoNotice } from "./UiState";

export interface SummaryResultViewProps {
  run: SummaryRunResponse;
  isLatestRevision: boolean;
  latestRevisionKnown: boolean;
  actionBusy: boolean;
  onCancel: (run: SummaryRunResponse) => Promise<void>;
  onContinue: (run: SummaryRunResponse) => Promise<void>;
  onOpenLatest: (seriesId: string) => void;
  onRegenerate: (run: SummaryRunResponse) => Promise<void>;
  onRunUpdated: (run: SummaryRunResponse) => void;
  onDirtyChange: (dirty: boolean) => void;
}

type Feedback =
  | "copied"
  | "saved"
  | "save-cancelled"
  | "clipboard-unavailable"
  | "clipboard-failed"
  | null;

type LocalActionResult<T> =
  | { completed: true; value: T }
  | { completed: false };

export function SummaryResultView({
  run,
  isLatestRevision,
  latestRevisionKnown,
  actionBusy,
  onCancel,
  onContinue,
  onOpenLatest,
  onRegenerate,
  onRunUpdated,
  onDirtyChange,
}: SummaryResultViewProps) {
  const { t } = useTranslation();
  const { summaryApi } = useServices();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<SummaryEditableContent | null>(() =>
    run.content ? editableFromContent(run.content) : null,
  );
  const [markdownCache, setMarkdownCache] = useState<{
    key: string;
    value: string;
  } | null>(null);
  const [evidenceCache, setEvidenceCache] = useState<{
    key: string;
    value: SummaryEvidenceResponse;
  } | null>(null);
  const [error, setError] = useState<SummaryErrorCode | null>(null);
  const [busy, setBusy] = useState<
    "copy" | "evidence" | "save" | "update" | null
  >(null);
  const [feedback, setFeedback] = useState<Feedback>(null);
  const localBusyRef = useRef(false);
  const operationGenerationRef = useRef(0);
  const cacheKey = [
    run.id,
    run.contentVersion ?? "none",
    run.updatedAt,
    String(run.isStale),
    run.staleReason ?? "none",
    run.staleAt ?? "none",
  ].join(":");
  const markdown = markdownCache?.key === cacheKey ? markdownCache.value : null;
  const evidence = evidenceCache?.key === cacheKey ? evidenceCache.value : null;

  useEffect(
    () => () => {
      operationGenerationRef.current += 1;
    },
    [],
  );

  const persistedEditable = useMemo(
    () => (run.content ? editableFromContent(run.content) : null),
    [run.content],
  );
  const dirty =
    draft !== null &&
    persistedEditable !== null &&
    JSON.stringify(draft) !== JSON.stringify(persistedEditable);

  useEffect(() => {
    onDirtyChange(dirty);
    return () => onDirtyChange(false);
  }, [dirty, onDirtyChange]);
  const canEdit =
    run.status === "completed" &&
    run.provider === "deepseek" &&
    isLatestRevision &&
    run.content !== null &&
    run.contentVersion !== null;
  const canRegenerate =
    isLatestRevision &&
    run.provider === "deepseek" &&
    (run.status === "completed" ||
      run.status === "failed" ||
      run.status === "cancelled") &&
    run.staleReason !== "source_deleted";

  async function runLocalAction<T>(
    kind: Exclude<typeof busy, null>,
    action: (isCurrent: () => boolean) => Promise<T>,
  ): Promise<LocalActionResult<T>> {
    if (localBusyRef.current) return { completed: false };
    const generation = ++operationGenerationRef.current;
    const isCurrent = () => operationGenerationRef.current === generation;
    localBusyRef.current = true;
    setBusy(kind);
    setError(null);
    setFeedback(null);
    try {
      const value = await action(isCurrent);
      return isCurrent() ? { completed: true, value } : { completed: false };
    } catch (reason: unknown) {
      if (isCurrent()) setError(toSummaryError(reason).code);
      return { completed: false };
    } finally {
      if (isCurrent()) {
        localBusyRef.current = false;
        setBusy(null);
      }
    }
  }

  async function saveEdit() {
    if (!draft || !run.contentVersion || !dirty) return;
    const result = await runLocalAction("update", () =>
      summaryApi.updateContent({
        meetingId: run.meetingId,
        summaryId: run.id,
        expectedContentVersion: run.contentVersion as string,
        editableContent: normalizeEditableContent(draft),
      }),
    );
    if (result.completed) {
      setDraft(
        result.value.content ? editableFromContent(result.value.content) : null,
      );
      setEditing(false);
      setMarkdownCache(null);
      onRunUpdated(result.value);
    }
  }

  async function loadEvidence(evidenceId: string) {
    const result = await runLocalAction("evidence", () =>
      summaryApi.getEvidence({
        meetingId: run.meetingId,
        summaryId: run.id,
        evidenceId,
      }),
    );
    if (result.completed) {
      setEvidenceCache({
        key: cacheKey,
        value: result.value,
      });
    }
  }

  async function requestMarkdown() {
    if (!run.contentVersion || dirty) {
      throw new Error("summary_markdown_not_current");
    }
    return summaryApi.getMarkdown({
      meetingId: run.meetingId,
      summaryId: run.id,
      expectedContentVersion: run.contentVersion,
    });
  }

  async function previewMarkdown() {
    const result = await runLocalAction("copy", requestMarkdown);
    if (result.completed) {
      setMarkdownCache({ key: cacheKey, value: result.value.markdown });
    }
  }

  async function copyMarkdown() {
    if (!navigator.clipboard?.writeText) {
      setFeedback("clipboard-unavailable");
      return;
    }
    const result = await runLocalAction("copy", async (isCurrent) => {
      const markdownResult = await requestMarkdown();
      if (!isCurrent())
        return { markdown: markdownResult.markdown, copied: false };
      try {
        await navigator.clipboard.writeText(markdownResult.markdown);
        return { markdown: markdownResult.markdown, copied: true };
      } catch {
        return { markdown: markdownResult.markdown, copied: false };
      }
    });
    if (result.completed) {
      setMarkdownCache({ key: cacheKey, value: result.value.markdown });
      setFeedback(result.value.copied ? "copied" : "clipboard-failed");
    }
  }

  async function saveMarkdown() {
    if (!run.contentVersion || dirty) return;
    const result = await runLocalAction("save", () =>
      summaryApi.saveMarkdown({
        meetingId: run.meetingId,
        summaryId: run.id,
        expectedContentVersion: run.contentVersion as string,
      }),
    );
    if (result.completed) {
      setFeedback(result.value.cancelled ? "save-cancelled" : "saved");
    }
  }

  return (
    <section
      className="content-panel summary-result"
      data-acceptance="summary-result"
    >
      <header className="section-heading">
        <div>
          <p className="eyebrow">{t("summaryMinutes.result.eyebrow")}</p>
          <h2>
            {t("summaryMinutes.result.revision", { revision: run.revision })}
          </h2>
        </div>
        <span className={`summary-status summary-status--${run.status}`}>
          {t(`summaryMinutes.status.${run.status}`)}
        </span>
      </header>

      <dl className="summary-run-meta">
        <div>
          <dt>{t("summaryMinutes.result.template")}</dt>
          <dd>{t(`summaryConsent.templateValue.${run.template}`)}</dd>
        </div>
        <div>
          <dt>{t("summaryMinutes.result.language")}</dt>
          <dd>
            {t(`summaryConsent.outputLanguageValue.${run.outputLanguage}`)}
          </dd>
        </div>
        <div>
          <dt>{t("summaryMinutes.result.updated")}</dt>
          <dd>{formatLocalDate(run.updatedAt)}</dd>
        </div>
      </dl>

      {!isLatestRevision ? (
        <InfoNotice>
          <p>
            {latestRevisionKnown
              ? t("summaryMinutes.result.oldRevision")
              : t("summaryMinutes.result.latestUnknown")}
          </p>
          <button
            className="button button--secondary"
            onClick={() => onOpenLatest(run.seriesId)}
            type="button"
          >
            {latestRevisionKnown
              ? t("summaryMinutes.result.openLatest")
              : t("summaryMinutes.result.findLatest")}
          </button>
        </InfoNotice>
      ) : null}

      {run.isStale ? (
        <InfoNotice>
          <p>
            {t(`summaryMinutes.stale.${run.staleReason ?? "segment_revised"}`)}
          </p>
        </InfoNotice>
      ) : null}

      {run.provider === "deterministic_mock" ? (
        <InfoNotice>
          <p>{t("summaryMinutes.result.mockReadOnly")}</p>
        </InfoNotice>
      ) : null}

      {error ? (
        <ErrorNotice code={error} message={summaryErrorMessage(error)} />
      ) : null}

      {run.status === "queued" ? (
        <div className="summary-state-actions">
          <p>{t("summaryMinutes.result.queuedManual")}</p>
          {run.provider === "deepseek" ? (
            <button
              className="button button--primary"
              disabled={actionBusy}
              onClick={() => void onContinue(run)}
              type="button"
            >
              {t("summaryMinutes.actions.continue")}
            </button>
          ) : null}
          <button
            className="button button--secondary"
            disabled={actionBusy}
            onClick={() => void onCancel(run)}
            type="button"
          >
            {t("summaryMinutes.actions.cancelRun")}
          </button>
        </div>
      ) : null}

      {run.status === "running" ? (
        <div className="summary-state-actions" role="status">
          <span className="spinner" aria-hidden="true" />
          <p>{t("summaryMinutes.result.runningBody")}</p>
          <button
            className="button button--secondary"
            disabled={actionBusy}
            onClick={() => void onCancel(run)}
            type="button"
          >
            {t("summaryMinutes.actions.cancelRun")}
          </button>
        </div>
      ) : null}

      {run.status === "failed" ? (
        <ErrorNotice
          code={run.errorCode ?? "summary_provider_failed"}
          message={
            run.errorCode === "summary_interrupted"
              ? t("summaryMinutes.result.interrupted")
              : summaryErrorMessage(
                  (run.errorCode ??
                    "summary_provider_failed") as SummaryErrorCode,
                )
          }
        />
      ) : null}

      {run.status === "cancelled" ? (
        <p>{t("summaryMinutes.result.cancelledBody")}</p>
      ) : null}

      {run.status === "completed" && run.content ? (
        <>
          {editing && draft ? (
            <SummaryEditor
              draft={draft}
              disabled={busy !== null}
              onChange={setDraft}
            />
          ) : (
            <SummaryContentView
              content={run.content}
              onEvidence={loadEvidence}
            />
          )}

          <div className="form-actions summary-result__actions">
            {canEdit ? (
              editing ? (
                <>
                  <button
                    className="button button--primary"
                    disabled={!dirty || busy !== null}
                    onClick={() => void saveEdit()}
                    type="button"
                  >
                    {busy === "update"
                      ? t("common.saving")
                      : t("summaryMinutes.actions.saveEdit")}
                  </button>
                  <button
                    className="button button--ghost"
                    disabled={busy !== null}
                    onClick={() => {
                      setDraft(persistedEditable);
                      setEditing(false);
                    }}
                    type="button"
                  >
                    {t("common.cancel")}
                  </button>
                </>
              ) : (
                <button
                  className="button button--secondary"
                  onClick={() => setEditing(true)}
                  type="button"
                >
                  {t("summaryMinutes.actions.edit")}
                </button>
              )
            ) : null}
            <button
              className="button button--secondary"
              disabled={dirty || busy !== null}
              onClick={() => void previewMarkdown()}
              type="button"
            >
              {t("summaryMinutes.actions.previewMarkdown")}
            </button>
            <button
              className="button button--secondary"
              disabled={dirty || busy !== null}
              onClick={() => void copyMarkdown()}
              type="button"
            >
              {t("summaryMinutes.actions.copyMarkdown")}
            </button>
            <button
              className="button button--secondary"
              disabled={dirty || busy !== null}
              onClick={() => void saveMarkdown()}
              type="button"
            >
              {t("summaryMinutes.actions.saveMarkdown")}
            </button>
            {canRegenerate ? (
              <button
                className="button button--primary"
                disabled={actionBusy || dirty || busy !== null}
                onClick={() => void onRegenerate(run)}
                type="button"
              >
                {t("summaryMinutes.actions.regenerate")}
              </button>
            ) : null}
          </div>

          {dirty ? (
            <p className="restriction">
              {t("summaryMinutes.edit.dirtyExport")}
            </p>
          ) : null}
          {run.isUserEdited ? (
            <p className="supporting-copy">
              {t("summaryMinutes.edit.editedNotice")}
            </p>
          ) : null}
          <p className="supporting-copy">
            {t("summaryMinutes.result.evidenceLimitation")}
          </p>
          <p className="supporting-copy">
            {t("summaryMinutes.result.exportWarning")}
          </p>

          <p className="sr-only" aria-live="polite">
            {feedback ? t(`summaryMinutes.feedback.${feedback}`) : ""}
          </p>
          {feedback ? (
            <p className="operation-result">
              {t(`summaryMinutes.feedback.${feedback}`)}
            </p>
          ) : null}

          {markdown !== null ? (
            <section className="summary-markdown-preview">
              <div className="section-heading">
                <h3>{t("summaryMinutes.markdown.title")}</h3>
                <button
                  className="text-link"
                  onClick={() => setMarkdownCache(null)}
                  type="button"
                >
                  {t("summaryMinutes.actions.close")}
                </button>
              </div>
              <pre>{markdown}</pre>
            </section>
          ) : null}

          {evidence ? (
            <aside
              aria-labelledby="summary-evidence-title"
              className="summary-evidence-card"
            >
              <div className="section-heading">
                <h3 id="summary-evidence-title">{evidence.label}</h3>
                <button
                  className="text-link"
                  onClick={() => setEvidenceCache(null)}
                  type="button"
                >
                  {t("summaryMinutes.actions.close")}
                </button>
              </div>
              <p>{evidence.excerpt}</p>
              <dl className="summary-run-meta">
                {evidence.kind === "transcript" ? (
                  <>
                    <div>
                      <dt>{t("summaryMinutes.evidence.kind")}</dt>
                      <dd>{t("summaryMinutes.evidence.transcript")}</dd>
                    </div>
                    <div>
                      <dt>{t("summaryMinutes.evidence.speaker")}</dt>
                      <dd>
                        {evidence.speakerSnapshot ?? t("common.notAvailable")}
                      </dd>
                    </div>
                  </>
                ) : (
                  <>
                    <div>
                      <dt>{t("summaryMinutes.evidence.kind")}</dt>
                      <dd>{t("summaryMinutes.evidence.document")}</dd>
                    </div>
                    <div>
                      <dt>{t("summaryMinutes.evidence.pages")}</dt>
                      <dd>
                        {formatDocumentPages(
                          evidence.pageStart,
                          evidence.pageEnd,
                        )}
                      </dd>
                    </div>
                    <div>
                      <dt>{t("summaryMinutes.evidence.section")}</dt>
                      <dd>
                        {evidence.sectionPath.length === 0
                          ? t("common.notAvailable")
                          : evidence.sectionPath.join(" › ")}
                      </dd>
                    </div>
                    <div>
                      <dt>{t("summaryMinutes.evidence.table")}</dt>
                      <dd>
                        {evidence.tableOrdinal === null
                          ? t("common.notAvailable")
                          : formatNumber(evidence.tableOrdinal + 1)}
                      </dd>
                    </div>
                    <div>
                      <dt>{t("summaryMinutes.evidence.parserVersion")}</dt>
                      <dd>{evidence.parserVersion}</dd>
                    </div>
                  </>
                )}
                <div>
                  <dt>{t("summaryMinutes.evidence.ordinal")}</dt>
                  <dd>{formatNumber(evidence.ordinal + 1)}</dd>
                </div>
              </dl>
              {evidence.sourceDeleted ? (
                <p className="restriction">
                  {evidence.kind === "transcript"
                    ? t("summaryMinutes.evidence.sourceDeleted")
                    : t("summaryMinutes.evidence.documentDeleted")}
                </p>
              ) : null}
            </aside>
          ) : null}
        </>
      ) : null}

      {canRegenerate && run.status !== "completed" ? (
        <button
          className="button button--primary"
          disabled={actionBusy}
          onClick={() => void onRegenerate(run)}
          type="button"
        >
          {t("summaryMinutes.actions.regenerate")}
        </button>
      ) : null}
    </section>
  );
}

function formatDocumentPages(
  pageStart: number | null,
  pageEnd: number | null,
): string {
  if (pageStart === null) return i18n.t("common.notAvailable");
  if (pageEnd === null || pageEnd === pageStart) return formatNumber(pageStart);
  return i18n.t("summaryMinutes.evidence.pageRange", {
    start: formatNumber(pageStart),
    end: formatNumber(pageEnd),
  });
}

function SummaryContentView({
  content,
  onEvidence,
}: {
  content: SummaryContent;
  onEvidence: (id: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  const evidenceLabels = useMemo(() => evidenceLabelMap(content), [content]);
  return (
    <div className="summary-content">
      <SummarySection title={t("summaryMinutes.sections.overview")}>
        <p>{content.overview.text}</p>
        <EvidenceLinks
          ids={content.overview.evidenceIds}
          labels={evidenceLabels}
          onOpen={onEvidence}
        />
      </SummarySection>
      <SummaryListSection
        items={content.decisions}
        labels={evidenceLabels}
        onEvidence={onEvidence}
        title={t("summaryMinutes.sections.decisions")}
      />
      <section>
        <h3>{t("summaryMinutes.sections.actionItems")}</h3>
        {content.actionItems.length === 0 ? (
          <p>{t("summaryMinutes.result.none")}</p>
        ) : (
          <ol>
            {content.actionItems.map((item, index) => (
              <li key={`${index}:${item.text}`}>
                <p>{item.text}</p>
                <small>
                  {[
                    t(`summaryMinutes.itemStatus.${item.status}`),
                    item.owner,
                    item.dueDate,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </small>
                <EvidenceLinks
                  ids={item.evidenceIds}
                  labels={evidenceLabels}
                  onOpen={onEvidence}
                />
              </li>
            ))}
          </ol>
        )}
      </section>
      <SummaryListSection
        items={content.risks}
        labels={evidenceLabels}
        onEvidence={onEvidence}
        title={t("summaryMinutes.sections.risks")}
      />
      <SummaryListSection
        items={content.openQuestions}
        labels={evidenceLabels}
        onEvidence={onEvidence}
        title={t("summaryMinutes.sections.openQuestions")}
      />
    </div>
  );
}

function SummarySection({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <h3>{title}</h3>
      {children}
    </section>
  );
}

function SummaryListSection({
  title,
  items,
  labels,
  onEvidence,
}: {
  title: string;
  items: Array<{
    text: string;
    evidenceIds: string[];
    status?: SummaryDecisionStatus | SummaryActionStatus;
  }>;
  labels: ReadonlyMap<string, number>;
  onEvidence: (id: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  return (
    <section>
      <h3>{title}</h3>
      {items.length === 0 ? (
        <p>{t("summaryMinutes.result.none")}</p>
      ) : (
        <ol>
          {items.map((item, index) => (
            <li key={`${index}:${item.text}`}>
              <p>{item.text}</p>
              {item.status ? (
                <small>{t(`summaryMinutes.itemStatus.${item.status}`)}</small>
              ) : null}
              <EvidenceLinks
                ids={item.evidenceIds}
                labels={labels}
                onOpen={onEvidence}
              />
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function EvidenceLinks({
  ids,
  labels,
  onOpen,
}: {
  ids: string[];
  labels: ReadonlyMap<string, number>;
  onOpen: (id: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  return (
    <span className="summary-evidence-links">
      {ids.map((id, index) => (
        <button
          aria-label={t("summaryMinutes.evidence.open", {
            number: labels.get(id) ?? index + 1,
          })}
          className="text-link"
          key={id}
          onClick={() => void onOpen(id)}
          type="button"
        >
          {t("summaryMinutes.evidence.label", {
            number: labels.get(id) ?? index + 1,
          })}
        </button>
      ))}
    </span>
  );
}

function SummaryEditor({
  draft,
  disabled,
  onChange,
}: {
  draft: SummaryEditableContent;
  disabled: boolean;
  onChange: (draft: SummaryEditableContent) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="summary-editor">
      <label>
        {t("summaryMinutes.sections.overview")}
        <textarea
          disabled={disabled}
          maxLength={16_384}
          onChange={(event) =>
            onChange({ ...draft, overviewText: event.currentTarget.value })
          }
          rows={6}
          value={draft.overviewText}
        />
      </label>
      <EditableTextList
        disabled={disabled}
        items={draft.decisionTexts}
        label={t("summaryMinutes.sections.decisions")}
        onChange={(decisionTexts) => onChange({ ...draft, decisionTexts })}
      />
      <fieldset disabled={disabled}>
        <legend>{t("summaryMinutes.sections.actionItems")}</legend>
        {draft.actionItems.map((item, index) => (
          <div className="summary-editor__action" key={index}>
            <textarea
              aria-label={t("summaryMinutes.edit.actionText", {
                number: index + 1,
              })}
              maxLength={16_384}
              onChange={(event) =>
                onChange({
                  ...draft,
                  actionItems: replaceAt(draft.actionItems, index, {
                    ...item,
                    text: event.currentTarget.value,
                  }),
                })
              }
              rows={3}
              value={item.text}
            />
            <input
              aria-label={t("summaryMinutes.edit.owner", { number: index + 1 })}
              maxLength={255}
              onChange={(event) =>
                onChange({
                  ...draft,
                  actionItems: replaceAt(draft.actionItems, index, {
                    ...item,
                    owner: emptyStringToNull(event.currentTarget.value),
                  }),
                })
              }
              value={item.owner ?? ""}
            />
            <input
              aria-label={t("summaryMinutes.edit.dueDate", {
                number: index + 1,
              })}
              maxLength={255}
              onChange={(event) =>
                onChange({
                  ...draft,
                  actionItems: replaceAt(draft.actionItems, index, {
                    ...item,
                    dueDate: emptyStringToNull(event.currentTarget.value),
                  }),
                })
              }
              value={item.dueDate ?? ""}
            />
          </div>
        ))}
      </fieldset>
      <EditableTextList
        disabled={disabled}
        items={draft.riskTexts}
        label={t("summaryMinutes.sections.risks")}
        onChange={(riskTexts) => onChange({ ...draft, riskTexts })}
      />
      <EditableTextList
        disabled={disabled}
        items={draft.openQuestionTexts}
        label={t("summaryMinutes.sections.openQuestions")}
        onChange={(openQuestionTexts) =>
          onChange({ ...draft, openQuestionTexts })
        }
      />
    </div>
  );
}

function EditableTextList({
  disabled,
  items,
  label,
  onChange,
}: {
  disabled: boolean;
  items: string[];
  label: string;
  onChange: (items: string[]) => void;
}) {
  return (
    <fieldset disabled={disabled}>
      <legend>{label}</legend>
      {items.map((item, index) => (
        <textarea
          aria-label={`${label} ${index + 1}`}
          key={index}
          maxLength={16_384}
          onChange={(event) =>
            onChange(replaceAt(items, index, event.currentTarget.value))
          }
          rows={3}
          value={item}
        />
      ))}
    </fieldset>
  );
}

function editableFromContent(content: SummaryContent): SummaryEditableContent {
  return {
    overviewText: content.overview.text,
    decisionTexts: content.decisions.map((item) => item.text),
    actionItems: content.actionItems.map(({ text, owner, dueDate }) => ({
      text,
      owner,
      dueDate,
    })),
    riskTexts: content.risks.map((item) => item.text),
    openQuestionTexts: content.openQuestions.map((item) => item.text),
  };
}

function replaceAt<T>(items: T[], index: number, value: T): T[] {
  return items.map((item, current) => (current === index ? value : item));
}

function emptyStringToNull(value: string): string | null {
  return value.length === 0 ? null : value;
}

function normalizeEditableContent(
  content: SummaryEditableContent,
): SummaryEditableContent {
  return {
    ...content,
    actionItems: content.actionItems.map((item) => ({
      ...item,
      owner: trimOptionalMetadata(item.owner),
      dueDate: trimOptionalMetadata(item.dueDate),
    })),
  };
}

function trimOptionalMetadata(value: string | null): string | null {
  if (value === null) return null;
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

function evidenceLabelMap(
  content: SummaryContent,
): ReadonlyMap<string, number> {
  const labels = new Map<string, number>();
  const claims = [
    content.overview,
    ...content.decisions,
    ...content.actionItems,
    ...content.risks,
    ...content.openQuestions,
  ];
  for (const claim of claims) {
    for (const id of claim.evidenceIds) {
      if (!labels.has(id)) labels.set(id, labels.size + 1);
    }
  }
  return labels;
}
