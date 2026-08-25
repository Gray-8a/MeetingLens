# MeetingLens Development Preview User Guide

> Development Preview: this guide describes the local desktop workflows provided by the current `feat/summary-mode-minutes` implementation. It is not a production manual; the branch connects transcript input, explicit DeepSeek authorization, five-section results, editing, evidence, Markdown copy/save, and whole-result regeneration while keeping schema v5.

## 1. Introduction to MeetingLens

MeetingLens is a local-first Personal Edition meeting workspace for Windows 11. It turns pasted or imported meeting transcripts into editable, evidence-linked minutes that can be copied or saved as Markdown. The current feature implementation can accept, persist, review, and correct transcripts, explicitly authorize the Rust-only DeepSeek path, show the generated result, save body edits, inspect evidence, and regenerate the whole result as a new revision.

The same application includes Simplified Chinese (`zh-CN`) and English (`en-US`); two installations are not required.

## 2. Current version status

The current version is a development preview. Available capabilities include:

- creating, editing, starting, ending, and deleting local meetings;
- seeing Summary, Translation, and Meeting entries on Home; only Summary can currently be opened, while the Translation and Meeting roadmaps are Frozen/Deferred;
- pasting UTF-8 meeting notes or importing TXT, SRT, and VTT through the native picker;
- viewing transcript sources, reviewing paginated segments, correcting text and speaker display names, and deleting sources;
- selecting transcript sources, one of three templates, and Chinese, English, or bilingual output in the Minutes workspace;
- explicitly generating or continuing a DeepSeek run, viewing queued/running/completed/failed/cancelled status, and cancelling active work;
- viewing the fixed `overview`, `decisions`, `actionItems`, `risks`, and `openQuestions` result, exact cited evidence, and revision history;
- editing the allowed body fields with conflict protection, copying canonical Markdown, saving `.md` through a Rust-owned native dialog, and regenerating the whole result as a new revision;
- running a fully synthetic, memory-only mock transcript flow;
- safely importing, deduplicating, listing, and deleting meeting-file copies through the native picker;
- viewing Mock Provider, local Sidecar, and database status;
- storing a DeepSeek API key in Windows Credential Manager without allowing the WebView to read it back;
- running development validation of the isolated offline Docling Worker, which is not connected to the product UI.
- validating schema-v4 Summary series/revisions, immutable transcript snapshots, and run-owned evidence, plus schema-v5 minimal remembered consent;
- using Settings to see the fixed DeepSeek host/model, remembered-consent state and time, and to revoke future authorization;
- validating the Rust DeepSeek protocol, strict JSON/evidence checks, failures, and cancellation with synthetic transports; the deterministic Mock remains test/debug-only and historical Mock runs remain readable/exportable but read-only.

The user-facing application can initiate DeepSeek generation only after an explicit Generate or Continue action and the required disclosure/consent checks. Remembered consent never authorizes page-load sending, restart replay, or background retry. Recording-file transcription, live recording, and realtime transcription are not available. Translation and Meeting still display Coming soon and remain Frozen/Deferred. The Rust production provider is fixed to DeepSeek; Mock is isolated to tests/debug and makes no network request. A real paid DeepSeek API smoke has not been performed, so real key/account/balance/public-network service behavior is not claimed as validated.

## 3. System requirements

- Windows 11;
- WebView2;
- for source development: Node.js 22+, npm 10+, Python 3.12.x, Rust stable, and Visual Studio Build Tools 2022;
- the Desktop development with C++ workload with MSVC v143, a Windows 10/11 SDK, and C++ CMake tools.

Windows 11 and Python 3.12 are the project baselines. Other systems or Python versions do not replace formal acceptance on those baselines.

## 4. Starting the application

Install locked dependencies and start the desktop development build from the repository root:

```powershell
npm ci
npm run tauri dev
```

Run the complete automated checks with:

```powershell
npm run check
```

The desktop environment is verified only after the native MeetingLens window opens and the Settings page reports local runtime status. Passing CI does not replace local window acceptance.

## 5. Home and the three modes

Home presents three task-oriented modes:

- **Summary mode:** available now; opens transcript input/review and the Minutes workspace for generation and results.
- **Translation mode:** the UI displays Coming soon, while the roadmap status is Frozen/Deferred; it does not start a microphone, realtime translation, or an external request.
- **Meeting mode:** the UI displays Coming soon, while the independent workspace is Frozen/Deferred; it does not request audio or screen permissions.

