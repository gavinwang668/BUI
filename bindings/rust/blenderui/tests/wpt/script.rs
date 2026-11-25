//! A page's scripts, run in QuickJS against `dom.js`: the page's tree goes in as JSON, with its
//! layout before the scripts ran (what `offsetWidth` & co. read); the scripts run in document
//! order, then its load events, then its timers and animation frames on a virtual clock until
//! none is left (which is when a `reftest-wait` page would be done). The tree comes back
//! U+0001-separated (`dom.js`'s `__encode`) for [`crate::doc::Doc`] to read.

use std::time::{Duration, Instant};

use rquickjs::{Context, Function, Runtime};

const DOM: &str = include_str!("dom.js");

/// How long a page's scripts may run (an endless loop is the test's failure, not the run's).
const BUDGET: Duration = Duration::from_secs(10);

/// Timers and frames a page may run (an animation loop never ends by itself).
const STEPS: usize = 2000;

/// The tree `tree` (JSON) becomes once its scripts ran; `layout`: element serial -> box (JSON),
/// `serials`: the first serial free for new elements, `parse`: an HTML fragment as tree JSON
/// (`innerHTML`). `None` when the DOM itself failed to start.
pub fn run(tree: &str, layout: &str, serials: u32, parse: impl Fn(&str) -> String + 'static) -> Option<String> {
    let rt = Runtime::new().ok()?;
    rt.set_memory_limit(512 << 20);
    rt.set_max_stack_size(8 << 20);
    let deadline = Instant::now() + BUDGET;
    rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    let ctx = Context::full(&rt).ok()?;
    let verbose = std::env::var_os("WPT_JSLOG").is_some();
    let started = ctx.with(|ctx| -> rquickjs::Result<i32> {
        let g = ctx.globals();
        g.set("__tree", tree)?;
        g.set("__layout", layout)?;
        g.set("__serials", serials)?;
        g.set("__log", Function::new(ctx.clone(), move |s: String| {
            if verbose {
                eprintln!("js: {s}");
            }
        })?)?;
        g.set("__parse", Function::new(ctx.clone(), move |s: String| parse(&s))?)?;
        ctx.eval::<(), _>(DOM)?;
        g.get("__scripts")
    });
    let scripts = match started {
        Ok(n) => n,
        Err(e) => {
            if verbose {
                eprintln!("js: the DOM failed: {e}");
            }
            return None;
        }
    };
    // promise jobs run between tasks, as a browser's microtasks do
    let drain = || {
        while rt.is_job_pending() && Instant::now() < deadline {
            let _ = rt.execute_pending_job();
        }
    };
    let call = |name: &str, arg: i32| -> bool {
        ctx.with(|ctx| ctx.globals().get::<_, Function>(name).and_then(|f| f.call::<_, Option<bool>>((arg,))))
            .ok().flatten().unwrap_or(false)
    };
    for i in 0..scripts {
        call("__script", i);
        drain();
    }
    call("__loaded", 0);
    drain();
    for _ in 0..STEPS {
        if !call("__step", 0) || Instant::now() > deadline {
            break;
        }
        drain();
    }
    rt.set_interrupt_handler(None);
    ctx.with(|ctx| ctx.globals().get::<_, Function>("__encode").and_then(|f| f.call::<_, String>(()))).ok()
}
