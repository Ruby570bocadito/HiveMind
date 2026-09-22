// Lua scripting console for the beekeeper TUI.
// rlua (archived upstream) was replaced by mlua in ronda 8; the public API of
// this module is unchanged, so `tui.rs` needed no edits.
//
// Ronda 13 (the "no way out of the console" fix):
//   - `print()` used to call `println!` directly on the process stdout while
//     the TUI owned the terminal in raw mode + alternate screen — every
//     print corrupted the display. It is now captured into a buffer that
//     `tui.rs` drains into the Lua Console tab after each `eval()`.
//   - `sleep()` used to block the async event loop for as long as the script
//     asked (and `while true do end` hung the TUI forever, with Ctrl+C
//     swallowed by raw mode). Sleeps are clamped and runaway scripts are
//     aborted by an instruction-count interrupt once the eval deadline
//     passes.

use mlua::{HookTriggers, Lua, Result as LuaResult, Table, Value, VmState};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Upper bound for a single Lua `sleep()` (seconds). The console runs on
/// the TUI event loop, so anything larger would freeze the interface.
const MAX_SLEEP_SECS: f64 = 2.0;

/// Wall-clock budget for one `eval()`. Exceeded budgets abort the script
/// via the interrupt hook instead of hanging the TUI forever.
const EVAL_DEADLINE: Duration = Duration::from_secs(5);

/// Hard cap on buffered `print()` lines per eval (a `for` loop printing
/// thousands of lines must not grow the buffer unbounded).
const MAX_BUFFERED_PRINTS: usize = 500;

/// Clamp a Lua-requested sleep to a TUI-safe duration. NaN and negatives
/// become 0 (they would panic `from_secs_f64` otherwise).
fn clamp_sleep(secs: f64) -> f64 {
    if secs.is_nan() || secs < 0.0 {
        0.0
    } else {
        secs.min(MAX_SLEEP_SECS)
    }
}

pub struct LuaEngine {
    lua: Lua,
    /// Lines printed by Lua `print()`, drained by the TUI after eval.
    prints: Arc<Mutex<VecDeque<String>>>,
    /// Wall-clock deadline for the running `eval()`, read by the
    /// instruction interrupt.
    deadline: Arc<Mutex<Instant>>,
    /// Configurable eval budget (tests shorten it).
    eval_budget: Duration,
}

impl LuaEngine {
    pub fn new() -> Self {
        Self {
            lua: Lua::new(),
            prints: Arc::new(Mutex::new(VecDeque::new())),
            // Far-future initial deadline: raw table ops (update_colony)
            // never execute VM instructions, but keep a wide margin so
            // nothing outside eval() can ever observe an expired one.
            deadline: Arc::new(Mutex::new(Instant::now() + Duration::from_secs(3600))),
            eval_budget: EVAL_DEADLINE,
        }
    }

    pub fn init(&mut self) {
        if let Err(e) = self.init_inner() {
            eprintln!("lua init error: {}", e);
        }
    }

    fn init_inner(&self) -> LuaResult<()> {
        let lua = &self.lua;
        let globals = lua.globals();
        globals.set("BEE_VERSION", "3.0.0")?;
        let info = lua.create_table()?;
        info.set("name", "Hive Colony")?;
        info.set("version", "3.0.0")?;
        info.set("agents", 0)?;
        globals.set("colony", info)?;

        let sleep_fn = lua.create_function(|_, secs: f64| {
            std::thread::sleep(Duration::from_secs_f64(clamp_sleep(secs)));
            Ok(())
        })?;
        globals.set("sleep", sleep_fn)?;

        // print() → buffer, never the raw stdout (ronda 13).
        let prints = Arc::clone(&self.prints);
        let print_fn = lua.create_function(move |_, msg: String| {
            let mut buf = prints.lock().unwrap();
            if buf.len() >= MAX_BUFFERED_PRINTS {
                buf.pop_front();
            }
            buf.push_back(msg);
            Ok(())
        })?;
        globals.set("print", print_fn)?;

        // Runaway-script guard (ronda 13): the hook fires every N VM
        // instructions; once the eval deadline has passed it aborts the
        // script with a runtime error instead of looping forever.
        let deadline = Arc::clone(&self.deadline);
        self.lua.set_hook(
            HookTriggers::new().every_nth_instruction(500),
            move |_lua, _debug| {
                if Instant::now() >= *deadline.lock().unwrap() {
                    Err(mlua::Error::runtime(
                        "script interrupted: eval deadline exceeded",
                    ))
                } else {
                    Ok(VmState::Continue)
                }
            },
        );
        Ok(())
    }

