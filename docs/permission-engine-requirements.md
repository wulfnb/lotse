# Permission grants and revocation: pinned-engine requirements

Persistent automatic grants and live revocation remain blocked in Servo 0.7.0.
The current implementation deliberately retains document-scoped grants and exact
top-level-origin Block/Ask controls. Block resolves matching queued responders
and future embedder prompts; it cannot invalidate a grant already cached by a
document. The UI now states that limit and labels approval “Allow for this page”.

## Confirmed source limitations

- [`PermissionRequest`](../vendor/servo/webview_delegate.rs) has a feature and an
  allow/deny responder, but no requesting origin, pipeline or document identity.
  [`PromptPermission`](../vendor/servo/servo.rs) receives only webview ID, feature
  and responder. The browser can identify the top-level URL, not the requesting
  iframe. Treating that URL as the requesting origin could automatically grant
  a third-party frame a remembered first-party permission.
- The pinned script implementation caches decisions in the requesting
  `GlobalScope.permission_state_invocation_results`. Its `permission_revoke`
  function is empty. See the exact published source at
  [permissions.rs](https://github.com/servo/servo/blob/aac43a3f31a259f04a574f5ec4e959c943ad7cc7/components/script/dom/permission/permissions.rs).
- The pinned [`MediaStreamTrack` implementation](https://github.com/servo/servo/blob/aac43a3f31a259f04a574f5ec4e959c943ad7cc7/components/script/dom/media/mediastreamtrack.rs)
  implements kind, ID and clone; it has no stop operation. The embedder WebView
  API also has no origin-scoped device-stop/revoke operation. Page JavaScript
  cannot safely provide browser-enforced revocation, especially across frames.

## Required engine changes

1. Carry the trusted requesting origin plus pipeline/document identity through
   every permission prompt, including wake locks and worker requests. Reject
   opaque origins and stale document identities before applying persisted grants.
2. Add an acknowledged embedder revocation command scoped to origin and feature.
   It must invalidate script-side cached permission state, update live
   `PermissionStatus` objects and emit change events in every affected global.
3. Stop camera/microphone sources and derived/cloned tracks, release wake locks,
   and revoke other feature-specific live capabilities. Return success only
   after the capability has actually ended; navigation or JavaScript patching is
   not an adequate substitute.
4. Only then add profile-backed Always allow and a Stop access action. Clearing
   site permissions must delete stored grants and call the acknowledged revoke
   path. A failed revoke must remain visible and cannot be reported as stopped.

## Acceptance checks before enabling these controls

- Same-origin and cross-origin iframes, workers, opaque/sandboxed origins, and
  navigation while a prompt is queued must preserve attribution and isolation.
- Profile reload must retain only intended origin/feature grants; browsing-data
  clearing must remove them without affecting other sites.
- Camera and microphone indicators, original and cloned tracks, and permission
  state/change events must prove that revocation ends active access. Re-request
  behavior must follow the new policy without a stale cached grant.

These are engine work items, not implemented capabilities. No Always allow or
Stop access control is exposed until the above contract can be verified.
