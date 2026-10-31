# Lume Code usage reference

The full reference for commands, sessions, limits and publishing. For an overview, see the [README](../README.md).

## Install and sign in

For an npm installation, run `npm install --global @selfnify/lume-code`, then
type `lume` from any project folder. The package supports Windows x64 and macOS (Apple Silicon and Intel). macOS support is new and still being tested.
`lume login`, `lume logout`, and `lume --project "C:\path\to\project"` forward to
the native app. `lume --help` and `lume --version` also work without signing in.

### Publish the npm package

The publishable package is in `npm/`. Its explicit file list includes only the
launcher, release executable, public README and package metadata. Rust sources,
local sessions and credentials are not packaged. Consumers do not need Rust.
Before packing or publishing, `prepack` runs the native tests, builds a locked
release, and copies the executable into the package. Run `node scripts/build.cjs`
on each supported OS: it writes `npm/native/<platform>-<arch>/`. Copy the macOS
folders into `npm/native/` on the machine you publish from, then publish with Rust installed. Both Cargo and npm versions must match for each release.

```powershell
cd path\to\Lume-code\npm
npm test
npm login
npm publish --access public --dry-run
npm publish --access public
```

The configured name is `@selfnify/lume-code`; your npm account must own or have
publishing access to the `selfnify` scope. If you use your personal scope instead,
run `npm pkg set "name=@YOUR_NPM_USERNAME/lume-code"` before publishing and use
that name when installing.

To install this local build before publishing, run `npm install --global .` from
the `npm/` folder after `npm pack`, then run `lume` in your project folder.

Build with `cargo build --release`, then run `target\release\lume-code.exe login` on Windows or `target/release/lume-code login` on macOS, or launch normally when signed out. Select **Sign in with browser** (Enter or click). The backend creates an exactly 100-character public token and the app opens Lume's existing login page. Use your normal password or email-code sign-in there, compare the confirmation code, then select **Connect Lume Code**. The desktop app finishes automatically. The login screen lives in `src/login/`.

If the browser does not open, the terminal shows the full public token and URL. Select **Copy URL** or press C, then open that URL yourself. The link lasts ten minutes. Esc or Ctrl-C cancels sign-in; expired or denied requests can be retried with a new link. The URL token is not an account credential: a separate private verifier remains in the desktop process, and only an approved session is saved in Windows Credential Manager or the macOS Keychain. Passwords and email codes are entered only in the existing website.

Run the executable from your project folder, or pass `--project "C:\path\to\project"`. Relative file operations stay inside that folder. The terminal header shows the selected folder. Sessions are stored in Windows Credential Manager or the macOS Keychain; the client contains no model-provider credentials. `lume-code logout` revokes the session and removes its local credential.

## Controls

Type `/` to open the command picker above the textbox. It shows up to five commands, filters as you type, and scrolls as you move with Up/Down. The selected row and arrow are blue. Tab or Enter inserts the selected command; Enter again runs it. Press Esc to close the picker. Press `?` in an empty textbox, or run `/help`, for the keyboard guide. A question mark in a message remains ordinary text.

### Session commands

- `/add-dir PATH` adds an explicitly selected directory to this session (up to eight extras). Lume selects it using the `dir1`, `dir2`, etc. aliases on local tools. Relative file paths and existing traversal/link checks still apply. Commands and destructive actions still require approval.
- `/autocompact 80` sets when automatic summarizing starts, as a percentage of the estimated working-context budget. The default is 80%; accepted values are 10–95%. It is an input-context estimate, not a precise provider-token meter or an output-token cap. Without an argument it shows the current setting.
- `/btw QUESTION` asks a quick text-only side question while the main reply can keep running. It uses a separate request and a small recent-context snapshot; its answer does not enter the main model conversation. One side question runs at a time. It uses the normal account message allowance.
- `/clear` archives the current conversation, then starts a fresh session with empty model context. `/new` is an alias. Project files remain untouched.
- `/cd PATH` moves the active conversation to another working directory and rebinds local tools there. Existing saved sessions at either location are archived before replacement. Paths with spaces can be quoted.
- `/compact` summarizes older context immediately through Lume's existing summarizer. It preserves the latest conversation and original history on disk, cannot run tools or continue the task, and uses the normal message allowance. Tool activity shows compaction progress.
- `/effort off|low|medium|high|max` changes the existing reasoning control. `/reasoning` is an alias. Effort, auto-compaction and added directories are saved with the project session.
- `/logout` revokes the native account session, removes the local credential and exits.
- `/resume` opens a searchable session picker showing titles, last activity dates and working folders. Type to filter, use Up/Down and Enter to open a session, or Esc to close. It includes archives in the current project and saved sessions in previously opened folders, scoped to the signed-in account and API. `/resume ID` and `/resume latest` remain available. The current session is archived before a switch; old account-data epochs cannot restore. The catalog shows up to 200 entries from up to 100 remembered folders, with up to 30 archives per folder.
- `/usage` shows the shared usage counters and reset times without asking the model.
- Usage also appears automatically above the textbox from 90%, switching to a reached-limit banner at 100%. It reads actual session/weekly counters on startup, after accepted messages and completed replies, and every 15 seconds. Expired windows stop showing a stale warning. Click the banner to prepare `/usage`.
- `/cd` and `/add-dir` offer matching folders above the textbox as you type their paths. Up/Down selects; Tab or Enter inserts a quoted path; Enter again runs the command. Tab again drills into the selected folder. The folder strip shows access aliases; click it or use `/dirs` for the full paths.
- `/side-history` displays saved side questions and answers separately. New side questions save their partial answers and completion state locally, survive restarting, and travel with session archives. They never enter the main inference history. The last side answer is shown when restoring a session. Up to 50 side questions are kept per session; Esc stops a side question when no main reply is running. Finish or stop it before switching sessions.
- `/recovery` lists confirmed and unfinished tool activity from the latest interrupted run. The banner's Continue action, or Ctrl-R, prepares a continuation message; Enter sends it. Continuing includes recorded results as untrusted reference data and asks Lume to inspect current files before acting. No command is automatically replayed, and existing approvals remain required.

