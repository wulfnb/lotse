# Windows installer

The portable archive remains supported. The per-user installer adds a Start menu
shortcut, an uninstall entry and browser capabilities for HTTP, HTTPS, HTM and HTML.
It does not set defaults silently; use Settings → Browsing → Open default browser
settings after installing. Opening URLs while another Wega instance already owns
the profile still needs a single-instance handoff implementation.

From the repository root on Windows with Inno Setup 6.3 or newer installed:

```powershell
ISCC.exe packaging\windows\wega.iss
scripts\windows-installer-check.ps1
```

Prepare `dist\wega-windows-x64` using the release packaging steps first. The
compiler writes `dist\Wega-Windows-x64-Setup.exe`. The CI check uses disposable
installation files and refuses to overwrite an existing Wega registration.
It does not delete browser profile data or change the user's default browser.

These installer steps have not been executed on the development Mac. Windows CI
must compile and pass install/uninstall checks before this is considered a tested
release artifact. Executable/installer signing requires a release signing identity
and is not configured. Do not describe the installer as signed.

Registration follows [Microsoft's capability registration documentation](https://learn.microsoft.com/en-us/windows/win32/shell/default-programs).
Per-user setup and removal use the [Inno privilege](https://jrsoftware.org/ishelp/topic_setup_privilegesrequired.htm)
and [registry flags](https://jrsoftware.org/ishelp/topic_registrysection.htm) documentation.
