/* The desktop shell on macOS (shell.h). Main thread only, as AppKit; manual retain / release
 * (-fno-objc-arc), as kit_window_cocoa.m and GHOST.
 *
 *  tray icons      NSStatusItem (a template image for Blender's icons), its menu an NSMenu shown
 *                  on a right or control click, as tauri's tray-icon
 *  notifications   UNUserNotificationCenter, else NSUserNotificationCenter: both need the app to
 *                  run from a bundle with an identifier (an unbundled program has no
 *                  notifications); the UserNotifications framework is loaded at run time
 *  progress        the Dock icon with a bar drawn on it, as GHOST_WindowCocoa::setProgressBar /
 *                  endProgressBar (the Dock icon is the app's: the last window's call wins)
 *  recent files    NSDocumentController, as GHOST_SystemPathsCocoa::addToSystemRecentFiles
 *
 * Not run in CI here (no macOS): written against the AppKit API, compiled only on macOS.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#import <Cocoa/Cocoa.h>
#include <dlfcn.h>
#include <stdlib.h>

#include "kit_intern.h"
#include "shell.h"

#define MENU_BAR_PT 18.0 /* a status item's image, in points */

/* UNAuthorizationOption / UNNotificationPresentationOptions (UserNotifications, not linked) */
enum { UN_SOUND = 1 << 1, UN_ALERT = 1 << 2, UN_LIST = 1 << 3, UN_BANNER = 1 << 4 };

/* The selectors of UserNotifications (and of the deprecated NSUserNotification) this file sends
 * to `id` receivers: declared so the compiler knows their types. */
@protocol BUIShellNotificationAPI
+ (id)currentNotificationCenter;
+ (id)defaultUserNotificationCenter;
+ (id)requestWithIdentifier:(NSString *)identifier content:(id)content trigger:(id)trigger;
- (void)requestAuthorizationWithOptions:(NSUInteger)options
                      completionHandler:(void (^)(BOOL granted, NSError *error))handler;
- (void)addNotificationRequest:(id)request withCompletionHandler:(void (^)(NSError *error))handler;
- (void)setBody:(NSString *)body;
- (void)setInformativeText:(NSString *)text;
- (void)deliverNotification:(id)notification;
- (id)notification;
- (id)request;
@end

static bool g_progress_visible = false;

@interface BUIShellTarget : NSObject
@end

static BUIShellTarget *g_target;

static BKTray *tray_of(NSInteger uid)
{
  for (BKTray *t = bk_shell_trays; t; t = t->next) {
    if (t->uid == uid) {
      return t;
    }
  }
  return NULL;
}

/* A menu's items from `*id` on, numbered as bk_shell_menu_item does. */
static NSMenu *build_menu(const BKMenuSpec *items, int n, int *id, NSInteger uid)
{
  NSMenu *menu = [[[NSMenu alloc] initWithTitle:@""] autorelease];
  menu.autoenablesItems = NO;
  for (int i = 0; i < n; i++) {
    const BKMenuSpec *it = &items[i];
    const int item_id = ++*id;
    if (bk_menuspec_is_sep(it)) {
      [menu addItem:[NSMenuItem separatorItem]];
      continue;
    }
    NSMenuItem *mi = [[[NSMenuItem alloc] initWithTitle:[NSString stringWithUTF8String:it->text]
                                                 action:@selector(chose:)
                                          keyEquivalent:@""] autorelease];
    mi.target = g_target;
    mi.tag = item_id;
    mi.representedObject = [NSNumber numberWithInteger:uid];
    mi.enabled = it->enabled != 0;
    mi.state = it->checked > 0 ? NSControlStateValueOn : NSControlStateValueOff;
    if (it->nsub > 0) {
      mi.submenu = build_menu(it->sub, it->nsub, id, uid);
      mi.action = NULL;
    }
    [menu addItem:mi];
  }
  return menu;
}

@implementation BUIShellTarget

