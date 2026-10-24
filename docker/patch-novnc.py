#!/usr/bin/env python3
"""Expose measured render backlog and local viewport transforms in noVNC 1.6.

Fail closed when Debian changes the audited integration points. Keep original
MPL headers; noVNC's protocol, decoders and input coordinate mapping are retained.
"""
from pathlib import Path
import hashlib
import sys

AUDITED = {
    'display.js': '257694ea2ad5634fcf0528cdb91eb5a76b3e4b3820ec55c7fa691d89fe4fa277',
    'rfb.js': '563a8c8469dfb55d1deee05262a311637d0e6fe8f65ae34c8222ee22e34e8a93',
}


def patch(root):
    root = Path(root)
    display = root / 'core/display.js'
    rfb = root / 'core/rfb.js'
    changes = {
        display: [
            ('        this._renderQ = [];', '        this._wegaFrames = 0;\n        this._renderQ = [];'),
            ('    // ===== PUBLIC METHODS =====', '''    // Modified by Wega contributors, 2026: public, read-only queue measurements.
    get renderStats() {
        return {pending: this._renderQ.length,
                frames: this._renderQ.filter(action => action.type === 'flip').length,
                oldestMs: this._renderQ.length ? performance.now() - this._renderQ[0].wegaQueuedAt : 0,
                completed: this._wegaFrames};
    }
    get viewportMetrics() {
        return {...this._viewportLoc, scale: this._scale, width: this._fbWidth, height: this._fbHeight};
    }

    // ===== PUBLIC METHODS ====='''),
            ('        this._renderQ.push(action);', '        action.wegaQueuedAt = performance.now();\n        this._renderQ.push(action);'),
            ('                this._targetCtx.drawImage(this._backbuffer,', '                this._wegaFrames++;\n                this._targetCtx.drawImage(this._backbuffer,'),
        ],
        rfb: [('    get qualityLevel() {', '''    // Modified by Wega contributors, 2026: viewer integration without private-field access.
    get renderStats() { return this._display.renderStats; }
    get viewportMetrics() { return this._display.viewportMetrics; }
    setViewport(scale, x, y) {
        if (![scale, x, y].every(Number.isFinite) || scale <= 0 || scale > 8) return;
        this._scaleViewport = false;
        this._clipViewport = true;
        this._wegaViewportScale = scale;
        this._display.clipViewport = true;
        this._display.scale = scale;
        this._display.viewportChangeSize(this._screen.clientWidth / scale, this._screen.clientHeight / scale);
        const current = this._display.viewportMetrics;
        this._display.viewportChangePos(x - current.x, y - current.y);
    }

    get qualityLevel() {'''),
            ('        this._scaleViewport = scale;', '        this._wegaViewportScale = null;\n        this._scaleViewport = scale;'),
            ('    _updateClip() {', '''    _updateClip() {
        if (this._wegaViewportScale) {
            const m = this.viewportMetrics;
            this.setViewport(this._wegaViewportScale, m.x, m.y);
            return;
        }'''),
            ('    _updateScale() {', '''    _updateScale() {
        if (this._wegaViewportScale) {
            const m = this.viewportMetrics;
            this.setViewport(this._wegaViewportScale, m.x, m.y);
            return;
        }'''),
        ],
    }
    # Validate every anchor before writing either file; incompatible upgrades fail the image build.
    updated = {}
    for file, edits in changes.items():
        if hashlib.sha256(file.read_bytes()).hexdigest() != AUDITED[file.name]:
            raise ValueError(f'Unaudited noVNC source: {file.name}; review upstream changes before applying the patch')
        source = file.read_text()
        for old, new in edits:
            if source.count(old) != 1:
                raise ValueError(f'Unsupported noVNC source: {file.name}: {old!r}')
            source = source.replace(old, new, 1)
        updated[file] = source
    for file, source in updated.items():
        file.write_text(source)


if __name__ == '__main__':
    patch(sys.argv[1])
