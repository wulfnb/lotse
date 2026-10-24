// Quality follows real decode/paint backlog; idle time never upgrades quality.
export class QualityController {
  constructor() { this.reset(false); }
  reset(slow) { this.level = slow ? 2 : 6; this.bad = 0; this.good = 0; this.completed = 0; }
  sample(stats, slow) {
    if (!stats || !Number.isFinite(stats.oldestMs)) return slow ? Math.min(2, this.level) : this.level;
    const congested = stats.oldestMs >= 180 || stats.frames >= 2 || stats.pending >= 24;
    if (congested) {
      this.good = 0;
      if (++this.bad >= 2) { this.level = 2; this.bad = 0; }
    } else {
      this.bad = 0;
      if (stats.pending === 0 && stats.completed > this.completed) {
        if (++this.good >= 10) { this.level = this.level === 2 ? 6 : 9; this.good = 0; }
      } else { this.good = 0; }
    }
    this.completed = stats.completed;
    return slow ? Math.min(2, this.level) : this.level;
  }
}

// RFB owns scaling and pointer conversion. These gestures change its local
// viewport only, never remote page zoom or the remote desktop resolution.
export class ViewportController {
  constructor(client, screen) { this.client = client; this.screen = screen; this.active = false; this.points = []; }
  enable(active) {
    this.active = active; this.points = [];
    this.client.viewOnly = active;
    if (active) this.client.resizeSession = false;
    this.screen.classList.toggle('viewer-panning', active);
  }
  reset() {
    const m = this.client.viewportMetrics;
    if (!m?.width || !m.height) return;
    const scale = Math.min(this.screen.clientWidth / m.width, this.screen.clientHeight / m.height, 8);
    this.client.setViewport(scale, 0, 0);
  }
  start(points) { this.points = points; }
  move(points) {
    const before = this.points;
    this.points = points;
    const m = this.client.viewportMetrics;
    if (!m?.scale || before.length !== points.length || !points.length) return;
    const center = list => ({x: list.reduce((n,p)=>n+p.x,0)/list.length, y:list.reduce((n,p)=>n+p.y,0)/list.length});
    const a = center(before), b = center(points);
    let scale = m.scale;
    if (points.length === 2) {
      const distance = list => Math.hypot(list[0].x-list[1].x, list[0].y-list[1].y);
      const initial = distance(before);
      if (initial > 0) {
        const fit = Math.min(this.screen.clientWidth/m.width, this.screen.clientHeight/m.height, 8);
        scale = Math.min(8, Math.max(fit, Math.min(fit*4, m.scale*distance(points)/initial)));
      }
    }
    // Keep the point under the gesture's centroid anchored while zooming.
    this.client.setViewport(scale, m.x+a.x/m.scale-b.x/scale, m.y+a.y/m.scale-b.y/scale);
  }
}
