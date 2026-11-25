/* Native objects whose D wrappers the garbage collector finalized, freed later at a safe point.
 *
 * The GC finalizes in no particular order, so a finalizer must not reach any other D object: a
 * handler store, what a wrapper keeps alive, a custom node's handlers. Freeing a native node or
 * context there also calls back into D through the kit's hooks (BK_set_ref_hooks: GC.addRoot /
 * removeRoot, which a finalizer may not call). So a finalizer only cuts the native object's links
 * to D and queues it here, on a lock-free list in malloc'd memory (allowed in a finalizer, from
 * whichever thread collects); `freeCollected` frees the queue from ordinary code, contexts before
 * nodes (a context goes before its root). Whatever is still queued when the program ends is left
 * to the process teardown.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */
module bui.internal.collected;

import core.atomic : atomicExchange, atomicLoad, cas;
import core.stdc.stdlib : cfree = free, malloc;

import bui.kit;

private struct Entry {
    void* handle;
    bool context; /* a BKContext, else a BKNode */
    Entry* next;
}

private shared(Entry*) g_queue;

/// Queue a native node or context for `freeCollected`. Finalizer-safe: no GC memory, no D calls.
package(bui) void deferFree(void* handle, bool context) nothrow @nogc {
    auto e = cast(Entry*) malloc(Entry.sizeof);
    if (e is null) {
        return; /* no memory: the native object leaks, which beats a crash */
    }
    e.handle = handle;
    e.context = context;
    shared(Entry*) old;
    do {
        old = atomicLoad(g_queue);
        e.next = cast(Entry*) old;
    } while (!cas(&g_queue, old, cast(shared(Entry*)) e));
}

/// Free what finalizers queued: every context, then every node. Call from ordinary code only.
package(bui) void freeCollected() {
    auto all = cast(Entry*) atomicExchange(&g_queue, cast(shared(Entry*)) null);
    foreach (pass; 0 .. 2) {
        const contexts = pass == 0;
        for (auto e = all; e !is null; e = e.next) {
            if (e.context != contexts || e.handle is null) {
                continue;
            }
            if (contexts) {
                BK_context_free(cast(BKContext*) e.handle);
            }
            else {
                auto node = cast(BKNode*) e.handle;
                if (auto parent = BK_node_parent(node)) {
                    BK_node_remove(parent, node);
                }
                BK_node_free(node);
            }
        }
    }
    while (all !is null) {
        auto next = all.next;
        cfree(all);
        all = next;
    }
}
