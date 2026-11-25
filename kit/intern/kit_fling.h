/* Touch scrolling physics, a port of Android's: VelocityTracker (the LSQ2 strategy, a least squares
 * quadratic over the last 100 ms of touches) and OverScroller's spline fling (the curve and the
 * friction a ScrollView flings with). A window host feeds touches to a tracker and, when the finger
 * lifts, starts a fling from its velocity and reads the distance covered as time passes.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#ifndef KIT_FLING_H
#define KIT_FLING_H

#include <stdbool.h>

/* VelocityTracker.HISTORY_SIZE */
#define BK_VT_HISTORY 20

typedef struct BKVelocityTracker {
  double t[BK_VT_HISTORY]; /* ms, ascending */
  float x[BK_VT_HISTORY], y[BK_VT_HISTORY];
  int count; /* touches kept, oldest dropped when the ring is full */
} BKVelocityTracker;

void bk_vt_clear(BKVelocityTracker *vt);
/* A touch at (x, y) px at `t_ms` (the event's own time, not the time it was handled). */
void bk_vt_add(BKVelocityTracker *vt, double t_ms, float x, float y);
/* The velocity at `now_ms`, px/s. False when there is none: fewer than two touches within 100 ms,
 * or the finger rested for 40 ms (ASSUME_POINTER_STOPPED_TIME). */
bool bk_vt_velocity(const BKVelocityTracker *vt, double now_ms, float *r_vx, float *r_vy);

/* One axis of OverScroller.SplineOverScroller.fling(velocity): no bounds, the stretch is the
 * list's. `density` is dp to px. A velocity below the minimum or above the maximum fling velocity
 * (50 and 8000 dp/s) is not a fling or is clamped. */
typedef struct BKFling {
  float distance_px; /* signed: the whole distance, reached at duration_ms */
  float duration_ms;
} BKFling;

bool bk_fling_start(BKFling *f, float velocity, float density);
/* The distance covered `elapsed_ms` after the start (signed px); `*r_done` once it is all covered. */
float bk_fling_at(const BKFling *f, double elapsed_ms, bool *r_done);

#endif
