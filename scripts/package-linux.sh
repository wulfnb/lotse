#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "$0")/.." && pwd)"
cd "$project_dir"

dist_dir="$project_dir/dist/lotse-linux-x86_64"
binary="${1:-$project_dir/target/release/servo-browser}"

if [[ ! -f "$binary" ]]; then
  echo "Binary not found at $binary. Pass path to executable as first argument." >&2
  exit 1
fi

rm -rf "$dist_dir"
mkdir -p "$dist_dir/bin" "$dist_dir/share/applications" "$dist_dir/share/icons/hicolor/256x256/apps"

cp "$binary" "$dist_dir/bin/servo-browser"
chmod 755 "$dist_dir/bin/servo-browser"

if [[ -f "$project_dir/assets/icons/browser-icon-256.png" ]]; then
  cp "$project_dir/assets/icons/browser-icon-256.png" "$dist_dir/share/icons/hicolor/256x256/apps/lotse.png"
fi

if [[ -f "$project_dir/LICENSE" ]]; then
  cp "$project_dir/LICENSE" "$dist_dir/"
fi

cat > "$dist_dir/bin/lotse" <<'EOF'
#!/usr/bin/env sh
set -eu
app_dir="$(cd "$(dirname "$0")/.." && pwd)"
exec "$app_dir/bin/servo-browser" "$@"
EOF
chmod 755 "$dist_dir/bin/lotse"

cat > "$dist_dir/share/applications/lotse.desktop" <<'EOF'
[Desktop Entry]
Name=Lotse
Comment=Lotse Native Rust Browser powered by Servo
Exec=lotse %U
Icon=lotse
Terminal=false
Type=Application
Categories=Network;WebBrowser;
MimeType=text/html;text/xml;application/xhtml+xml;x-scheme-handler/http;x-scheme-handler/https;
EOF

tar -czf "$project_dir/dist/Lotse-Linux-x86_64.tar.gz" -C "$project_dir/dist" lotse-linux-x86_64
echo "Packaged $project_dir/dist/Lotse-Linux-x86_64.tar.gz"

