# Claude Usage Tray — Omarchy / Waybar

Port nativo de la [app de Windows](../README.md)
para Omarchy: uso real de tu plan Claude Pro/Max (ventana de 5 h y semanal) en la topbar,
con un panel GTK4 (layer-shell) que usa los colores y la fuente del tema activo.

## Instalar

    ./install.sh

Copia `claude-usage-tray` a `~/.local/bin`, añade el módulo `custom/claude-usage` a
`~/.config/waybar/config.jsonc` + estilos, y reinicia Waybar (respaldos: `*.bak-claude-usage`).

## Usar

- Icono en la barra: `✱ 24%` (color según uso). Clic: panel · Clic derecho: re-verificar.
- Panel → **Vincular mi cuenta de Claude**: abre el login oficial (`claude auth login`) en una
  terminal flotante + tu navegador. Al terminar, la cuenta aparece sola con su uso.
- Dos cuentas (Personal / Trabajo), cada una en un perfil aislado de Claude Code:
  `~/.local/share/claude-usage-tray/accounts/<1|2>/claude`.
- Usar Claude Code con una cuenta vinculada: `claude-usage-tray run 1` (o `2`).
- Notificaciones (mako) al 80 / 95 / 100 % de la ventana de 5 h.

## Cómo obtiene los datos (sin tocar credenciales)

Igual que el original: nunca lee ni guarda tokens. El login lo hace el CLI oficial; el uso viene del
`statusLine` de Claude Code (se configura solo en el perfil aislado) o de una consulta mínima
(`claude -p` con haiku, 1 turno) cuando no hay datos recientes. Esa consulta automática se puede
apagar con el interruptor del panel (o `"probe_enabled": false` en `~/.local/share/claude-usage-tray/settings.json`).
Si un dato no se conoce, la UI dice "No disponible".

## Comandos

`daemon` · `popup` · `connect N` · `disconnect N` · `probe N` · `refresh` · `status` · `run N`
