# Wega UI/UX review — 22 October 2026

Implementation status: all 16 findings now have code changes. See
[implementation and verification](ui-ux-implementation.md). The observations
below describe the original pre-change experience. The [23 October second-pass
proposals](ui-ux-proposals.md#implementation-status--23-october-2026) have their own
implementation status and remaining backlog; the original 16-item completion
claim does not apply to that newer backlog. Continued implementation now includes
the address-bar site panel, theme swatches, compact tabs and explicit viewer
clipboard actions. The third follow-up adds new-tab tiles, About, Network layout
changes, quality presets and a Windows installer definition; see the implementation
log for validation limits, particularly the unrun Windows installer checks. The
fourth follow-up adds exact-origin permission blocks, address host emphasis and
request-scoped viewer-to-file-picker handoff and a mobile keyboard control;
live Docker/device checks remain.

The [24 October checkpoint](ui-ux-implementation.md#remaining-proposal-implementation--24-october-2026) adds frame-backlog quality adaptation, local viewer gestures, Windows media packaging/backdrop/instance handoff, per-element media messages and a scoped Vector icon fallback. Windows runtime, physical-device checks and engine-backed permission revocation remain open.

The current browser has consistent emerald styling, useful top/sidebar layouts,
and substantial working developer tools. It still has important gaps in input
flows, accessibility, theme coverage, and responsive behavior. Passing functional
tests does not establish that these experiences are usable or accessible.

This is an audit, not an implementation. Existing README and website edits were
preserved. No application code was changed for this review.

## Scope and evidence

- Ran all seven native suites against the current instrumented executable with
  isolated profiles: smoke, theme, capabilities, toolbox, download, WebDriver,
  and media. All passed. Results: `target/ui-ux-review/native/results.json`.
- Visually inspected current captures of history, Paper, Glass Light, Midnight,
  Network, Debugger, and right-docked Inspector. The theme suite exported all six
  themes. These are macOS captures, primarily at the default 1280-point window.
- Captured Paper with `wega://newtab` and the local website in the current
  production build. Evidence: `target/ui-ux-review/paper-newtab.window.png` and
  `target/ui-ux-review/website.window.png`.
- Reviewed desktop navigation, custom widgets, dialogs, settings, downloads,
  internal documents, Docker login/viewer, and website source.
- Windows/Linux native appearance, VoiceOver/NVDA operation, real mobile devices,
  real OS installation prompts, and wallpaper-dependent glass contrast were not
  interactively validated. Responsive findings below distinguish code-derived
  risks from observed screenshots. Docker findings are source-based; no new
  container session was started.

P1 means a broken or exclusionary task that should be addressed before broader
testing. P2 means substantial friction or inconsistency. P3 means polish.

## Findings and proposed changes

### 1. P1 — Website hero is unreadable in Wega itself

**Observed:** the headline sits on a full rectangular white/green gradient with
very pale lettering. This is visible in `website.window.png`. The CSS relies on
WebKit text clipping/fill behavior that is not rendering as intended in this
engine. [Source](../site/index.html#L134).

Use readable solid headline text as the base presentation. Enable gradient text
only behind a tested capability/fallback path; test this exact landing-page
selector in Servo as well as mainstream browsers. The existing interaction
fixture's text-gradient checks did not prevent this page-specific problem.

**Acceptance:** the full headline is readable in Wega, Chromium, Firefox and
Safari; no gradient rectangle covers the heading in the fallback rendering.

### 2. P1 — File upload has no normal file-selection experience

Users must type absolute paths rather than browse for a file. If a path is
relative, missing, or otherwise invalid, clicking Choose dismisses the picker
without an explanation. [Input](../src/ui.rs#L1312),
[validation and dismissal](../src/ui.rs#L1336).

Use native file dialogs on desktop, with file-type and multiple-selection rules.
Keep a path-entry fallback for constrained environments, show validation beside
the field, and keep the dialog open on errors. Docker needs a separate local
upload-to-remote flow; a local path cannot identify a file inside the container.

**Acceptance:** select a real file without typing a path; invalid selections
preserve input and explain the error; Cancel never submits files.

### 3. P1 — JavaScript prompt defaults can be lost

The prompt's default value is used as `hint_text`, while submission reads the
separate `chrome.prompt_text` buffer. Accepting an untouched prompt therefore
submits the buffer rather than the displayed default.
[Source](../src/ui.rs#L1226).

Initialize the editable buffer once when the prompt is queued. Keep default text,
an explicitly cleared value, and cancellation distinct.

**Acceptance:** `prompt('Name', 'Alice')` returns `Alice` when accepted unchanged,
returns an empty string after deliberate clearing, and returns null on Cancel.

### 4. P1 — Keyboard dismissal and dialog actions are incomplete

Site dialogs, authentication, permissions and data-clear confirmations handle
button clicks but discard the modal response. egui's `should_close()` response
is where Escape/backdrop dismissal is exposed. Default Enter actions and initial
field focus are also not explicitly provided in these flows.
[Dialogs](../src/ui.rs#L1154), [authentication](../src/ui.rs#L1174),
[clear data](../src/capabilities_ui.rs#L299).

Create shared dialog behavior: initial focus, visible focus, Tab order, Enter
for the appropriate default action, Escape as Cancel/Deny, and focus restoration.
Do not let dismissal authorize a permission or destructive action. Decide
backdrop behavior explicitly rather than treating it as consent.

**Acceptance:** complete login, prompt, permission denial and cancellation with
only the keyboard, without getting trapped or changing hidden page state.

### 5. P1 — Accessibility inspection is not OS screen-reader integration

The engine accessibility callback serializes nodes into toolbox events, and the
Accessibility tool activates the page tree for inspection. I found no application
bridge grafting that tree into the native window's accessibility tree or routing
assistive actions back to it. Custom toolbar icons, tabs and library rows are
painted manually without explicit semantic labels/roles or focus decoration.
[Engine callback](../src/engine.rs#L234), [activation](../src/toolbox.rs#L306),
[icons](../src/ui.rs#L215), [tabs](../src/ui.rs#L716),
[library rows](../src/ui.rs#L1012).

Connect chrome and page accessibility to the platform adapter; expose meaningful
labels, roles, selection and loading states. Preserve hover-only bookmark removal
for pointer users while exposing the action on keyboard focus or a context menu.
This is a code-level integration gap; VoiceOver/NVDA failure was not measured.

**Acceptance:** a screen reader can identify and operate navigation, tabs,
bookmarks and page controls; keyboard focus is visible on every custom control.

### 6. P2 — Internal pages ignore the chosen theme

**Observed:** Paper chrome surrounds a dark-green `wega://newtab` page. Internal
documents hardcode emerald colors, so browser-owned history, bookmarks and settings
documents have the same mismatch. This is browser-owned content, not a request
to recolor arbitrary websites. [Source](../src/capabilities.rs#L101).

Render internal documents from the active palette and update existing documents
when themes change. Keep website colors independent of chrome appearance.

**Acceptance:** all internal pages match each of the six themes immediately,
including system appearance changes in Auto mode.

### 7. P2 — Status feedback treats success as an error

The shared status strip renders every `self.status` message using `colors.danger`.
Successful screenshots, saved scripts and cleared browsing data therefore look
like failures. Feedback is truncated and may require hovering to read the action's
result. [Strip](../src/ui.rs#L430),
[success example](../src/capabilities_ui.rs#L228).

Use typed success/info/warning/error messages, matching icons, and suitable
durations. Keep actionable failures available until dismissed; offer Reveal for
saved screenshots and a way to read full messages without a hover requirement.

### 8. P2 — Small-window layout lacks a coordinated compact mode

**Code-derived risk:** the minimum window is 720 points wide, while the sidebar
can occupy 240 by default and the library another 320. That leaves roughly 160
points before central padding. Developer tools introduce more minimum widths,
and the navigation row reserves fixed control sizes.
[Window](../src/main.rs#L87), [sidebar](../src/ui.rs#L353),
[library](../src/ui.rs#L902), [navigation](../src/ui.rs#L557).

Define a shared width budget: collapse the sidebar, show the library as an
overlay/sheet, move secondary toolbar actions into overflow, and use an explicit
minimum remaining page width. Test 720×480 and 1024×768 with library and tools open.
Do not assume the existing 1280-point geometry assertions cover these cases.

### 9. P2 — Developer-tool footer is visibly clipped

**Observed:** the Network capture's summary wraps into the bottom clipped edge.
The request list reserves a fixed 28-point allowance, while the summary is a
wrapping label. [List budget](../src/devtools_ui.rs#L621),
[summary](../src/devtools_ui.rs#L758).

Reserve the summary's measured height or give it a stable footer with an expandable
detail view. In narrow docks, move secondary actions into an overflow menu and
make the request-list/detail split discoverable.

**Acceptance:** summary, horizontal scrollbar and close controls remain visible
at minimum tool height, in all dock positions and at larger UI text sizes.

### 10. P2 — Completed downloads require path copying

The completed state exposes Copy saved path but no Open file or Show in folder.
The download form prominently exposes connection count and protocol details,
and its URL field has no explicit Enter-to-start handling.
[Source](../src/downloads_ui.rs#L27).

Provide Open and Show in folder on desktop, retain Copy path as secondary, and
move connection count/protocol details under Advanced. Add a clear empty state,
field validation, and Enter submission. Keep the Docker viewer's save-to-local
behavior distinct from opening a file on the server.

### 11. P2 — Browser settings mix everyday tasks and engine diagnostics

Host rules, offline mode, page scripts, browsing data, media controls and WebRender
debugging share one long settings window. Internal `wega://settings` only tells
the user to open another menu. [Settings](../src/capabilities_ui.rs#L205),
[internal page](../src/capabilities_ui.rs#L39).

Group settings into Appearance, Browsing, Privacy/site data, Downloads and Advanced.
Put rendering diagnostics in developer tools. Make the internal settings route
lead to a useful settings surface without giving untrusted pages native privileges.

### 12. P2 — Destructive site-data actions are inconsistent

History and full data clearing ask for confirmation, but a Stored sites Delete
button immediately removes all storage types for that site. It gives little
indication that this may sign the user out or remove offline data.
[Source](../src/capabilities_ui.rs#L279).

Name the site and affected data before deletion, explain the consequence, and
use consistent destructive styling and confirmation. Preserve the existing
history-clear confirmation.

### 13. P2 — Mobile website navigation disappears and cards can overflow

At widths at or below 768px, the entire navigation is hidden without a replacement.
The feature grid keeps a 320px minimum column inside a container with 48px total
horizontal padding. At a 360px viewport, available content is only 312px, while
body overflow is hidden. This can clip cards instead of reflowing them.
[Navigation](../site/index.html#L361), [grid](../site/index.html#L267).

Add a labeled compact menu, preserve the download/repository actions, and use
`minmax(min(100%, 320px), 1fr)` or a single-column small-screen override. Add
anchor scroll offsets for the sticky header. These findings are source-derived;
mobile Safari/Chrome rendering is still to be verified.

### 14. P2 — Docker's remote UI scales rather than adapting

The viewer scales its remote viewport and explicitly disables session resizing.
Smaller screens can therefore receive an entire desktop UI shrunk into the
available area, with small text and targets. The outer toolbar wrapping does not
solve the remote desktop's scale. [Source](../docker/viewer/viewer.js#L14).

Offer Fit, Actual size, and a clear full-screen path. Evaluate safe dynamic remote
resolution and provide zoom/pan or a documented desktop-first fallback. Verify
touch input, on-screen keyboard, clipboard and upload/download handoff on devices.

### 15. P2 — Website Copy reports success before copying completes

The handler sets Copied immediately after calling `clipboard.writeText`, without
awaiting or catching rejection. Permission denial or an unavailable Clipboard API
can therefore produce a misleading success or no useful feedback.
[Source](../site/index.html#L418).

Await the operation, report success only on resolution, and offer selectable text
with an explanatory message when copying fails. Announce the result accessibly.

### 16. P3 — New-tab onboarding and tab titles could be more useful

The new-tab page is mostly introductory text and a link to Google; it offers no
in-page search or saved-site shortcuts. Tab titles use character-count shortening
and reserve favicon space even when no icon is present, reducing useful title
space. [New tab](../src/capabilities_ui.rs#L8), [tabs](../src/ui.rs#L725).

Provide a compact theme-aware start page with search guidance, bookmarks and recent
pages. Use measured pixel-based text elision, a fallback favicon, and retain full
title/URL access on hover and keyboard focus. Preserve whole-row selection,
reordering and the current tab baseline alignment.

## Theme and accessibility assessment

The opaque palettes are a strength. Calculated from the declared unblended colors:

| Theme | Body text / chrome | Muted text / chrome | Body text / selected |
| --- | ---: | ---: | ---: |
| Emerald | 14.89:1 | 8.75:1 | 6.62:1 |
| Midnight | 14.47:1 | 7.48:1 | 7.43:1 |
| Paper | 13.06:1 | 5.22:1 | 10.65:1 |

These sampled text pairs exceed the 4.5:1 normal-text target. This is not an audit
of every control, disabled state, website, or alpha-composited glass background.
Glass Light appears flat gray in the exported frame; judge native vibrancy with
the OS compositor and representative wallpapers before changing it based on a
GPU export alone. Test reduced transparency, increased contrast and focus rings.

Use [WCAG text contrast](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)
and [non-text contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)
as acceptance references: normal text generally needs 4.5:1, and essential
non-text component/state cues need 3:1 against adjacent colors. Screen-reader
and keyboard task testing are required in addition to palette checks.

## Proposed implementation order

1. Fix the website heading, file picker/error handling, prompt values and keyboard
   dialog lifecycle. Establish the native accessibility integration plan.
2. Complete internal-page themes, semantic feedback, compact layout budgets and
   the clipped developer-tool footer.
3. Improve downloads, settings grouping, site-data deletion, mobile website
   navigation, Docker viewport controls and Copy failure feedback.
4. Improve onboarding and title elision, then add a focused UX regression matrix:
   six themes; top/sidebar tabs; 720/1024/1280 widths; docked/detached tools;
   keyboard-only tasks; representative assistive technology; Docker device flows;
   first download/install on each supported OS.

Keep the existing pointer, layout, console, debugger, history and profile checks.
Add tests for user outcomes above; broad line coverage cannot substitute for
usability, visual review or assistive-technology validation.


### 23 October CI follow-up

The original implementation and proposal follow-ups were committed as `7d6b1bc`.
The subsequent [CI corrections and continued work](ui-ux-implementation.md#ci-corrections-and-continued-work--23-october-2026)
track the missing Docker build script, chooser coverage gaps and macOS navigation
race separately from the remaining platform/device validation. New-tab keyboard
focus and Windows associated-file path handling are included in this follow-up.


### Subsequent remaining-work follow-up

The CI corrections were committed as `6256600` before further work. The next
follow-up adds persisted most-visited shortcut ranking and macOS URL/HTML bundle
associations and launch-event delivery. It also fixes the pinned Servo Rc/Box
allocation mismatch behind the Linux Memory crash; the fresh Linux ARM64 toolbox
and runtime/shell regressions now pass. See the latest
[implementation checkpoint](ui-ux-implementation.md#allocation-reports-and-remaining-ui-work--23-october-2026)
for final validation and outstanding device, Windows and release work. This does
not mark the entire second-pass proposal complete.
