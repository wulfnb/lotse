# Run Lotse Browser in Docker

This image runs **our native Rust/Servo browser and its existing tabs, menus and
DevTools** on Linux. It does not install Firefox or use a LinuxServer image,
Selkies, XFCE, a video encoder, or a desktop control panel. A small project-owned
web page uses noVNC to forward display changes and keyboard/mouse input. Openbox
only manages the native browser and detached DevTools windows.

## Start

```sh
cp .env.docker.example .env
# Edit .env: set SERVO_WEB_USER, a unique SERVO_WEB_PASSWORD and SERVO_DOWNLOAD_DIR.
docker compose up -d --build
```

Open **https://localhost:7443** and accept the locally generated certificate.
The certificate and browser profile persist in the `servo-profile` named volume.
To use a trusted certificate, replace `/config/tls/server.crt` and
`/config/tls/server.key` in that volume and restart the service.

The first build compiles a web engine and needs considerable time and disk space.
The default builds an optimized release binary. Native Linux ARM64 and AMD64 are
selected by Docker; cross-architecture emulation makes this large build slower.
Rust is confined to the build stage. BuildKit can reuse Cargo registry and compilation caches after successful
builds. They are not copied into the runtime image, and interrupted builds or
cache eviction may require recompilation.

Optional settings can be placed in `.env` (see `.env.docker.example`):

```sh
SERVO_PORT=7443
SERVO_BIND_ADDRESS=127.0.0.1
SERVO_WEB_USER=servo
SERVO_WEB_PASSWORD='your-unique-password-here'
SERVO_DOWNLOAD_DIR=./downloads
SERVO_RESOLUTION=1440x900
SERVO_START_URL=https://example.com/
SERVO_BUILD_PROFILE=release
SERVO_BUILD_JOBS=2
```

Use `SERVO_BUILD_JOBS=1` on memory-constrained builders. For a development build:

```sh
SERVO_BUILD_PROFILE=dev docker compose up -d --build
```

`SERVO_START_URL` sets the first-run page. Later starts restore the profile’s
saved tabs.

The viewer scales the fixed remote resolution to fit the client. Smaller values,
for example `1280x800`, reduce display/rendering work. Allowed dimensions are
800–3840 pixels wide and 600–2160 pixels high. Change the environment setting and
recreate the container to change resolution. The viewer offers full screen,
reconnect, and a text clipboard dialog; the rest of the UI is our native browser.
The remote browser uses Linux Ctrl shortcuts. Your client browser may intercept
combinations such as Ctrl+L, Ctrl+T, or F12; use the remote browser’s address bar
and menu controls when that happens.

## Profile, logs and diagnostics

```sh
docker compose logs -f servo-browser
docker compose exec servo-browser supervisorctl -c /etc/servo-supervisord.conf status
docker stats --no-stream
# Stop while preserving the profile:
docker compose down
```

The browser runs as UID/GID 1000, with its profile at `/config/browser` and a
Downloads directory at `/config/Downloads`, mounted from `SERVO_DOWNLOAD_DIR` on
the host (default `./downloads`). Changing that variable selects a different host
folder; existing files stay in the previous folder. All display, viewer and application
workers run as that unprivileged user. The small supervisor stays root to manage
workers and forward their logs after initial volume/certificate setup.
No host Docker socket or privileged mode is used. Closing the browser restarts it
inside the same container; use Compose to stop the session.

The runtime has six supervised programs: display, minimal window manager, VNC,
authentication gateway with its WebSocket bridge, HTTPS server, and Lotse Browser. Health checks require all six, the HTTPS endpoint, and the authentication gateway
to respond. Internal VNC/WebSocket sockets bind to loopback
inside the container; only HTTPS is published, on host loopback by default.

Authentication is required. On first start, set `SERVO_WEB_USER` and
`SERVO_WEB_PASSWORD` in `.env`; Compose passes them as `BROWSER_USER` and
`BROWSER_PASSWORD`. The username defaults to `servo`; there is no default password.
Startup refuses missing or placeholder credentials. Passwords contain 12–128 UTF-8
bytes without line breaks. Single-quote values in `.env` to preserve literal `$`,
`#` and spaces. Keep `.env` private (`chmod 600 .env`).

