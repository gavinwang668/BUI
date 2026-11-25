/* Accessibility trees (BUI_kit.h "accessibility").
 *
 * A context's tree is built on demand from its nodes: each class says what it is (BKClass
 * a11y_role) and, for the items it draws itself, describes them (a11y_item); this file reads the
 * rest from the generic node fields (text, label, value, range, flags). Layout containers are
 * generic: their children belong to the nearest node a reader sees. A node holding a host's
 * subtree (BK_a11y_set_subtree) shows that subtree instead of its children; kit widgets the subtree
 * names appear there, not where they are in the kit's tree.
 *
 * Ids are stable while the nodes live: a kit node's is its address (shifted), an item's adds its
 * index, a host node's mixes the host's key with its holder's id (top bit set). Requests on kit
 * nodes are done here; those on host nodes go to the host (BK_EV_A11Y_ACTION).
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "a11y.h"
#include "bui_a11y.h"
#include "kit_intern.h"

/* bui-a11y mirrors the struct (a11y/src/node.rs asserts the same size) */
_Static_assert(sizeof(BKA11yNode) == 72 + 4 * sizeof(void *), "BKA11yNode changed: update a11y/src/node.rs");

/* A host's subtree, copied: the nodes, then their strings (one block). */
typedef struct BKA11ySub {
  int n;
  BKA11yNode nodes[];
} BKA11ySub;

/* Where a tree node comes from: a kit node (item -1), an item it draws, or a host's node (key). */
typedef struct BKA11ySrc {
  BKNode *node; /* the kit node, or the node holding the host's subtree */
  uint64_t key; /* the host's key; 0: not a host node */
  int item;
} BKA11ySrc;

/* The strings of a tree (blocks that never move, so the nodes can point into them). */
typedef struct Chunk {
  struct Chunk *next;
  size_t used, cap;
  char data[];
} Chunk;

typedef struct BKA11yCtx {
  BKA11yNode *nodes; /* the last tree, its root first */
  BKA11ySrc *src;
  int n, cap;
  int *map; /* while subtrees are added (they nest): their node index -> tree index */
  int mapn, mapcap;
  Chunk *strings;
  char *title;
  BKNode *focus;     /* a kit node a reader focused (kit widgets keep no keyboard focus) */
  uint64_t focus_id; /* the tree's focus */
  bool read;         /* a reader asked: hosts keep their subtrees */
  bool dirty;        /* changed since the tree last went out (a subtree, a request) */
  void (*wake)(void *data);
  void *wake_data;
} BKA11yCtx;

static BKA11yCtx *state(BKContext *ctx)
{
  if (!ctx->a11y) {
    ctx->a11y = (BKA11yCtx *)calloc(1, sizeof(BKA11yCtx));
  }
  return ctx->a11y;
}

/* -------------------------------------------------------------------- */
/** \name Ids
 * \{ */

#define HOST_BIT (UINT64_C(1) << 63)
#define ROOT_ID UINT64_C(1)
#define MAX_ITEMS 0xfffe

/* User-space addresses fit in 48 bits and are 8-aligned: 45 bits, shifted past 16 bits of item. */
static uint64_t kit_id(const BKNode *n)
{
  return ((uint64_t)(uintptr_t)n >> 3) << 16;
}

