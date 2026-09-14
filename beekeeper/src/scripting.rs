// Lua scripting console for the beekeeper TUI.
// rlua (archived upstream) was replaced by mlua in ronda 8; the public API of
// this module is unchanged, so `tui.rs` needed no edits.

use mlua::{Lua, Result as LuaResult, Table, Value};

pub struct LuaEngine {
    lua: Lua,
}

impl LuaEngine {
    pub fn new() -> Self {
        Self { lua: Lua::new() }
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
            std::thread::sleep(std::time::Duration::from_secs_f64(secs));
            Ok(())
        })?;
        globals.set("sleep", sleep_fn)?;

        let print_fn = lua.create_function(|_, msg: String| {
            println!("[lua] {}", msg);
            Ok(())
        })?;
        globals.set("print", print_fn)?;
        Ok(())
    }

    pub fn eval(&mut self, code: &str) -> String {
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

    #[allow(dead_code)]
    pub fn update_colony(&mut self, agents: usize) {
        let globals = self.lua.globals();
        let colony: Table = globals
            .get("colony")
            .unwrap_or_else(|_| self.lua.create_table().unwrap());
        colony.set("agents", agents as i64).unwrap();
        globals.set("colony", colony).unwrap();
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
}