Meeting history remains available. Create meeting manually is an advanced preview path with lower visual priority than Summary mode, but it is not removed. Home does not require ordinary users to understand Providers, Docling, or internal state machines.

## 6. Summary mode input

### Paste meeting notes

1. Select Summary mode on Home.
2. Optionally enter a meeting title. When left empty, the application creates a localized title from the local date and time.
3. Paste strict UTF-8 text. The page displays its actual UTF-8 byte use, up to 2 MiB.
4. Select Save and review. MeetingLens atomically creates the draft meeting and transcript; a failure does not leave an empty meeting.
5. After success, the Transcript review page opens and the unsubmitted React text state is cleared.

### Import a meeting transcript

1. Optionally enter a meeting title.
2. Select Choose TXT, SRT or VTT.
3. Select one strict UTF-8 file, up to 10 MiB, in the native Windows picker opened by MeetingLens.
4. Cancelling is a safe no-op and does not create an empty meeting.
5. Matching content in the same meeting displays Already added and does not create a second source.

The absolute source path never returns to the WebView, and the source file is not copied, changed, or deleted. TXT, SRT, and VTT use the bounded Rust parser, not Docling, the Python Sidecar, or external AI.

### Generate meeting minutes

1. From Transcript review or meeting details, open the Minutes workspace.
2. Select one or more transcript sources in the intended order. Sources with no segments cannot be selected.
3. Select Detailed meeting minutes, Concise meeting summary, or Actions and decisions, then choose Chinese, English, or Bilingual output. Interface language and output language are independent.
4. Select Generate. If no API key is configured, use the Settings link; no run or network request is created.
5. Review the fixed Provider/host/model, frozen source/segment/byte counts, template, language, and data categories. Choose Allow once, Remember my choice, or Cancel. Closing or cancelling a disclosure for a newly queued run cancels that run.
6. View queued/running state and use Cancel when required. A restored queued run never starts automatically; select Continue generation or Cancel task.
7. After completion, review Overview, Decisions, Action items, Risks, and Open questions. Select a citation to retrieve that exact evidence item.

Remembered consent can shorten a newly and explicitly triggered Generate action. It does not send data when the page opens, the application starts, or a failed/queued task is restored. MeetingLens performs no automatic Provider retry.

### Edit, copy, save, and regenerate

- Select Edit to change existing section text and action-item owner/due date. Items, statuses, evidence links, order, and Provider metadata cannot be added, removed, or changed.
- Select Save changes to persist the current revision. Conflict protection asks you to reload if another view changed the content. The first real edit marks the revision as user edited; the original Provider draft and a separate edit timeline are not retained inside that revision.
- Save pending edits before Copy Markdown or Save Markdown. Both operations use the same canonical Rust Markdown output. Copy writes only after an explicit click; MeetingLens does not read the clipboard.
- Save Markdown opens a native Windows Save As dialog owned by Rust. Cancelling is a safe no-op, and the selected path is never returned to the WebView.
- Regenerate whole result is available only from the latest terminal DeepSeek revision. It freezes the current live transcript into a new revision and preserves the old revision; it never sends the edited minutes back to DeepSeek. Older revisions direct you to the latest one, and historical Mock results are read-only.
- A `segment_revised` notice allows explicit whole-result regeneration. A `source_deleted` result remains viewable, editable, copyable, and saveable, but cannot regenerate within that series.

Evidence continues to refer to the original frozen transcript excerpt. Editing a claim does not cause AI or Rust to revalidate the new wording. Markdown saved outside MeetingLens and clipboard content are user-controlled copies and are not removed when a meeting is deleted.

## 7. Reviewing and correcting transcripts

Transcript review shows the meeting title, transcript sources, segment totals, available time range, speaker labels, and input status.