static uint64_t host_id(const BKNode *holder, uint64_t key)
{
  uint64_t z = key ^ kit_id(holder); /* splitmix64 */
  z = (z ^ (z >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
  z = (z ^ (z >> 27)) * UINT64_C(0x94d049bb133111eb);
  return (z ^ (z >> 31)) | HOST_BIT;
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Building
 * \{ */

static void strings_free(BKA11yCtx *a)
{
  while (a->strings) {
    Chunk *next = a->strings->next;
    free(a->strings);
    a->strings = next;
  }
}

/* `s` copied into the tree's strings (NULL for none). */
static const char *keep(BKA11yCtx *a, const char *s, size_t len)
{
  if (!s || !len) {
    return NULL;
  }
  Chunk *c = a->strings;
  if (!c || c->cap - c->used < len + 1) {
    const size_t cap = BK_MAX(len + 1, 4096);
    c = (Chunk *)malloc(sizeof(Chunk) + cap);
    c->next = a->strings;
    c->used = 0;
    c->cap = cap;
    a->strings = c;
  }
  char *out = c->data + c->used;
  memcpy(out, s, len);
  out[len] = '\0';
  c->used += len + 1;
  return out;
}

static const char *keep_str(BKA11yCtx *a, const char *s)
{
  return s ? keep(a, s, strlen(s)) : NULL;
}

static int push(BKA11yCtx *a, const BKA11yNode *node, BKA11ySrc src)
{
  if (a->n == a->cap) {
    a->cap = BK_MAX(64, a->cap * 2);
    a->nodes = (BKA11yNode *)realloc(a->nodes, sizeof(BKA11yNode) * (size_t)a->cap);
    a->src = (BKA11ySrc *)realloc(a->src, sizeof(BKA11ySrc) * (size_t)a->cap);
  }
  a->nodes[a->n] = *node;
  a->src[a->n] = src;
  return a->n++;
}

static const char *nonempty(const char *s)
{
  return s && s[0] ? s : NULL;
}

static bool text_role(int role)
{
  return role == BK_ROLE_TEXT_INPUT || role == BK_ROLE_MULTILINE_TEXT_INPUT ||
         role == BK_ROLE_SEARCH_INPUT || role == BK_ROLE_PASSWORD_INPUT;
}

static bool numeric_role(int role)
{
  return role == BK_ROLE_SPIN_BUTTON || role == BK_ROLE_SLIDER || role == BK_ROLE_PROGRESS ||
         role == BK_ROLE_METER || role == BK_ROLE_SCROLL_BAR;
}

/* Opens a popup of its own (dropdowns, menu buttons, popovers). */
static bool opens_popup(const BKNode *n)
{
  return n->cls == &bk_cls_dropdown || n->cls == &bk_cls_menubutton || n->cls == &bk_cls_popover;
}

static void node_bounds(const BKNode *n, BKA11yNode *r)
{
  r->x = (float)n->x;
  r->y = (float)n->y;
  r->w = (float)n->w;
  r->h = (float)n->h;
}

/* A kit node as a reader sees it (id, role, name, value, states, bounds). */
static void describe(BKA11yCtx *a, BKContext *ctx, BKNode *n, BKA11yNode *r)
{
  const int role = n->cls->a11y_role;
  memset(r, 0, sizeof(*r));
  r->id = kit_id(n);
  r->role = (uint16_t)role;
  r->min = r->max = NAN;
  node_bounds(n, r);
  /* the name: its label, else its text (unless that is its value), else its tooltip */
  const char *name = nonempty(n->label);
  if (!name && !text_role(role)) {
    name = nonempty(n->text);
  }
  if (!name && text_role(role)) {
    name = nonempty(n->placeholder);
  }
  const char *desc = nonempty(n->desc);
  if (!name) {
    name = desc;
    desc = NULL;
  }
  r->name = keep_str(a, name);
  r->description = keep_str(a, desc);
  char buf[1024];
  if (text_role(role)) {
    const char *t = n->edit ? n->edit->text :
                    n->cls == &bk_cls_textarea ? bk_textarea_text(n) :
                                                 (n->text ? n->text : "");
    if (n->mask[0]) { /* a password: what it shows */
      r->value = keep(a, buf, (size_t)bk_mask_text(t, (int)strlen(t), n->mask, buf, sizeof(buf)));
    }
    else {
      r->value = keep_str(a, t);
    }
    r->flags |= BK_A11Y_EDITABLE;
  }
  else if (role == BK_ROLE_SPIN_BUTTON || role == BK_ROLE_SLIDER) {
    r->value = keep_str(a, bk_number_display(n));
  }
  else if (n->cls->to_text && (role == BK_ROLE_COMBO_BOX || role == BK_ROLE_COLOR_WELL) &&
           n->cls->to_text(n, buf, sizeof(buf)))
  {
    r->value = keep_str(a, buf);
  }
  if (numeric_role(role)) {
    r->flags |= BK_A11Y_NUMERIC;
    r->num = n->value;
    const bool ranged = role != BK_ROLE_SPIN_BUTTON;
    r->min = (ranged || (n->flags & NF_HAS_MIN)) ? n->vmin : NAN;
    r->max = (ranged || (n->flags & NF_HAS_MAX)) ? n->vmax : NAN;
    if (!(r->max > r->min)) {
      r->min = r->max = NAN;
    }
    r->step = n->step;
    if (role == BK_ROLE_PROGRESS || role == BK_ROLE_METER) {
      r->flags |= BK_A11Y_READONLY;
    }
  }
  /* states */
  if (!NODE_ENABLED(n)) {
    r->flags |= BK_A11Y_DISABLED;
  }
  if (n->cls->flags & CLS_INTERACTIVE) {
    r->flags |= BK_A11Y_FOCUSABLE | BK_A11Y_CLICKABLE;
  }
  if (n->cls->flags & CLS_VALUE) {
    r->flags |= BK_A11Y_CONTEXT_MENU;
  }
  /* checkboxes, radios, and buttons that hold a value (toggles, decorators) */
  if (role == BK_ROLE_CHECKBOX || role == BK_ROLE_RADIO || role == BK_ROLE_SWITCH ||
      (role == BK_ROLE_BUTTON && (n->cls->flags & CLS_VALUE)))
  {
    r->flags |= BK_A11Y_CHECKABLE | (n->value != 0 ? BK_A11Y_CHECKED : 0);
  }
  if (opens_popup(n)) {
    r->flags |= BK_A11Y_POPUP | BK_A11Y_EXPANDABLE |
                (bk_owns_popup(ctx, n) ? BK_A11Y_EXPANDED : 0);
  }
  if (n->cls == &bk_cls_panel) {
    r->flags |= BK_A11Y_EXPANDABLE | ((n->flags & NF_COLLAPSED) ? 0 : BK_A11Y_EXPANDED);
  }
  if (n->flags & NF_ACTIVE_DEFAULT) {
    r->flags |= BK_A11Y_DEFAULT;
  }
  if (role == BK_ROLE_LIST_BOX && n->selmode != 0) {
    r->flags |= BK_A11Y_MULTISELECTABLE;
  }
  if (ctx->focus == n) {
    r->flags |= BK_A11Y_FOCUSABLE | BK_A11Y_FOCUSED;
  }
}

static void walk(BKA11yCtx *a, BKContext *ctx, BKNode *n, int parent);

/* A kit node, its items and its children under `parent`; `name` (a host's) replaces its own. */
static int add_kit(BKA11yCtx *a, BKContext *ctx, BKNode *n, int parent, const char *name)
{
  BKA11yNode r;
  describe(a, ctx, n, &r);
  if (name) {
    r.name = name;
  }
  r.parent = parent;
  const int at = push(a, &r, (BKA11ySrc){n, 0, -1});
  for (int i = 0; n->cls->a11y_item && i < MAX_ITEMS; i++) {
    BKA11yNode it;
    memset(&it, 0, sizeof(it));
    it.min = it.max = NAN;
    if (!n->cls->a11y_item(n, i, &it)) {
      break;
    }
    if (it.role == BK_ROLE_GENERIC) { /* separators and the like */
      continue;
    }
    it.id = kit_id(n) | (uint64_t)(i + 1);
    it.parent = at;
    it.name = keep_str(a, it.name);
    it.description = keep_str(a, it.description);
    it.value = keep_str(a, it.value);
    if (!NODE_ENABLED(n)) {
      it.flags |= BK_A11Y_DISABLED;
    }
    push(a, &it, (BKA11ySrc){n, 0, i});
  }
  for (int i = 0; i < n->nkids; i++) {
    walk(a, ctx, n->kids[i], at);
  }
  return at;
}

/* The subtree `holder` shows, under `parent`. */
static void add_subtree(BKA11yCtx *a, BKContext *ctx, BKNode *holder, int parent)
{
  const BKA11ySub *sub = holder->a11y_sub;
  /* a widget in it may hold a subtree too: each takes its own part of `map` */
  const int base = a->mapn;
  if (a->mapcap < base + sub->n) {
    a->mapcap = BK_MAX(base + sub->n, 2 * a->mapcap);
    a->map = (int *)realloc(a->map, sizeof(int) * (size_t)a->mapcap);
  }
  a->mapn = base + sub->n;
  for (int i = 0; i < sub->n; i++) {
    const BKA11yNode *h = &sub->nodes[i];
    const int up = h->parent < 0 ? parent : a->map[base + h->parent];
    int at;
    if (h->widget) {
      /* the kit widget, named as the host names it (an HTML <label>) */
      at = add_kit(a, ctx, h->widget, up, nonempty(h->name) ? keep_str(a, h->name) : NULL);
      if (h->flags & BK_A11Y_FOCUSED) {
        a->nodes[at].flags |= BK_A11Y_FOCUSABLE | BK_A11Y_FOCUSED;
      }
    }
    else if (h->role == BK_ROLE_GENERIC && !nonempty(h->name) && !(h->flags & ~BK_A11Y_CLIPS)) {
      at = up; /* a plain container: looked through */
    }
    else {
      BKA11yNode r = *h; /* its strings copied: the host may replace the subtree */
      r.id = host_id(holder, h->id);
      r.parent = up;
      r.name = keep_str(a, h->name);
      r.description = keep_str(a, h->description);
      r.value = keep_str(a, h->value);
      at = push(a, &r, (BKA11ySrc){holder, h->id, -1});
    }
    a->map[base + i] = at;
  }
  a->mapn = base;
}

static void walk(BKA11yCtx *a, BKContext *ctx, BKNode *n, int parent)
{
  if (!NODE_VISIBLE(n) || n->a11y_holder) {
    return; /* hidden, or shown in its holder's subtree */
  }
  if (n->a11y_sub) {
    add_subtree(a, ctx, n, parent);
    return;
  }
  if (n->cls->a11y_role == BK_ROLE_GENERIC) {
    for (int i = 0; i < n->nkids; i++) {
      walk(a, ctx, n->kids[i], parent);
    }
    return;
  }
  add_kit(a, ctx, n, parent, NULL);
}

static int find(const BKA11yCtx *a, uint64_t id)
{
  for (int i = 0; i < a->n; i++) {
    if (a->nodes[i].id == id) {
      return i;
    }
  }
  return -1;
}

/* The focus: the open menu's row, else the field being edited, else what the host focused, else
 * what a reader focused, else the window. */
static uint64_t focus_of(const BKA11yCtx *a, BKContext *ctx)
{
  if (ctx->npopups) {
    BKNode *top = ctx->popups[ctx->npopups - 1];
    const uint64_t row = kit_id(top) | (uint64_t)(top->hot + 1);
    if (top->hot >= 0 && find(a, row) >= 0) {
      return row;
    }
    return kit_id(top);
  }
  if (ctx->focus) {
    return kit_id(ctx->focus);
  }
  for (int i = 0; i < a->n; i++) {
    if (a->nodes[i].flags & BK_A11Y_FOCUSED) {
      return a->nodes[i].id;
    }
  }
  if (a->focus && find(a, kit_id(a->focus)) >= 0) {
    return kit_id(a->focus);
  }
  return ROOT_ID;
}

static void build(BKA11yCtx *a, BKContext *ctx, const char *title)
{
  a->n = 0;
  strings_free(a);
  BKA11yNode root;
  memset(&root, 0, sizeof(root));
  root.id = ROOT_ID;
  root.parent = -1;
  root.role = BK_ROLE_WINDOW;
  root.name = keep_str(a, nonempty(title) ? title : a->title);
  root.min = root.max = NAN;
  root.w = (float)ctx->win_w;
  root.h = (float)ctx->win_h;
  push(a, &root, (BKA11ySrc){NULL, 0, -1});
  a->mapn = 0;
  if (ctx->header) {
    walk(a, ctx, ctx->header, 0);
  }
  if (ctx->root) {
    walk(a, ctx, ctx->root, 0);
  }
  for (int i = 0; i < ctx->npopups; i++) {
    walk(a, ctx, ctx->popups[i], 0);
  }
  a->focus_id = focus_of(a, ctx);
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Requests
 * \{ */

static void node_center(const BKNode *n, int *r_x, int *r_y)
{
  *r_x = n->x + n->w / 2;
  *r_y = n->y + n->h / 2;
}

/* A request on a kit node (item -1) or one of its items. */
static int kit_do(BKA11yCtx *a, BKContext *ctx, BKNode *n, int item, const BKA11yRequest *q)
{
  if (!NODE_ENABLED(n) && q->action != BK_A11Y_FOCUS) {
    return 0;
  }
  if (n->cls->a11y_do && n->cls->a11y_do(n, item, q->action)) {
    return 1;
  }
  if (item >= 0) {
    return 0;
  }
  int cx, cy;
  node_center(n, &cx, &cy);
  const int role = n->cls->a11y_role;
  switch (q->action) {
    case BK_A11Y_EXPAND:
      if (opens_popup(n) && bk_owns_popup(ctx, n)) {
        return 1;
      }
      /* fall through: opening is its default action */
    case BK_A11Y_CLICK: {
      /* what Return does to it, as for a keyboard user; else a click on its middle */
      BKEvent e;
      memset(&e, 0, sizeof(e));
      e.x = cx;
      e.y = cy;
      e.lx = cx - n->x;
      e.ly = cy - n->y;
      e.text = "";
      e.keysym = "Return";
      e.inside = true;
      if (bk_node_key(n, &e)) {
        return 1;
      }
      if (n->cls->flags & CLS_INTERACTIVE) {
        BK_press(ctx, cx, cy, 0);
        BK_release(ctx, cx, cy, 0);
        return 1;
      }
      return 0;
    }
    case BK_A11Y_COLLAPSE:
      if (bk_owns_popup(ctx, n)) {
        BK_close_popups(ctx);
        return 1;
      }
      return 0;
    case BK_A11Y_FOCUS:
      a->focus = n;
      if (text_role(role) || role == BK_ROLE_SPIN_BUTTON) {
        BK_focus_node(ctx, n);
      }
      return 1;
    case BK_A11Y_SET_VALUE:
      if (ctx->focus == n) {
        bk_end_edit(ctx, false); /* the value is the reader's, not what was being typed */
      }
      if (q->text && n->cls == &bk_cls_textarea) {
        return bk_textarea_set_text(n, q->text);
      }
      if (q->text && n->cls->from_text) {
        return n->cls->from_text(n, q->text);
      }
      if (!q->text && (n->cls->flags & CLS_VALUE) && isfinite(q->num)) {
        bk_set_value(n, q->num);
        return 1;
      }
      return 0;
    case BK_A11Y_INCREMENT:
    case BK_A11Y_DECREMENT:
      if (n->cls->step) {
        n->cls->step(n, q->action == BK_A11Y_INCREMENT ? 1 : -1);
        return 1;
      }
      return 0;
    case BK_A11Y_CONTEXT_MENU_SHOW:
      return BK_context_menu(ctx, cx, cy, 0);
    default:
      return 0;
  }
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name API
 * \{ */

int BK_a11y_supported(void)
{
  return 1;
}

int BK_a11y_wanted(const BKNode *node)
{
  const BKContext *ctx = node ? bk_ctx(node) : NULL;
  return ctx && ctx->a11y && ctx->a11y->read;
}

/* `w` is `holder` or around it: it can't be a widget of its subtree. */
static bool around(const BKNode *w, const BKNode *holder)
{
  for (const BKNode *p = holder; p; p = p->parent) {
    if (p == w) {
      return true;
    }
  }
  return false;
}

static const BKNode *top_of(const BKNode *n)
{
  while (n->parent) {
    n = n->parent;
  }
  return n;
}

static void sub_release(BKNode *holder)
{
  BKA11ySub *sub = holder->a11y_sub;
  if (!sub) {
    return;
  }
  for (int i = 0; i < sub->n; i++) {
    BKNode *w = sub->nodes[i].widget;
    if (w && w->a11y_holder == holder) {
      w->a11y_holder = NULL;
    }
  }
  free(sub);
  holder->a11y_sub = NULL;
}

static size_t str_size(const char *s)
{
  return s ? strlen(s) + 1 : 0;
}

static const char *str_copy(char **at, const char *s)
{
  if (!s) {
    return NULL;
  }
  const size_t len = strlen(s) + 1;
  char *out = *at;
  memcpy(out, s, len);
  *at += len;
  return out;
}

int BK_a11y_set_subtree(BKNode *node, const BKA11yNode *nodes, int n)
{
  if (!node || n < 0 || bui_a11y_validate(nodes, n, 1) != 0) {
    return 0;
  }
  size_t size = sizeof(BKA11ySub) + sizeof(BKA11yNode) * (size_t)n;
  for (int i = 0; i < n; i++) {
    const BKNode *w = nodes[i].widget;
    if (w && (around(w, node) || top_of(w) != top_of(node) ||
              (w->a11y_holder && w->a11y_holder != node)))
    {
      return 0;
    }
    size += str_size(nodes[i].name) + str_size(nodes[i].description) + str_size(nodes[i].value);
  }
  sub_release(node);
  if (n > 0) {
    BKA11ySub *sub = (BKA11ySub *)malloc(size);
    sub->n = n;
    char *at = (char *)(sub->nodes + n);
    for (int i = 0; i < n; i++) {
      BKA11yNode *c = &sub->nodes[i];
      *c = nodes[i];
      c->name = str_copy(&at, nodes[i].name);
      c->description = str_copy(&at, nodes[i].description);
      c->value = str_copy(&at, nodes[i].value);
      if (c->widget) {
        c->widget->a11y_holder = node;
      }
    }
    node->a11y_sub = sub;
  }
  BKContext *ctx = bk_ctx(node);
  if (ctx && ctx->a11y) {
    ctx->a11y->dirty = true;
    bk_ctx_request(ctx);
  }
  return 1;
}

int BK_a11y_tree(BKContext *ctx, const char *title, const BKA11yNode **r_nodes, uint64_t *r_focus)
{
  BKA11yCtx *a = state(ctx);
  if (!a->read) {
    a->read = true; /* from now on hosts keep their subtrees */
    if (ctx->root) {
      bk_emit(ctx->root, BK_EV_A11Y_TREE, NULL, 0, 0);
    }
  }
  build(a, ctx, title);
  if (r_nodes) {
    *r_nodes = a->nodes;
  }
  if (r_focus) {
    *r_focus = a->focus_id;
  }
  return a->n;
}

int BK_a11y_action(BKContext *ctx, const BKA11yRequest *req)
{
  if (!req || req->action < 0 || req->action >= BK_A11Y_ACTION_COUNT) {
    return 0;
  }
  BKA11yCtx *a = state(ctx);
  build(a, ctx, NULL); /* this frame's nodes: the last tree's may be gone */
  const int i = find(a, req->id);
  if (i < 0) {
    return 0;
  }
  const BKA11ySrc src = a->src[i];
  int done = 0;
  if (src.key) {
    /* the host's node: the host does it */
    BKA11yRequest q = *req;
    q.id = src.key;
    bk_emit(src.node, BK_EV_A11Y_ACTION, &q, req->action, 0);
    done = 1;
  }
  else if (src.node) {
    done = kit_do(a, ctx, src.node, src.item, req);
  }
  if (done) {
    a->dirty = true;
    bk_ctx_request(ctx);
  }
  return done;
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Kit internals
 * \{ */

void bk_a11y_node_free(BKNode *n)
{
  sub_release(n);
  BKNode *holder = n->a11y_holder;
  if (holder && holder->a11y_sub) { /* its holder's subtree no longer shows it */
    for (int i = 0; i < holder->a11y_sub->n; i++) {
      if (holder->a11y_sub->nodes[i].widget == n) {
        holder->a11y_sub->nodes[i].widget = NULL;
      }
    }
  }
  n->a11y_holder = NULL;
}

void bk_a11y_ctx_free(BKContext *ctx)
{
  BKA11yCtx *a = ctx->a11y;
  if (!a) {
    return;
  }
  strings_free(a);
  free(a->nodes);
  free(a->src);
  free(a->map);
  free(a->title);
  free(a);
  ctx->a11y = NULL;
}

void bk_a11y_forget(BKContext *ctx, const BKNode *subtree)
{
  BKA11yCtx *a = ctx->a11y;
  if (a && a->focus && around(subtree, a->focus)) {
    a->focus = NULL;
  }
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Window hosts
 * \{ */

static int host_tree(void *data, const BKA11yNode **r_nodes, int *r_count, uint64_t *r_focus)
{
  BKContext *ctx = (BKContext *)data;
  *r_count = BK_a11y_tree(ctx, NULL, r_nodes, r_focus);
  ctx->a11y->dirty = false;
  return *r_count > 0;
}

static void host_wake(void *data)
{
  const BKA11yCtx *a = ((BKContext *)data)->a11y; /* set before the adapter exists, never changed */
  a->wake(a->wake_data);
}

BuiA11y *bk_a11y_open(BKContext *ctx, void *native, void (*wake)(void *data), void *data)
{
  BKA11yCtx *a = state(ctx);
  a->wake = wake;
  a->wake_data = data;
  const BuiA11yHost host = {ctx, host_tree, host_wake};
  return bui_a11y_new(native, &host);
}

void bk_a11y_title(BKContext *ctx, const char *title)
{
  BKA11yCtx *a = state(ctx);
  if (!bk_streq(a->title, title)) {
    bk_set_text(&a->title, title);
    a->dirty = true;
  }
}

void bk_a11y_sync(BKContext *ctx, BuiA11y *adapter, bool drawn)
{
  BKA11yCtx *a = ctx->a11y;
  if (!a || !bui_a11y_active(adapter) || !(drawn || a->dirty)) {
    return;
  }
  build(a, ctx, NULL);
  a->dirty = false;
  bui_a11y_update(adapter, a->nodes, a->n, a->focus_id);
}

void bk_a11y_serve(BKContext *ctx, BuiA11y *adapter)
{
  BKA11yRequest q;
  while (bui_a11y_take(adapter, &q)) {
    BK_a11y_action(ctx, &q);
  }
}

/** \} */
