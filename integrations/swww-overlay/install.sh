#!/usr/bin/env bash
#
# cozy — swww-overlay integration (cozy rendering *alongside* swww/hyprpaper).
#
# This integration runs cozy in --overlay mode: a transparent rain layer above
# the wallpaper drawn by your existing swww/hyprpaper daemon, with droplets
# refracting a synced copy of the same image (Plan A).
#
# STATUS: not yet available — ships in Phase 2 (the --overlay render mode and
# the dual-daemon cozy-wall are not implemented yet). This stub keeps the
# integrations tree complete and honest.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
# shellcheck source=../common.sh
[ -r "$SCRIPT_DIR/../common.sh" ] && . "$SCRIPT_DIR/../common.sh"

if command -v warn >/dev/null 2>&1; then :; else
    warn() { printf '!  %s\n' "$*"; }
    info() { printf '   %s\n' "$*"; }
fi

warn "The swww-overlay integration is not available yet (Phase 2)."
info ""
info "cozy can't yet render *alongside* swww/hyprpaper — the --overlay mode that"
info "makes the rain transparent over an external wallpaper is still in design."
info ""
info "For now, to run cozy on plain Hyprland (cozy owns the wallpaper), use:"
info "    COZY_INTEGRATION=standalone curl -fsSL \\"
info "      https://raw.githubusercontent.com/lubabs770/cozy/main/install.sh | bash"
info ""
info "Design / progress:"
info "    docs/superpowers/specs/2026-06-18-cozy-multi-integration-monorepo-design.md"
exit 1