- A source may contain up to 20,000 segments. The page reads stable bounded pages of no more than 200 rows rather than rendering everything at once.
- Relative time displays as `00:12:34`; a segment without timing shows only its paragraph ordinal and never receives a fabricated timestamp.
- Speaker display priority is the user-confirmed name, source label, then Unknown speaker.
- Text and speaker-name edits use revision conflict protection. If content changed elsewhere, reload it; MeetingLens does not overwrite silently.
- A display name can be set or cleared for matching speaker labels in the selected source. Original labels remain unchanged, names do not carry across sources or meetings, and AI does not infer identity.
- Source deletion requires confirmation. Deleting the final source shows no usable input; it never deletes the user's source file or changes the meeting-file area.
- A source used by a queued or running development Summary run cannot be deleted until that run is terminal. Deleting it afterwards keeps only cited evidence, redacts the full snapshot text, and marks the historical result stale/source deleted.

Open Minutes to generate from the selected sources. Network access is still impossible until you explicitly select Generate/Continue and satisfy Rust's key, disclosure, consent, frozen-scope, and run-state checks.

## 8. Advanced mock meeting and meeting files

Create meeting manually remains available to validate the existing desktop preview:

1. Create a draft and select Start meeting on its details page.
2. Review the fully synthetic, memory-only captions, pause or resume them, and add temporary important marks.
3. End the meeting to return to details. Mock captions and temporary notes are not persisted as a formal transcript.

Meeting details also lets `draft`, `recording`, and `ready` meetings paste/import transcripts and open Transcript review. `processing`, `deleting`, `delete_failed`, and other disallowed states safely disable changes; the backend remains authoritative.

Reference meeting files continue to use the separate Add files workflow:

1. Open meeting details and select Add files.
2. Use the native picker opened by MeetingLens to select an allowed file.
3. Review validation, copying, saving, duplicate, or failure progress.
4. After a successful import, review the safe display name, type, size, time, and status in the meeting-file list.

Reference files and transcript sources are different data types. Reference files are copied into private application storage without changing the source. They are not currently opened or parsed in the UI and are not sent to the Sidecar, Docling, or DeepSeek.

## 9. Setting a DeepSeek API key

1. Open Settings.
2. Enter the key in the DeepSeek API area.
3. Select Save API key.
4. Confirm that the status is Configured and the input remains empty.

The key is stored in the current Windows user's Credential Manager, not SQLite, WebView storage, or the repository. The UI can query configuration status, set, and delete the key, but cannot read it back. Only the trusted Rust DeepSeek Provider may read it immediately before an authorized request; saving a key does not validate it or send anything.

Settings also shows the fixed provider (`deepseek`), host (`api.deepseek.com`), model (`deepseek-v4-pro`), and whether Remember my choice is active. Select Revoke sharing consent to require confirmation again for future sends and clear both remembered authorization and every unused one-time confirmation. Revocation cannot recall data already delivered to DeepSeek. There is deliberately no button to authorize without a specific frozen Summary run and no model or base-URL editor.

Never paste a key into issue reports, logs, screenshots, meeting notes, or version control. Revoke an exposed key immediately.

## 10. Switching the interface language

1. Open Settings.
2. Find Interface language.
3. Select 简体中文 or English.
4. Interface copy changes immediately.

The application stores only an allowed locale enum in the non-sensitive WebView preference `meetinglens.uiLocale`. The selection persists across restarts. Clearing WebView storage resets only the interface language and does not delete meetings, file copies, or credentials.

The interface language is independent of the meeting-minutes output language selected in the Minutes workspace. Changing the UI language never changes an existing run or silently changes the next output choice.

## 11. Viewing meeting history

Select History in the left navigation to view meetings ordered by creation time. Each entry displays a localized status and creation, start, and end times. The current model has no independent editable occurrence date; Created is not presented as the meeting date. Select View details to return to meeting details, transcript actions, and the file list.

Meetings, transcript sources, normalized segments, correction revisions, Summary runs/results, evidence, and saved body edits stored in SQLite should remain after closing and reopening the application. A running Summary abandoned by a crash becomes safely interrupted; a queued Summary remains queued and requires an explicit Continue action. Mock transcript entries and temporary notes are not persistent meeting transcripts.

## 12. Deleting meetings, transcripts, and documents

To delete a transcript source:

1. Select Delete source in Transcript review.
2. Confirm removal of its normalized segments from MeetingLens; the user's original TXT/SRT/VTT remains unchanged.
3. A failed deletion keeps the source visible with a stable error instead of claiming early success.

To delete a document:

1. Select Delete for a document on the meeting details page.
2. Confirm that only the MeetingLens local copy will be deleted; the source remains unchanged.
3. Use Retry deletion if deletion fails.

To delete a meeting:

