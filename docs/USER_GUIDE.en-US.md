# MeetingLens Development Preview User Guide

> Development Preview: this guide describes the local desktop workflows that the current repository actually provides. It is not a production manual; existing transcripts can be imported, reviewed, and corrected, while the merged Summary Run Core remains a developer-only zero-network foundation and real AI minutes are not available.

## 1. Introduction to MeetingLens

MeetingLens is a local-first meeting workspace for Windows 11. Its product goal is to turn pasted or imported meeting transcripts into detailed, editable, traceable minutes. The current development preview can safely accept, persist, review, and correct transcripts. PR #21 established the persistent Summary run/snapshot boundary for development validation, but it does not expose real minutes to ordinary users.

The same application includes Simplified Chinese (`zh-CN`) and English (`en-US`); two installations are not required.

## 2. Current version status

The current version is a development preview. Available capabilities include:

- creating, editing, starting, ending, and deleting local meetings;
- choosing among Summary, Translation, and Meeting entries on Home; only Summary can currently be opened;
- pasting UTF-8 meeting notes or importing TXT, SRT, and VTT through the native picker;
- viewing transcript sources, reviewing paginated segments, correcting text and speaker display names, and deleting sources;
- running a fully synthetic, memory-only mock transcript flow;
- safely importing, deduplicating, listing, and deleting meeting-file copies through the native picker;
- viewing Mock Provider, local Sidecar, and database status;
- storing a DeepSeek API key in Windows Credential Manager without allowing the WebView to read it back;
- running development validation of the isolated offline Docling Worker, which is not connected to the product UI.
- on the current feature branch, validating schema-v4 Summary series/revisions, immutable transcript snapshots, run-owned evidence, and a deterministic Rust Mock through development-only APIs and synthetic data.

The user-facing application cannot currently generate real AI meeting minutes, call DeepSeek, process real recordings, or run realtime transcription. Generate meeting minutes remains disabled; Translation and Meeting mode display Coming soon only. The Rust Mock makes no network request and is not a production AI result.

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

- **Summary mode:** available now; opens the paste or transcript-import workflow.
- **Translation mode:** coming soon; it does not start a microphone, realtime translation, or an external request.
- **Meeting mode:** coming soon; it does not request audio or screen permissions.

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

## 7. Reviewing and correcting transcripts

Transcript review shows the meeting title, transcript sources, segment totals, available time range, speaker labels, and input status.

- A source may contain up to 20,000 segments. The page reads stable bounded pages of no more than 200 rows rather than rendering everything at once.
- Relative time displays as `00:12:34`; a segment without timing shows only its paragraph ordinal and never receives a fabricated timestamp.
- Speaker display priority is the user-confirmed name, source label, then Unknown speaker.
- Text and speaker-name edits use revision conflict protection. If content changed elsewhere, reload it; MeetingLens does not overwrite silently.
- A display name can be set or cleared for matching speaker labels in the selected source. Original labels remain unchanged, names do not carry across sources or meetings, and AI does not infer identity.
- Source deletion requires confirmation. Deleting the final source shows no usable input; it never deletes the user's source file or changes the meeting-file area.
- A source used by a queued or running development Summary run cannot be deleted until that run is terminal. Deleting it afterwards keeps only cited evidence, redacts the full snapshot text, and marks the historical result stale/source deleted.

Generate meeting minutes remains disabled with an explicit later-stage notice and makes no network request.

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

The key is stored in the current Windows user's Credential Manager, not SQLite, WebView storage, or the repository. The UI can query configuration status, set, and delete the key, but cannot read it back. No real DeepSeek Provider uses the key in this preview.

Never paste a key into issue reports, logs, screenshots, meeting notes, or version control. Revoke an exposed key immediately.

## 10. Switching the interface language

1. Open Settings.
2. Find Interface language.
3. Select 简体中文 or English.
4. Interface copy changes immediately.

The application stores only an allowed locale enum in the non-sensitive WebView preference `meetinglens.uiLocale`. The selection persists across restarts. Clearing WebView storage resets only the interface language and does not delete meetings, file copies, or credentials.

The interface language is independent of the future meeting-minutes output language. This preview does not provide a minutes-output language setting.

## 11. Viewing meeting history

Select History in the left navigation to view meetings ordered by creation time. Each entry displays a localized status and creation, start, and end times. The current model has no independent editable occurrence date; Created is not presented as the meeting date. Select View details to return to meeting details, transcript actions, and the file list.

