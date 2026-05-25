use rlua::{Lua, Result as LuaResult};

pub struct LuaEngine {
    lua: Lua,
}

impl LuaEngine {
    pub fn new() -> Self {
        Self { lua: Lua::new() }
    }

    pub fn init(&mut self) {
        if let Err(e) = self.lua.context(|ctx| {
            let globals = ctx.globals();
            globals.set("BEE_VERSION", "3.0.0").unwrap();
            let info = ctx.create_table().unwrap();
            info.set("name", "Hive Colony").unwrap();
            info.set("version", "3.0.0").unwrap();
            info.set("agents", 0).unwrap();
            globals.set("colony", info).unwrap();

            let sleep_fn = ctx.create_function(|_, secs: f64| {
                std::thread::sleep(std::time::Duration::from_secs_f64(secs));
                Ok(())
            }).unwrap();
            globals.set("sleep", sleep_fn).unwrap();

            let print_fn = ctx.create_function(|_, msg: String| {
                println!("[lua] {}", msg);
                Ok(())
            }).unwrap();
            globals.set("print", print_fn).unwrap();
            Ok::<_, rlua::Error>(())
        }) {
            eprintln!("lua init error: {}", e);
        }
    }

    pub fn eval(&mut self, code: &str) -> String {
        let result: LuaResult<String> = self.lua.context(|ctx| {
            let val: rlua::Value = ctx.load(code).eval()?;
            Ok(format!("{:?}", val))
        });
        match result {
            Ok(s) => s,
            Err(e) => format!("Error: {}", e),
        }
    }

    pub fn update_colony(&mut self, agents: usize) {
        let _ = self.lua.context(|ctx| {
            let globals = ctx.globals();
            let colony: rlua::Table = globals.get("colony").unwrap_or_else(|_| ctx.create_table().unwrap());
            colony.set("agents", agents as i64).unwrap();
            globals.set("colony", colony).unwrap();
            Ok::<_, rlua::Error>(())
        });
    }
}