1. Only Preparing or Ready meetings can request deletion.
2. End an In meeting session before deleting it.
3. After confirmation, MeetingLens coordinates database records and controlled file copies.
4. Deleting or Needs attention means recovery has not completed; do not create replacement data to hide the state.

Deletion is irreversible. Confirm that the selected meeting and files are correct first. Meeting deletion removes locally managed runs/results/evidence but cannot recall data already delivered to DeepSeek, clear the system clipboard, or delete Markdown files that you saved outside MeetingLens.

## 13. Data storage and privacy

- Meeting metadata is stored in the private application SQLite database.
- Submitted transcript sources and normalized segments are stored in the same private database.
- Schema v4 stores Summary runs and immutable transcript snapshots locally. Its durable limit remains 20,000 final segments and 10 MiB of UTF-8 text; the production DeepSeek policy is limited to 1 MiB and fails before transport when exceeded. Explicit source deletion redacts terminal full-text snapshots while retaining only cited evidence.
- Schema v5 stores at most one minimal remembered DeepSeek preference: provider, policy version/fingerprint, and consent/update times. It never stores transcript text, run IDs, prompts, responses, paths, or keys. One-time consent exists only in Rust memory for up to 10 minutes and is consumed once.
- Minutes editing reuses schema v5: it updates the current revision's five-section `content_json` and sticky `is_user_edited` flag. A Rust-derived `contentVersion` protects edits and Markdown output but is not stored in SQLite. There is no separate Provider-draft copy or edit-history table in the same revision.
- Unsubmitted pasted text exists only briefly in React memory and is cleared after save, cancel, or navigation away.
- Transcript bodies, speakers, and source paths are not written to localStorage, sessionStorage, IndexedDB, or console.
- Imported copies are stored in private application storage; React does not receive absolute paths.
- The DeepSeek key is stored in Windows Credential Manager.
- Interface language is the only application setting allowed in WebView preference storage.
- The current preview does not record audio or read the screen. The user-facing Generate/Continue controls can ask the Rust provider to send only a frozen, explicitly authorized transcript scope to the fixed DeepSeek endpoint; React, Python, and Docling cannot make that request.
- Canonical Markdown is generated in Rust. Native Save As does not store a destination path in SQLite or return it to React; clipboard and external `.md` copies remain outside MeetingLens lifecycle control.
- Tests and issue reports must use synthetic data only.

See [SECURITY.md](../SECURITY.md) for the detailed boundaries.

## 14. Features not yet supported

The following near-term Personal Edition capabilities are not implemented:

- local batch transcription of a recording file;
- simplified Docling product orchestration and document evidence;
- optional user-selected window context.

The following capabilities are Frozen/Deferred and are not part of the current Personal Edition roadmap:

- live audio recording, WASAPI, and realtime transcription;
- realtime speaker diarization and identity enhancement;
- realtime translation, caption overlay, checkpoints, and automatic topic detection;
- independent Translation and Meeting workspaces;
- section/paragraph regeneration, edit history/restore, rich HTML results, collaborative conflict merging, Structured Visuals, video import, commercial distribution, and DOCX/PDF export;
- a production PDF user manual and final screenshot workflow.

## 15. Frequently asked questions

### Why can I not generate minutes after saving a DeepSeek key?

Saving a key only configures SecretStore; it does not validate the account or authorize a request. Open Minutes, select at least one non-empty transcript source, choose the template/language, and select Generate. Rust still requires an eligible meeting, a valid frozen scope, current consent, and a terminal-safe run state. Only stable safe errors are shown; a real paid API smoke has not been performed by this project.

### Why does a queued result ask me to continue after restart?

MeetingLens never treats remembered consent as permission for startup or background sending. A queued run restored from SQLite stays queued until you explicitly select Continue generation or Cancel task. A run that had already reached running before an abnormal exit is marked interrupted and is not replayed automatically.

### Does transcript import copy or delete the source file?

No. Rust reads it only during selection and stores normalized segments; the absolute path does not enter the WebView or SQLite. Deleting a transcript source does not affect the user's file.

### Why is there no parsed result after importing a file?

The current UI provides safe copying and lifecycle management only. The offline Docling Worker development foundation is complete but is not connected to the database, commands, or UI.

### Does switching languages translate meeting titles or filenames?

