# MeetingLens Development Preview User Guide

> Development Preview: this guide describes the local desktop workflows that the current repository actually provides. It is not a production manual and does not imply that real transcription or AI minutes are available.

## 1. Introduction to MeetingLens

MeetingLens is a local-first meeting workspace for Windows 11. Its product goal is to turn pasted or imported meeting transcripts into detailed, editable, traceable minutes. The current development preview primarily validates meeting management, safe file import, the local service, and secure settings boundaries.

The same application includes Simplified Chinese (`zh-CN`) and English (`en-US`); two installations are not required.

## 2. Current version status

The current version is a development preview. Available capabilities include:

- creating, editing, starting, ending, and deleting local meetings;
- running a fully synthetic, memory-only mock transcript flow;
- safely importing, deduplicating, listing, and deleting meeting-file copies through the native picker;
- viewing Mock Provider, local Sidecar, and database status;
- storing a DeepSeek API key in Windows Credential Manager without allowing the WebView to read it back;
- running development validation of the isolated offline Docling Worker, which is not connected to the product UI.

The application cannot currently generate real meeting minutes or process real recordings or realtime transcription.

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

## 5. Home page overview

The Home page shows a meeting-status summary, recent meetings, and the New meeting entry point. Preparing, In meeting, and Ready are localized presentations of stable internal states; users are not asked to manipulate internal values directly.

The Home page does not yet provide paste-transcript, transcript-import, or generate-minutes shortcuts. Those belong to the upcoming transcript-first stages.

## 6. Creating a meeting

1. Select New meeting in the left navigation.
2. Enter a short, recognizable title.
3. Select Create meeting.
4. On the meeting details page, confirm the title, timeline, language baseline, and file area.

Creating a meeting stores local metadata only. It does not request microphone, screen, external-model, or file access.

## 7. Mock meeting flow

1. Select Start meeting on a draft meeting's details page.
2. The application opens the mock meeting workspace and displays synthetic transcript entries.
3. You can pause or resume the mock transcript and add memory-only important marks.
4. Select End meeting.
5. The application returns to meeting details and displays the meeting as Ready.

The mock transcript is not recording, realtime ASR, or real meeting content. Temporary transcript entries, notes, and important marks are not saved as a real transcript when the page closes.

## 8. Importing and managing meeting files

1. Open meeting details and select Add files.
2. Use the native picker opened by MeetingLens to select an allowed file.
3. Review validation, copying, saving, duplicate, or failure progress.
4. After a successful import, review the safe display name, type, size, time, and status in the meeting-file list.

The source absolute path is never returned to the WebView. MeetingLens copies allowed files into private application storage and does not modify the source. Files are not currently opened or parsed in the UI and are not sent to the Sidecar, Docling, or DeepSeek.

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

Select History in the left navigation to view meetings ordered by creation time. Each entry displays a localized status and creation, start, and end times. Select View details to return to meeting details and the file list.

Meetings stored in SQLite should remain after closing and reopening the application. Mock transcript entries and temporary notes are not persistent meeting transcripts.

## 12. Deleting meetings and documents

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
- Imported copies are stored in private application storage; React does not receive absolute paths.
- The DeepSeek key is stored in Windows Credential Manager.
- Interface language is the only application setting allowed in WebView preference storage.
- The current preview does not record audio, read the screen, or send meeting content to external AI.
- Tests and issue reports must use synthetic data only.

See [SECURITY.md](../SECURITY.md) for the detailed boundaries.

## 14. Features not yet supported

The following features are not implemented:

- pasted meeting transcripts;
- TXT, SRT, or VTT transcript import;
- real meeting-minutes generation;
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

Only secure credential storage is complete. The real Summary Provider, immutable input snapshot, sending preview, and evidence validation belong to a later independent stage.

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
- Secure storage unavailable: confirm that the application is running in a supported Windows user session; do not fall back to plaintext files.
- When reporting a problem, include the action, application version, and stable error code. Do not attach keys, meeting content, recordings, databases, logs, private paths, or uploaded files.

## 17. Roadmap

The next core delivery path is:

1. `feat/transcript-data-core`: schema v3, transcript sources and segments, and local paste/TXT/SRT/VTT parsing;
2. `feat/summary-mode-import`: the Summary entry point, paste, native import, preview, and basic speaker correction;
3. `feat/summary-run-core`: immutable snapshots, runs/revisions, evidence, and Mock behavior;
4. `feat/consent-data-core`: shared consent records, session expiry, and audit behavior;
5. `feat/summary-provider-deepseek`: the bounded DeepSeek adapter, SecretStore use, and outbound consent;
6. `feat/summary-mode-minutes`: templates, run state, evidence navigation, and Markdown view, copy, and download;
7. `feat/summary-editing-export`: editing, revisions, stale state, section regeneration, and DOCX;
8. `feat/document-parsing-orchestration` and `feat/document-evidence-index`: later optional reference-document enrichment.

MeetingLens reaches the minimum threshold for generating real minutes from existing transcripts only after the first six stages. A real external Provider still requires user approval of cost, terms, and outbound data scope before implementation. [DEVELOPMENT_PLAN.md](../DEVELOPMENT_PLAN.md) is authoritative for future stages and branches; PR numbers are historical only after creation.
