# Licensing and release notices

Wega is licensed under the [Mozilla Public License 2.0](../LICENSE).
[NOTICE](../NOTICE) holds the copyright line, the Servo trademark notice and the
warranty/liability terms (§ 521 BGB).

## What every download contains

| Download | Location of the notices |
| --- | --- |
| macOS `Wega.app` (DMG/ZIP) | `Wega.app/Contents/Resources/` |
| Linux `Wega-Linux-x86_64.tar.gz` | archive root (`wega-linux-x86_64/`) |
| Windows `Wega-Windows-x64.zip` | archive root |
| Docker image | generated in the builder stage by the same script |

Each location has `LICENSE`, `NOTICE` and `THIRD-PARTY-NOTICES.html`. The shipped
`NOTICE` ends with the source commit the binary was built from and a link to that
exact tree. The commit is taken from `WEGA_SOURCE_COMMIT`, then `GITHUB_SHA`,
then `git rev-parse HEAD`; a build from an uncommitted working tree records
`HEAD`, not the local changes.

## Generating the notices

```sh
cargo install cargo-about --version 0.9.2 --features cli --locked
scripts/third-party-notices.sh OUT_DIR [cargo-about generate options...]
# Examples:
scripts/third-party-notices.sh target/notices --features media
scripts/third-party-notices.sh dist/win --no-default-features --features "desktop brotli-streams" --target x86_64-pc-windows-msvc
```

cargo-about 0.9 builds its executable only with `--features cli`. The script needs
network access the first time, because `cargo metadata` resolves every target's
crates. It runs `cargo about generate --fail`, so an unknown or unaccepted license
fails the release instead of being omitted.

`about.toml` lists the accepted SPDX licenses. Each one occurs in the dependency
graph for macOS, Linux or Windows. For `A OR B` crates cargo-about picks the first
accepted choice, so permissive licenses come first. `GPL-2.0-only` is not accepted;
its one occurrence (`self_cell`) is dual-licensed and used under Apache-2.0.
Without `--target`, the notices cover every platform's dependencies, which is
deliberately a superset.

### Bundled fonts

egui's `epaint_default_fonts` crate compiles four fonts into every desktop
binary. Its license expression omits the Hack and emoji-icon licenses, and the crate
ships no MIT/Apache text for its own code. `about.toml` therefore clarifies the
crate from the font license files it ships, pinned by SHA-256:

| Font | License | File in the crate |
| --- | --- | --- |
| Ubuntu Light | Ubuntu Font Licence 1.0 | `fonts/UFL.txt` |
| Noto Emoji | SIL Open Font License 1.1 | `fonts/OFL.txt` |
| Hack | MIT, plus Bitstream Vera License for Vera-derived glyphs | `fonts/Hack-Regular.txt` |
| emoji-icon-font / egui-icons | MIT | `fonts/emoji-icon-font-mit-license.txt` |

`about.hbs` also names the fonts in a dedicated section. When egui is upgraded,
cargo-about fails on a checksum mismatch; re-check the files and update the
checksums.

## macOS media libraries

The macOS app bundles GStreamer plugins and their dependency closure from
Homebrew (`scripts/bundle-macos-media.py`). Most are LGPL (GStreamer, GLib, GnuTLS,
libnice, ...), others BSD/MIT/Apache. The bundler writes
`Contents/Resources/media-libraries.json` (file, source path, Homebrew formula and
version) and `Contents/Resources/MEDIA-LIBRARIES.txt`. The text file gives each
library's Homebrew formula and version, its license (from `brew info`), project
page, formula recipe and source archive URL. It also contains a written offer to
provide the corresponding source for at least three years. The libraries stay
separate, replaceable dylibs, as the LGPL requires.

No GPL or patent-encumbered codecs are bundled. `gstlibav` was removed from the
plugin list (see [vendor/README.md](../vendor/README.md)), and macOS decodes
H.264/HEVC/AAC through Apple's VideoToolbox/AudioToolbox via `gstapplemedia`. The
bundler refuses to continue if any `libx264*`, `libx265*`, `libav*`, `libswscale*`,
`libswresample*`, `libpostproc*`, `libfaac*` or `libgstlibav*` library would be
copied. `tests/test_release_tools.py` checks the guard and the shipped lists.

## Release provenance

The CD workflow attests build provenance for the release archives (through
`SHA256SUMS.txt`) and for the GHCR image. Verify with:

```sh
gh attestation verify Wega-macOS-arm64.dmg --repo wulfnb/wega
gh attestation verify oci://ghcr.io/wulfnb/wega:<commit> --repo wulfnb/wega
```

Pushes to `main` update the moving `test-preview` prerelease. They also create an
immutable `test-preview-<12-char commit>` prerelease with the same files.