No. Titles, filenames, and future transcripts are user data. They are never used as translation keys or translated automatically.

### Does clearing browser or WebView storage delete meetings?

No. It resets only the non-sensitive interface-language preference. Meetings and file copies are managed by the local database and private file storage.

## 16. Troubleshooting

- Application does not start: confirm that the C++ Build Tools, WebView2, Rust, Node.js, and Python 3.12 are available, then run `npm run check`.
- Local service unavailable: close residual development processes and restart the application; do not weaken authentication or security checks.
- Database unavailable: stop write actions and retain the safe error code; do not edit or delete database files directly.
- File import fails: check the type, size, quota, and file integrity; do not rename an extension to bypass validation.
- Transcript import fails: confirm that it is a strict UTF-8 TXT/SRT/VTT file no larger than 10 MiB and that cue/timestamp syntax is complete. Do not include the path or transcript body in a report.
- Correction conflict: reload the current segment before editing again; do not bypass revision protection.
- Minutes generation is blocked: configure the key, select a non-empty source, review the disclosure, and use Generate/Continue explicitly. Do not retry by changing Provider URLs or bypassing consent.
- Minutes edit conflict: reload the current revision before reapplying the edit; stale `contentVersion` output is intentionally rejected.
- Clipboard unavailable: use Save Markdown. MeetingLens never reads clipboard content and does not use a legacy copy fallback.
- Markdown save fails: choose a writable local `.md` target that is not a directory, symlink, or reparse-point escape. Do not include the chosen path or minutes body in a report.
- Secure storage unavailable: confirm that the application is running in a supported Windows user session; do not fall back to plaintext files.
- When reporting a problem, include the action, application version, and stable error code. Do not attach keys, meeting content, recordings, databases, logs, private paths, or uploaded files.

## 17. Roadmap

The completed and next Personal Edition delivery path is:

1. `feat/transcript-data-core` (PR #18 merged): schema v3, transcript sources and segments, and local paste/TXT/SRT/VTT parsing;
2. `feat/transcript-command-api` (PR #19 merged): secure transcript commands, the Rust-native picker, and atomic meeting + transcript creation;
3. `feat/summary-mode-input-ui` (PR #20 merged): the Summary entry point, paste, native import, paginated review, and basic speaker/text correction;
4. `feat/summary-run-core` (PR #21 merged): schema v4, immutable full-text snapshots, series/revisions, evidence allowlisting, recovery, and the Rust zero-network Mock;
5. `docs/personal-edition-descope` (PR #23 merged): Personal Edition scope, Frozen/Deferred boundaries, and development-process re-baseline;
6. `feat/summary-provider-deepseek` (PR #24 merged): schema v5 minimal preference, Rust DeepSeek/SecretStore, strict JSON/evidence, one-time/remember/revoke, cancellation and fail-closed behavior; no general Consent Data Core;
7. `feat/summary-mode-minutes` (current feature delivery, expected PR #25, daily-use milestone): formal Generate/consent/status, five-section Chinese/English/bilingual results and exact evidence, protected body editing, canonical Markdown copy/native save, and whole-result regeneration while preserving old revisions; no section regeneration or DOCX/PDF;
8. `feat/audio-file-transcription` (next only after the current feature is reviewed and merged; expected PR #26): locally batch-transcribe a user-imported meeting recording through a one-shot worker; no live audio, WASAPI, or realtime ASR;
9. `feat/document-parsing-orchestration` (expected PR #27): simplified wiring of the existing Docling Worker, document chunks/evidence, one automatic retry, then a prompt to re-import;
10. `feat/window-context` (optional, expected PR #28 or later): read only a specific window explicitly selected by the user, and only after a repeated real need is confirmed.

The current `feat/summary-mode-minutes` delivery targets the daily-use loop from an existing transcript to real minutes that can be viewed, edited, copied, or saved. Realtime capture/ASR, realtime translation, speaker diarization, checkpoints, independent Translation/Meeting workspaces, Structured Visuals, commercial distribution, and DOCX/PDF remain Frozen/Deferred. Minimal Consent is for a single-user Personal Edition and is not an enterprise compliance audit system; a request without valid confirmation must fail closed. [DEVELOPMENT_PLAN.md](../DEVELOPMENT_PLAN.md) is authoritative for future stages and branches; expected PR numbers are not authoritative identifiers.