- (void)clicked:(NSStatusBarButton *)sender
{
  BKTray *t = tray_of(sender.tag);
  NSEvent *ev = [NSApp currentEvent];
  if (!t) {
    return;
  }
  const bool right = ev.type == NSEventTypeRightMouseUp ||
                     (ev.modifierFlags & NSEventModifierFlagControl);
  if (right && t->nmenu > 0) {
    NSStatusItem *item = (NSStatusItem *)t->os;
    int id = 0;
    item.menu = build_menu(t->menu, t->nmenu, &id, t->uid); /* shown by the click below */
    [item.button performClick:nil];
    item.menu = nil;
  }
  else if (!right && ev.clickCount == 2) {
    bk_shell_emit(t, BK_EV_DOUBLE, NULL, 1, 0);
  }
  else {
    bk_shell_emit(t, BK_EV_CLICK, NULL, right ? 3 : 1, 0);
  }
}

- (void)chose:(NSMenuItem *)sender
{
  BKTray *t = tray_of([(NSNumber *)sender.representedObject integerValue]);
  if (t) {
    bk_shell_menu_chosen(t, (int)sender.tag);
  }
}

/* UNUserNotificationCenterDelegate (the protocol's header is not included: not linked) */
- (void)userNotificationCenter:(id)center
    didReceiveNotificationResponse:(id)response
             withCompletionHandler:(void (^)(void))done
{
  (void)center;
  NSString *ident = [[[response notification] request] identifier];
  const int id = [[ident substringFromIndex:4] intValue]; /* "bui-<id>" */
  dispatch_async(dispatch_get_main_queue(), ^{
    bk_shell_emit(NULL, BK_EV_ACTIVATE, NULL, id, 0);
  });
  done();
}

- (void)userNotificationCenter:(id)center
       willPresentNotification:(id)notification
         withCompletionHandler:(void (^)(NSUInteger options))done
{
  (void)center;
  (void)notification;
  done(UN_BANNER | UN_LIST | UN_SOUND); /* shown while the app is in front too */
}

/* NSUserNotificationCenterDelegate */
- (void)userNotificationCenter:(id)center didActivateNotification:(id)note
{
  (void)center;
  bk_shell_emit(NULL, BK_EV_ACTIVATE, NULL, [[note identifier] intValue], 0);
}

- (BOOL)userNotificationCenter:(id)center shouldPresentNotification:(id)note
{
  (void)center;
  (void)note;
  return YES;
}

@end

static void target_ready(void)
{
  if (!g_target) {
    g_target = [[BUIShellTarget alloc] init];
  }
}

/* -- notification centers -- */

/* The UserNotifications center (with an app bundle), its delegate and permission asked once;
 * nil without one. */
static id un_center(void)
{
  static id center = nil;
  static bool tried = false;
  if (tried) {
    return center;
  }
  tried = true;
  if (![[NSBundle mainBundle] bundleIdentifier]) {
    return nil; /* the center raises for a process without a bundle */
  }
  dlopen("/System/Library/Frameworks/UserNotifications.framework/UserNotifications", RTLD_LAZY);
  Class cls = NSClassFromString(@"UNUserNotificationCenter");
  if (!cls) {
    return nil;
  }
  target_ready();
  center = [[cls performSelector:@selector(currentNotificationCenter)] retain];
  [center setDelegate:g_target];
  [center requestAuthorizationWithOptions:(UN_ALERT | UN_SOUND)
                        completionHandler:^(BOOL granted, NSError *error) {
                          (void)granted;
                          (void)error;
                        }];
  return center;
}

/* NSUserNotificationCenter (deprecated since macOS 11, still there): also needs a bundle. */
static id ns_center(void)
{
  Class cls = [[NSBundle mainBundle] bundleIdentifier] ?
                  NSClassFromString(@"NSUserNotificationCenter") :
                  nil;
  id center = [cls performSelector:@selector(defaultUserNotificationCenter)];
  if (center) {
    target_ready();
    [center setDelegate:g_target];
  }
  return center;
}

/* -- images -- */