Replies render Markdown as they stream and when conversations are restored: paragraphs, headings, nested bullet and numbered lists, **bold**, *italic*, strikethrough, checklists, quotes, links, inline code and fenced code blocks. Code keeps its original indentation and literal formatting characters. Your own messages retain their line breaks without interpreting Markdown.

- Shift-Tab changes Plan/Edit. Plan only inspects and plans. A model request for Edit waits for you to change mode and send Continue.
- Tool activity appears inline immediately in expandable groups, separated by Lume's text. Running headers shimmer; completed groups stop animating. Click a header for its tool results and live file details. Ctrl-T or `/tasks` expands the latest group and scrolls to it; it no longer opens a separate bottom drawer. New groups stay collapsed until you choose to open them.
- Escape stops the current response. Completed local writes remain saved.
- Y approves a pending command or destructive file action; N denies it. Expired actions are checked again before execution.
- `/reasoning off|low|medium|high|max` sets the existing Lume reasoning control for subsequent messages (default: off).
- `/usage` shows your shared five-hour and weekly allowance and reset times.
- `/new` starts a new conversation without deleting project files.
- `/attach relative/path.png` uploads an attachment; images are included with your next message. Source files are also available through local read/search tools.
- `/deep your question` asks for a Deep Research proposal.
- `/research show ID` displays the proposal or report. `/research start ID` starts it, `/research watch ID` streams progress and report text, and `/research cancel ID` stops it. `/research edit ID plan.json` pauses and replaces a plan using a local JSON file.
- `/coworker list` lists jobs. `/coworker show ID` shows status and logs. `/coworker upload ID ["main.py","data.csv"]` uploads project files with the first file as entrypoint. Uploading does not start execution. Use `/coworker start ID`, `/coworker stop ID`, or `/coworker restart ID` explicitly.

The ordinary Lume website remains available for account settings, support and graphical file/research views. Terminal file presentations identify the saved local file. Coworker programs and sandbox `run_code` jobs execute remotely; local `run_command` runs on your PC with your user permissions after approval. An approved shell command can affect files outside the selected project, so review the full command.

## Continuity and limits

Conversations and Tasker activity are saved atomically under `%LOCALAPPDATA%\LumeCode\sessions` on Windows or `~/Library/Application Support/LumeCode/sessions` on macOS, separately for each account, backend and project. Credentials are never saved in these JSON files. Account data-epoch changes discard stale local continuity on the next connection. `/clear` and `/new` archive the current conversation before starting fresh; `/resume` restores an archived session. Local history storage is capped at 32 MiB per project conversation.

An atomic progress journal records the current request, partial reply and tool events at roughly half-second intervals and on tool execution/results. Restarting restores that partial reply without duplication, identifies unconfirmed actions, and offers Continue. A stream ending without its completion marker remains interrupted. Progress saved just before a process closes can lag by up to one journal interval; an unconfirmed action must be inspected rather than assumed to have failed. A per-project OS lock prevents two windows from overwriting the same saved session and releases automatically when a process exits. Account verification still needs the API when opening a session. Side history uses a separate local journal and is included in session snapshots, without being sent to the main model.

Local writes are at most 4,096 UTF-8 bytes per action, with resumable byte offsets and atomic replacement. Individual files and uploads are capped at 8 MiB; search/read results are bounded. Links, junctions, traversal, absolute file paths and reserved Windows device names are rejected by file tools. Commands run for at most 60 seconds with 64 KiB retained per output stream. On Windows a Job Object terminates descendants when stopped, and on macOS the command's whole process group is killed. Commands are intended for finite work; use Coworker for long-running programs.

The initial account allowance is 30 accepted messages per five-hour window and 150 per seven-day window, shared with normal Lume. These are Lume defaults, not a claim about another product's exact allowance. Tool rounds do not count as additional messages and output tokens are not capped by this feature. The server exposes both reset times. Administrators can configure `LUME_USAGE_FIVE_HOUR_MESSAGES` and `LUME_USAGE_WEEKLY_MESSAGES`.

## Backend and development

Lume Code connects only to the public Lume API. No backend source, database or inference service is included or required in this project. Server changes remain in the existing backend repository.

The default API is `https://api.lume.dpdns.org`. `LUME_CODE_API` can select another HTTPS backend, or localhost HTTP for development. Credentials and continuity are scoped to the configured backend. No `.env` file from an untrusted project is loaded automatically.

Run `cargo test` for local file, native command cleanup, terminal layout, continuity, streaming, Plan and approval tests. Backend `tests/code-real.py` checks real PostgreSQL ownership, desktop dispatch, shared limits, resets, concurrent admission and epoch invalidation in a disposable schema.