Meetings, transcript sources, normalized segments, and correction revisions stored in SQLite should remain after closing and reopening the application. Mock transcript entries and temporary notes are not persistent meeting transcripts.

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

Deletion is irreversible. Confirm that the selected meeting and files are correct first.

## 13. Data storage and privacy

- Meeting metadata is stored in the private application SQLite database.
- Submitted transcript sources and normalized segments are stored in the same private database.
- On the current feature branch, schema v4 stores developer Summary runs and immutable transcript snapshots locally. A run is limited to 20,000 final segments and 10 MiB of UTF-8 text; explicit source deletion redacts terminal full-text snapshots while retaining only cited evidence.
- Unsubmitted pasted text exists only briefly in React memory and is cleared after save, cancel, or navigation away.
- Transcript bodies, speakers, and source paths are not written to localStorage, sessionStorage, IndexedDB, or console.
- Imported copies are stored in private application storage; React does not receive absolute paths.
- The DeepSeek key is stored in Windows Credential Manager.
- Interface language is the only application setting allowed in WebView preference storage.
- The current preview does not record audio, read the screen, or send meeting content to external AI.
- Tests and issue reports must use synthetic data only.

See [SECURITY.md](../SECURITY.md) for the detailed boundaries.

## 14. Features not yet supported

The following features are not implemented:

- user-facing real AI meeting-minutes generation and a formal result workspace;
- the DeepSeek Summary Provider and outbound confirmation;
- real audio recording;
- realtime transcription;
- screen reading;
- speaker diarization;
- realtime translation;
- video import;
- Docling product database and UI orchestration;
- a production PDF user manual and final screenshot workflow.

## 15. Frequently asked questions

### Why can I not generate minutes after saving a DeepSeek key?

Secure credential storage and the zero-network Summary core are separate. The current Rust Mock validates immutable local snapshots and evidence without reading the key. The real Summary Provider, outbound consent/preview, and formal result workspace remain later independent stages.

### Why is Generate meeting minutes unavailable when the transcript is ready?

PR #20 completed input, persistence, paginated review, and manual correction. PR #21 completed developer-only immutable Summary runs, `series_id + revision`, a 20,000-segment/10-MiB snapshot policy, evidence allowlisting, cancellation delivered to the running Mock, stale/delete handling, and interrupted-run recovery through eight minimal commands. Development acceptance uses an isolated operating-system temporary data root and a matching synthetic WebView2 profile rather than the user's normal MeetingLens data locations. Consent Data Core is next, followed separately by the bounded DeepSeek Provider and minutes result workspace; the disabled action does not call AI.

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
- Secure storage unavailable: confirm that the application is running in a supported Windows user session; do not fall back to plaintext files.
- When reporting a problem, include the action, application version, and stable error code. Do not attach keys, meeting content, recordings, databases, logs, private paths, or uploaded files.

## 17. Roadmap

The completed and next delivery path is:

1. `feat/transcript-data-core` (PR #18 merged): schema v3, transcript sources and segments, and local paste/TXT/SRT/VTT parsing;
2. `feat/transcript-command-api` (PR #19 merged): secure transcript commands, the Rust-native picker, and atomic meeting + transcript creation;
3. `feat/summary-mode-input-ui` (PR #20 merged): the Summary entry point, paste, native import, paginated review, and basic speaker/text correction;
4. `feat/summary-run-core` (PR #21 merged): schema v4, immutable full-text snapshots, series/revisions, evidence allowlisting, recovery, and the Rust zero-network Mock;
5. `feat/consent-data-core` (next): shared consent records, session expiry, and audit behavior;
6. `feat/summary-provider-deepseek`: the bounded DeepSeek adapter, SecretStore use, and outbound consent;
7. `feat/summary-mode-minutes`: templates, run state, evidence navigation, and Markdown view, copy, and download;
8. `feat/summary-editing-export`: editing, revisions, stale state, section regeneration, and DOCX;
9. `feat/document-parsing-orchestration` and `feat/document-evidence-index`: connect reference-document parsing and evidence after editing/export, before realtime capture, ASR, screen, speaker, translation, and mode integration.

MeetingLens reaches the minimum threshold for generating real minutes from existing transcripts after the first seven stages; stage eight completes the first transcript-only daily-use loop with editing and export. The two Docling stages then complete Summary-mode reference enrichment without blocking the first minutes. A real external Provider still requires user approval of cost, terms, and outbound data scope before implementation. [DEVELOPMENT_PLAN.md](../DEVELOPMENT_PLAN.md) is authoritative for future stages and branches; PR numbers are historical only after creation.
