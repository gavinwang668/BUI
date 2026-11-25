/* Accessibility without the "a11y" feature (BUI_A11Y off): the API stays and reports it missing;
 * no tree is built and no adapter made.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <stddef.h>

#include "a11y.h"
#include "kit_intern.h"

int BK_a11y_supported(void)
{
  return 0;
}

int BK_a11y_wanted(const BKNode *node)
{
  (void)node;
  return 0;
}

int BK_a11y_set_subtree(BKNode *node, const BKA11yNode *nodes, int n)
{
  (void)node;
  (void)nodes;
  (void)n;
  return 0;
}

int BK_a11y_tree(BKContext *ctx, const char *title, const BKA11yNode **r_nodes, uint64_t *r_focus)
{
  (void)ctx;
  (void)title;
  if (r_nodes) {
    *r_nodes = NULL;
  }
  if (r_focus) {
    *r_focus = 0;
  }
  return 0;
}

int BK_a11y_action(BKContext *ctx, const BKA11yRequest *req)
{
  (void)ctx;
  (void)req;
  return 0;
}

void bk_a11y_node_free(BKNode *n)
{
  (void)n;
}

void bk_a11y_ctx_free(BKContext *ctx)
{
  (void)ctx;
}

void bk_a11y_forget(BKContext *ctx, const BKNode *subtree)
{
  (void)ctx;
  (void)subtree;
}

BuiA11y *bk_a11y_open(BKContext *ctx, void *native, void (*wake)(void *data), void *data)
{
  (void)ctx;
  (void)native;
  (void)wake;
  (void)data;
  return NULL;
}

void bk_a11y_title(BKContext *ctx, const char *title)
{
  (void)ctx;
  (void)title;
}

void bk_a11y_sync(BKContext *ctx, BuiA11y *a, bool drawn)
{
  (void)ctx;
  (void)a;
  (void)drawn;
}

void bk_a11y_serve(BKContext *ctx, BuiA11y *a)
{
  (void)ctx;
  (void)a;
}