    pub fn eval(&mut self, code: &str) -> String {
        *self.deadline.lock().unwrap() = Instant::now() + self.eval_budget;
        let result: LuaResult<String> = self
            .lua
            .load(code)
            .eval::<Value>()
            .map(|val| format!("{:?}", val));
        match result {
            Ok(s) => s,
            Err(e) => format!("Error: {}", e),
        }
    }

    /// Take (and clear) everything Lua printed since the last drain.
    /// The TUI shows these lines in the console pane.
    pub fn take_prints(&mut self) -> Vec<String> {
        let mut buf = self.prints.lock().unwrap();
        buf.drain(..).collect()
    }

    /// Shorten the eval deadline (used by tests to keep the interrupt
    /// check fast; production keeps the 5 s default).
    #[cfg(test)]
    pub fn set_eval_budget(&mut self, budget: Duration) {
        self.eval_budget = budget;
    }

    /// Live colony size, rendered by the TUI each frame so scripts can
    /// read `colony.agents` (ronda 13: wired in `render_tui`).
    pub fn update_colony(&mut self, agents: usize) {
        let globals = self.lua.globals();
        let colony: Table = globals
            .get("colony")
            .unwrap_or_else(|_| self.lua.create_table().unwrap());
        let _ = colony.set("agents", agents as i64);
        let _ = globals.set("colony", colony);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> LuaEngine {
        let mut e = LuaEngine::new();
        e.init();
        e
    }

    #[test]
    fn eval_arithmetic() {
        let mut e = engine();
        assert!(e.eval("1 + 1").contains("2"));
    }

    #[test]
    fn colony_globals_exposed() {
        let mut e = engine();
        let out = e.eval("colony.name");
        assert!(out.contains("Hive Colony"), "colony.name -> {}", out);
        let ver = e.eval("BEE_VERSION");
        assert!(ver.contains("3.0.0"), "BEE_VERSION -> {}", ver);
    }

    #[test]
    fn eval_syntax_error_is_string_not_panic() {
        let mut e = engine();
        let out = e.eval("this is not lua )(");
        assert!(out.starts_with("Error:"), "got: {}", out);
    }

    #[test]
    fn update_colony_writes_back() {
        let mut e = engine();
        e.update_colony(7);
        assert!(e.eval("colony.agents").contains("7"));
    }

    #[test]
    fn print_is_captured_into_the_buffer_not_stdout() {
        // Ronda 13: print() must go to the drained buffer (rendered in the
        // Lua tab), never to the raw stdout the TUI owns.
        let mut e = engine();
        e.eval(r#"print("hello hive"); print(42)"#);
        let prints = e.take_prints();
        assert_eq!(
            prints,
            vec!["hello hive".to_string(), "42".to_string()],
            "captured: {prints:?}"
        );
        // Drained, not duplicated:
        assert!(e.take_prints().is_empty());
    }

    #[test]
    fn print_buffer_is_bounded() {
        let mut e = engine();
        e.eval("for i = 1, 2000 do print(i) end");
        let prints = e.take_prints();
        assert!(
            prints.len() <= MAX_BUFFERED_PRINTS,
            "buffer held {} lines",
            prints.len()
        );
        // The newest lines survive (the oldest are evicted):
        let last = prints.last().map(|l| l.as_str()).unwrap_or_default();
        assert_eq!(last, "2000", "last buffered line: {last:?}");
    }

    #[test]
    fn sleep_is_clamped_to_a_tui_safe_maximum() {
        assert_eq!(clamp_sleep(0.5), 0.5);
        assert_eq!(clamp_sleep(0.0), 0.0);
        assert_eq!(clamp_sleep(-3.0), 0.0);
        assert_eq!(clamp_sleep(f64::NAN), 0.0);
        assert_eq!(clamp_sleep(1e9), MAX_SLEEP_SECS);
    }

    #[test]
    fn runaway_script_is_interrupted_by_deadline() {
        // `while true do end` used to hang the TUI forever (raw mode even
        // swallowed Ctrl+C). The interrupt must abort it.
        let mut e = engine();
        e.set_eval_budget(Duration::from_millis(250));
        let t0 = Instant::now();
        let out = e.eval("while true do end");
        assert!(out.starts_with("Error:"), "got: {out}");
        assert!(
            out.contains("interrupted") || out.contains("deadline"),
            "error should explain the interrupt: {out}"
        );
        assert!(
            t0.elapsed() < Duration::from_secs(4),
            "interrupt fired in {:?}",
            t0.elapsed()
        );
    }

    #[test]
    fn engine_recovers_after_interrupt() {
        // The engine must stay usable after an aborted script.
        let mut e = engine();
        e.set_eval_budget(Duration::from_millis(250));
        let _ = e.eval("while true do end");
        assert!(e.eval("2 * 21").contains("42"));
    }
}
