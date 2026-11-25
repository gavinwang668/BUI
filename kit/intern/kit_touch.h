/* Touch screens (BK_touch): the state machine that turns a host's contacts into the pointer,
 * scrolls, flings and gestures the widgets know, the same on every platform. See kit_touch.c.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#ifndef KIT_TOUCH_H
#define KIT_TOUCH_H

#include <stdbool.h>

#include "kit_fling.h"

typedef struct BKNode BKNode;
typedef struct BKContext BKContext;

#define BK_MAX_TOUCH 10 /* contacts followed at once; more are ignored until one lifts */

/* What one contact is (BKContact.role). */
typedef enum {
  BK_CONTACT_FREE = 0, /* the slot is unused */
  BK_CONTACT_LEAD,     /* the finger of the one-finger path (BKTouch.state says which) */
  BK_CONTACT_PAIR,     /* one of the gesture's two fingers */
  BK_CONTACT_RAW,      /* a BK_TOUCH_MODE_RAW custom node's (BKContact.raw) */
  BK_CONTACT_IDLE,     /* followed until it lifts, used for nothing (a third finger, a cancel) */
} BKContactRole;

typedef struct BKContact {
  int id;      /* the host's */
  BKContactRole role;
  float x, y;  /* its last position, window pixels */
  BKNode *raw; /* BK_CONTACT_RAW: the custom node it belongs to */
} BKContact;

/* What the fingers that are not raw do together. */
typedef enum {
  BK_TOUCH_IDLE = 0, /* none down */
  BK_TOUCH_TAP,      /* one finger, not moved: a click if it lifts, the context menu if held */
  BK_TOUCH_PRESS,    /* one finger on a widget that drags: the pointer, pressed */
  BK_TOUCH_SCROLL,   /* one finger scrolls the content */
  BK_TOUCH_PINCH,    /* two fingers: pan, magnify, rotate */
  BK_TOUCH_SPENT,    /* a gesture or a long press ended: nothing until every finger lifted */
} BKTouchState;

typedef struct BKTouch {
  BKContact c[BK_MAX_TOUCH];
  BKTouchState state;
  int lead;           /* the one-finger path's contact (TAP, PRESS, SCROLL), else -1 */
  int pair[2];        /* PINCH: its contacts, else -1 */
  float down_x, down_y; /* where the lead went down (the slop and the long press) */
  float last_x, last_y; /* the scroll's last point: the lead's, or the pair's centre */
  int anchor_x, anchor_y; /* what scrolls is under here: where the scroll began */
  float dist, angle;  /* PINCH: the fingers' spread and angle at the last update */
  unsigned moved;     /* PINCH: bit 0 / 1, that finger moved since the last update */
  BKNode *pinch_target; /* PINCH: magnify / rotate go to it (NULL: under the centre) */
  double long_press_at; /* TAP: bk_time() the context menu opens at, 0 none */
  BKVelocityTracker vt; /* SCROLL: the lead's touches */
  /* the fling after a one-finger scroll (Android's OverScroller, per axis) */
  BKFling fling_x, fling_y;
  float fling_done_x, fling_done_y;
  double fling_t0; /* bk_time() it began, 0: no fling */
} BKTouch;

void bk_touch_init(BKTouch *t);
/* The nodes of `subtree` are going: contacts and the pinch let go of them. */
void bk_touch_forget(BKTouch *t, const BKNode *subtree);
/* Every contact down is cancelled: nothing they did is kept (BK_cancel, the window lost focus). */
void bk_touch_cancel_all(BKContext *ctx);
/* The context's frame timer (bk_ctx_arm_timer): when the long press is due (bk_time), 0 none;
 * and its tick, which opens the context menu when it is. */
double bk_touch_due(const BKContext *ctx);
void bk_touch_tick(BKContext *ctx);
/* BK_render, before the layout: the fling's step. True while one runs (BK_animating). */
void bk_touch_frame(BKContext *ctx);
bool bk_touch_animating(const BKContext *ctx);

#endif