Sign in using the web login page. Open **Account** in the viewer toolbar to change
the username and password; enter the current password and confirm the new one.
The new account takes effect immediately and persists across restarts. Other
sessions and existing browser-control connections are revoked. **Sign out** signs
out all viewers of this shared account. Login sessions last up to 12 hours and use
Secure, HttpOnly, SameSite=Strict cookies. Password changes require a same-origin
request and a session CSRF token. Failed authentication attempts are rate-limited.

The account username and salted password hash are stored inside an encrypted
`/config/auth/account.enc` file. Passwords use scrypt with N=2^17, r=8, p=1 and a
random 16-byte salt, matching [OWASP's scrypt parameters](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html#scrypt).
[PyCA Fernet](https://cryptography.io/en/stable/fernet/) provides authenticated
account encryption. Its random key is a separate `/config/auth/key` file; both
files are mode 0600 inside a mode 0700 directory. Protect profile backups: someone
who obtains both files can decrypt the username and attempt to crack the password
hash. The changed password itself is never stored reversibly.

After the first successful start, encrypt the initial `.env` username/password:

```sh
python3 scripts/web-credentials.py encrypt-env
docker compose up -d --no-build
```

This replaces both values with `enc:v1:…` ciphertext. The key stays in the profile
volume, outside `.env` and the image. Worker processes do not inherit plaintext
bootstrap credentials. To view your **initial** login locally, deliberately run
`python3 scripts/web-credentials.py show`. This cannot recover a password changed
in Account. Encrypted bootstrap values require the original profile's key; keep
that volume when recreating the container. `.env` values initialize a new account
only; they do not overwrite changes made in Account on later starts.

The same login protects the viewer, its assets, the WebSocket control endpoint,
download listings and files. Only the login page, its styles/script and the small
health response are public. All viewers share one browser session and profile;
these are not separate user accounts. Credentials and session cookies travel over
HTTPS. The username is visible in the signed-in Account dialog, not in storage.

Alternatively, mount a password file and set `BROWSER_PASSWORD_FILE` to its
container path. It takes precedence over the environment password on the first start. It may contain a plain bootstrap password or its `enc:v1:…` encrypted value. Example Compose override:

```yaml
services:
  servo-browser:
    environment:
      BROWSER_PASSWORD_FILE: /run/secrets/browser-password
    secrets:
      - browser-password
secrets:
  browser-password:
    file: /path/to/browser-password.txt
```

The health URL remains unauthenticated and returns only `ok`. The control
WebSocket accepts only requests from the viewer’s own HTTPS origin. Keep the default
loopback publication for local use. Set `SERVO_BIND_ADDRESS=0.0.0.0` to listen on
all host interfaces for remote access, and install a trusted certificate for that
hostname before using it over the internet. Only HTTPS port 7443 is exposed.

## What differs from the desktop app

Rendering uses Linux OpenGL/Mesa. There is no macOS vibrancy; the browser uses its
opaque theme variants. Without a passed-through GPU, Mesa renders on the CPU.
noVNC sends changed screen regions rather than encoding a continuous video stream.
The VNC server polls Xvfb for changes because OpenGL updates can be missed by
XDamage tracking; this also allows a newly connected viewer to see an idle page.
This is intended for interactive browsing, not high-frame-rate video streaming.

Media support remains compiled into Servo, but this lightweight viewer does not
transport audio, microphones, webcams or file uploads. It supports text clipboard
transfer. Files can be copied using `docker compose cp` or a mounted directory.
Completed files are also available through **Downloads** in the viewer toolbar.
Start transfers with **↓** in the native browser: enter an HTTP(S) URL, use
**Download link/image…** from a page's context menu, or click an HTTP(S)
`<a download>` link. A native confirmation window lets you choose a file name and
1–8 connections (default four). Network request context menus offer downloads too.

Multipart downloads require `Accept-Ranges: bytes`, a known size and a strong ETag
or Last-Modified validator. Every response range and length is checked; invalid
ranges trigger a fresh single-connection transfer. Pause/resume retains completed
2 MiB segments; unfinished segments restart. If the server changes its validator,
old segments are discarded. Completed files persist in `/config/Downloads` and
are published atomically without overwriting existing names. The web viewer lists
only completed files; its authenticated HTTPS file endpoint supports byte ranges.

The downloader reuses site cookies for the original host, verifies TLS certificates,
and supports HTTP(S) GET files. Browser-held HTTP authentication, POST exports,
blob/data URLs and cross-host cookie propagation are not supported. For ordinary
links without a `download` attribute, use the context-menu download action.
Active download history/partials do not resume across browser restarts; completed
files remain. At most three jobs run at once. Downloads follow normal HTTP redirects.
Pages and DevTools retain the same Servo engine limitations as the native app.
Actual memory/speed depend on the sites opened and software rendering; reducing
desktop services alone does not guarantee lower total RAM than Firefox.

## Image without Compose

```sh
docker build -t servo-browser:local .
docker run -d --name servo-browser \
  -p 127.0.0.1:7443:7443 --shm-size=512m \
  -e BROWSER_USER=servo -e BROWSER_PASSWORD \
  -v servo-profile:/config -v /absolute/path/to/downloads:/config/Downloads \
  servo-browser:local
```

Export `BROWSER_PASSWORD` in your shell before the `docker run` command. For direct
Docker use, credentials are named `BROWSER_USER`/`BROWSER_PASSWORD`; the `SERVO_*`
settings above are the Compose interface. Ensure a custom download folder is
writable by container UID/GID 1000 on Linux.

Run `python3 scripts/docker-auth-test.py` after startup to verify authentication,
the control connection, and the configured download mount using the initial login.
After changing the account, use the UI with the new credentials.
`scripts/docker-account-test.sh` tests credential changes, encryption, CSRF, live
connection revocation, persistence and rate limiting in an isolated temporary account.

The Docker shared-memory limit is capacity, not preallocated application RAM.
The build-time `BUILD_JOBS` argument controls compilation concurrency only.

The browser's bottom status bar shows live CPU and RAM in small text next to
the zoom controls. In this image it reads cgroup v2 totals for the whole container,
including display/viewer services, and excludes inactive file cache from RAM as
`docker stats` does. It samples every two seconds; CPU 100% represents one core.
Hover over the readout for scope and browser uptime. If container counters are
unavailable, it shows browser-process readings and identifies that scope instead.

Native test scripts below require a test image built with `BUILD_FEATURES=test-harness`; production images exclude these drivers. Prefer the [dedicated isolated quality environment](testing.md#docker-verification).

After startup, `scripts/docker-smoke.sh` checks runtime libraries, viewer assets
and the browser’s native interaction smoke test on an isolated X11 display.
The script copies the small test fixtures into the running container; production
images do not carry test pages or source code.

`scripts/docker-download-test.sh` runs a real download-link test with an HttpOnly
cookie, four parallel connections and byte-for-byte verification on an isolated
display/profile. It does not write into the user's completed-download directory.

Run `scripts/docker-toolbox-test.sh` for the full developer-tools integration
suite. It starts a local HTTP fixture and a separate display/profile inside the
container, checks real requests, debugging, storage, CSS editing and docking,
then stops its fixture server and display. Run these test scripts one at a time.
Linux ARM64 has passed both suites; AMD64 has not been runtime verified here.

`scripts/docker-idle-test.sh` checks an isolated two-tab session with live resource
counters enabled. It catches the software-renderer CPU loop caused by scheduling
foreground redraws for hidden offscreen tabs. Run it separately from the other tests.

On 20 October 2026, the ARM64 container on an Apple Silicon Docker host used
about 239 MiB and 1.3% CPU at idle with one Example Domain tab, a connected viewer,
1440×900 resolution and software Mesa rendering. This includes all six services,
not just the browser process. Both Linux integration suites, remote menu input,
static-frame delivery and session restoration after restart were verified.
These are measurements for that workload, not limits or a Firefox benchmark.
