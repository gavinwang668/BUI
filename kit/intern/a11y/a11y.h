/* Accessibility, the kit's side (BUI_kit.h "accessibility"): each context's tree, built from its
 * nodes and the subtrees hosts attach (a11y.c), served to the platform by the window hosts through
 * bui-a11y (a11y/include/bui_a11y.h). Without the "a11y" feature (BUI_A11Y off) a11y_off.c keeps
 * the API and builds nothing.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */
#pragma once

#include <stdbool.h>

#include "BUI_kit.h"

typedef struct BuiA11y BuiA11y;

/* The node is freed / the context is freed / `subtree` leaves the context (kit_core.c). */
void bk_a11y_node_free(BKNode *n);
void bk_a11y_ctx_free(BKContext *ctx);
void bk_a11y_forget(BKContext *ctx, const BKNode *subtree);

/* -- the window hosts (kit_window_*.c) --------------------------------------------------------
 * A window's adapter: made with the window (`native` is its HWND ...), it starts when a reader
 * first asks for the tree. `wake(data)` runs on any thread when a request waits: the host then
 * calls bk_a11y_serve on the window's thread. NULL without an adapter for this platform. */
BuiA11y *bk_a11y_open(BKContext *ctx, void *native, void (*wake)(void *data), void *data);
/* The window's title: the name of its tree's root. */
void bk_a11y_title(BKContext *ctx, const char *title);
/* After a render (`drawn`: it drew something): while a reader reads the window, the tree goes out
 * when it may have changed. */
void bk_a11y_sync(BKContext *ctx, BuiA11y *a, bool drawn);
/* The requests waiting: performed now (BK_a11y_action). */
void bk_a11y_serve(BKContext *ctx, BuiA11y *a);