/* `icon` as an NSImage of `pt` points at 2x (autoreleased), nil when it is not one. */
static NSImage *load_image(const char *icon, CGFloat pt)
{
  const int px = (int)(pt * 2.0);
  const float black[4] = {0.0f, 0.0f, 0.0f, 1.0f}; /* template images: AppKit tints them */
  BKImBuf *ib = bk_shell_icon(icon, px, black);
  if (!ib) {
    return nil;
  }
  NSBitmapImageRep *rep = [[[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL
                                                                    pixelsWide:px
                                                                    pixelsHigh:px
                                                                 bitsPerSample:8
                                                               samplesPerPixel:4
                                                                      hasAlpha:YES
                                                                      isPlanar:NO
                                                                colorSpaceName:NSDeviceRGBColorSpace
                                                                   bytesPerRow:px * 4
                                                                  bitsPerPixel:32] autorelease];
  const uint32_t *src = BK_imbuf_pixels(ib, 0); /* premultiplied ARGB, as the rep's default */
  unsigned char *dst = rep.bitmapData;
  for (int i = 0; i < px * px; i++) {
    const uint32_t p = src[i];
    dst[i * 4 + 0] = (unsigned char)(p >> 16);
    dst[i * 4 + 1] = (unsigned char)(p >> 8);
    dst[i * 4 + 2] = (unsigned char)p;
    dst[i * 4 + 3] = (unsigned char)(p >> 24);
  }
  BK_imbuf_free(ib);
  NSImage *image = [[[NSImage alloc] initWithSize:NSMakeSize(pt, pt)] autorelease];
  [image addRepresentation:rep];
  image.template = BK_icon_id(icon) != 0; /* Blender's icons follow the menu bar's look */
  return image;
}

static void set_button_image(NSStatusItem *item, const char *icon)
{
  NSImage *image = icon ? load_image(icon, MENU_BAR_PT) : nil;
  if (!image) {
    image = [[[NSApp applicationIconImage] copy] autorelease];
    image.size = NSMakeSize(MENU_BAR_PT, MENU_BAR_PT);
  }
  item.button.image = image;
}

/* -- the backend -- */

int bk_shell_os_features(void)
{
  if (![NSThread isMainThread]) {
    return 0;
  }
  [NSApplication sharedApplication]; /* initializes NSApp */
  int f = BK_SHELL_TRAY | BK_SHELL_PROGRESS | BK_SHELL_RECENT;
  @autoreleasepool {
    if (un_center() || ns_center()) {
      f |= BK_SHELL_NOTIFY;
    }
  }
  return f;
}

void bk_shell_os_set_app_id(const char *id)
{
  (void)id; /* the bundle's identifier is the app's */
}

bool bk_shell_os_tray_add(BKTray *t)
{
  @autoreleasepool {
    target_ready();
    NSStatusItem *item =
        [[[NSStatusBar systemStatusBar] statusItemWithLength:NSVariableStatusItemLength] retain];
    if (!item) {
      return false;
    }
    set_button_image(item, t->icon);
    item.button.toolTip = [NSString stringWithUTF8String:t->tooltip];
    item.button.tag = t->uid;
    item.button.target = g_target;
    item.button.action = @selector(clicked:);
    [item.button sendActionOn:NSEventMaskLeftMouseUp | NSEventMaskRightMouseUp];
    t->os = item;
  }
  return true;
}

void bk_shell_os_tray_remove(BKTray *t)
{
  NSStatusItem *item = (NSStatusItem *)t->os;
  [[NSStatusBar systemStatusBar] removeStatusItem:item];
  [item release];
  t->os = NULL;
}

bool bk_shell_os_tray_update(BKTray *t, int changed)
{
  NSStatusItem *item = (NSStatusItem *)t->os;
  bool ok = true;
  @autoreleasepool {
    if (changed & BK_TRAY_ICON) {
      NSImage *image = t->icon ? load_image(t->icon, MENU_BAR_PT) : nil;
      ok = image || !t->icon; /* a file that isn't an image keeps the old icon */
      if (ok) {
        set_button_image(item, t->icon);
      }
    }
    if (changed & BK_TRAY_TOOLTIP) {
      item.button.toolTip = [NSString stringWithUTF8String:t->tooltip];
    }
  }
  return ok; /* BK_TRAY_MENU: built when it opens */
}

bool bk_shell_os_notify(int id, const char *title, const char *body, const char *icon)
{
  (void)icon; /* both centers show the app's icon (an attachment would need a file) */
  @autoreleasepool {
    NSString *t = [NSString stringWithUTF8String:title], *b = [NSString stringWithUTF8String:body];
    id un = un_center();
    if (un) {
      Class content_cls = NSClassFromString(@"UNMutableNotificationContent");
      Class request_cls = NSClassFromString(@"UNNotificationRequest");
      id content = [[[content_cls alloc] init] autorelease];
      [content setTitle:t];
      [content setBody:b];
      id request = [request_cls requestWithIdentifier:[NSString stringWithFormat:@"bui-%d", id]
                                              content:content
                                              trigger:nil];
      [un addNotificationRequest:request withCompletionHandler:nil];
      return true;
    }
    id ns = ns_center();
    if (!ns) {
      return false;
    }
    id note = [[[NSClassFromString(@"NSUserNotification") alloc] init] autorelease];
    [note setTitle:t];
    [note setInformativeText:b];
    [note setIdentifier:[NSString stringWithFormat:@"%d", id]];
    [ns deliverNotification:note];
  }
  return true;
}

/* GHOST_WindowCocoa::setProgressBar: the app's icon with a bar under it; `state` colors it (Blender
 * has the normal one only). NULL `fill`: the icon alone (endProgressBar). */
static void dock_icon(float value, NSColor *fill)
{
  @autoreleasepool {
    NSImage *dock = [[NSImage alloc] initWithSize:NSMakeSize(128, 128)];
    [dock lockFocus];
    [[NSImage imageNamed:@"NSApplicationIcon"] drawAtPoint:NSZeroPoint
                                                  fromRect:NSZeroRect
                                                 operation:NSCompositingOperationSourceOver
                                                  fraction:1.0];
    if (fill) {
      NSRect rect = {{8, 8}, {112, 14}};
      [[[NSColor whiteColor] colorWithAlphaComponent:0.6] setFill]; /* the track */
      [[NSBezierPath bezierPathWithRoundedRect:rect xRadius:7 yRadius:7] fill];
      [fill setFill];
      rect = NSInsetRect(rect, 2, 2);
      rect.size.width *= value;
      [[NSBezierPath bezierPathWithRoundedRect:rect xRadius:5 yRadius:5] fill];
    }
    [dock unlockFocus];
    [NSApp setApplicationIconImage:dock];
    [dock release];
  }
}

bool bk_shell_os_progress(BKWindow *win, int state, float value)
{
  (void)win; /* the Dock icon is the app's */
  if (state == BK_PROGRESS_NONE) {
    if (g_progress_visible) {
      g_progress_visible = false;
      dock_icon(0.0f, nil);
    }
    return true;
  }
  NSColor *fill = [[NSColor blackColor] colorWithAlphaComponent:0.7];
  if (state == BK_PROGRESS_PAUSED) {
    fill = [NSColor systemYellowColor];
  }
  else if (state == BK_PROGRESS_ERROR) {
    fill = [NSColor systemRedColor];
  }
  else if (state == BK_PROGRESS_INDETERMINATE) { /* no value: a full, faint bar */
    fill = [[NSColor blackColor] colorWithAlphaComponent:0.35];
    value = 1.0f;
  }
  dock_icon(value, fill);
  g_progress_visible = true;
  return true;
}

bool bk_shell_os_add_recent(const char *path)
{
  @autoreleasepool {
    NSString *p = [NSString stringWithUTF8String:path];
    if (!p || ![[NSFileManager defaultManager] fileExistsAtPath:p]) {
      return false;
    }
    [[NSDocumentController sharedDocumentController]
        noteNewRecentDocumentURL:[NSURL fileURLWithPath:p]];
  }
  return true;
}
