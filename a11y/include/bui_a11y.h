/* bui-a11y's C ABI (a11y/src/ffi.rs): the platform accessibility adapter of a native window, fed
 * the kit's flat trees (BKA11yNode, BUI_kit.h) and turning them into AccessKit updates. Only the
 * kit calls it (kit/intern/a11y and the window hosts).
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */
#pragma once

#include <stdint.h>

#include "BUI_kit.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct BuiA11y BuiA11y;

typedef struct BuiA11yHost {
  void *data;
  /* The whole tree now (a reader attached): the nodes, their count and the focused id, valid
   * until the next call. Returns 0 when it has none. Called on the window's thread. */
  int (*tree)(void *data, const BKA11yNode **r_nodes, int *r_count, uint64_t *r_focus);
  /* A request waits for bui_a11y_take. Called from any thread. */
  void (*wake)(void *data);
} BuiA11yHost;

/* bui_a11y_validate's results: 0 valid, else why not. */
enum {
  BUI_A11Y_INVALID_ARRAY = 1,
  BUI_A11Y_INVALID_ID,
  BUI_A11Y_INVALID_PARENT,
  BUI_A11Y_INVALID_ROLE,
  BUI_A11Y_INVALID_FLAGS,
  BUI_A11Y_INVALID_UTF8,
  BUI_A11Y_INVALID_WIDGET,
};
/* `subtree`: a host's subtree (any number of top nodes, widgets allowed), else a whole tree (its
 * root first, no widgets). */
int bui_a11y_validate(const BKA11yNode *nodes, int n, int subtree);

/* The adapter of a native window (HWND), NULL where this platform has none yet. */
BuiA11y *bui_a11y_new(void *window, const BuiA11yHost *host);
void bui_a11y_free(BuiA11y *a);
/* 1 once a reader asked for the tree: then send it after each change. */
int bui_a11y_active(const BuiA11y *a);
/* The tree as it is now (only what changed goes out): 0, or a BUI_A11Y_INVALID_* code. */
int bui_a11y_update(BuiA11y *a, const BKA11yNode *nodes, int n, uint64_t focus);
void bui_a11y_window_focus(BuiA11y *a, int focused);
/* The next request (its text valid until the next call): 1, or 0 when none waits. */
int bui_a11y_take(BuiA11y *a, BKA11yRequest *r_req);
#ifdef _WIN32
/* WM_GETOBJECT: 1 and *r_result when the adapter answers it, else 0 (DefWindowProc). */
int bui_a11y_wm_getobject(BuiA11y *a, uintptr_t wparam, intptr_t lparam, intptr_t *r_result);
#endif

#ifdef __cplusplus
}
#endif
