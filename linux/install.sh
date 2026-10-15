#!/usr/bin/env bash
# Instala Claude Usage Tray en Omarchy: script en ~/.local/bin y modulo en Waybar.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bin="$HOME/.local/bin/claude-usage-tray"
wb="$HOME/.config/waybar"

mkdir -p "$HOME/.local/bin"
install -m 755 "$here/claude-usage-tray" "$bin"

for f in config.jsonc style.css; do
  [[ -f "$wb/$f.bak-claude-usage" ]] || cp "$wb/$f" "$wb/$f.bak-claude-usage"
done

python3 - "$wb/config.jsonc" "$bin" <<'PY'
import sys
path, bin_ = sys.argv[1:]
s = open(path).read()
if "custom/claude-usage" not in s:
    module = f'''  "custom/claude-usage": {{
    "exec": "{bin_} daemon",
    "return-type": "json",
    "format": "{{}}",
    "tooltip": true,
    "restart-interval": 10,
    "on-click": "{bin_} popup",
    "on-click-right": "{bin_} refresh"
  }},
'''
    assert '"group/tray-expander",' in s and '  "cpu": {' in s, "config.jsonc inesperado"
    s = s.replace('"bluetooth",', '"custom/claude-usage",\n    "bluetooth",', 1)
    s = s.replace('  "cpu": {', module + '  "cpu": {', 1)
    open(path, "w").write(s)
PY

if ! grep -q "custom-claude-usage" "$wb/style.css"; then
  cat >> "$wb/style.css" <<'CSS'

#custom-claude-usage {
  min-width: 12px;
  margin-right: 17px;
}

#custom-claude-usage.disconnected {
  opacity: 0.5;
}

#custom-claude-usage.nodata {
  opacity: 0.7;
}
CSS
fi

omarchy restart waybar
echo "Listo. Haz clic en el icono de Claude en la barra para vincular tu cuenta."
