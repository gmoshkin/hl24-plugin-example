use std::io::Write;

type Result<T, E=Error> = std::result::Result<T, E>;

fn main() {
    let res = run();
    if let Err(e) = res {
        eprintln!("ERROR: {}", e.cause);
        eprintln!("Backtrace:");
        eprintln!("{}", e.backtrace);
    }
}

fn run() -> Result<()> {
    println!("Hello!");

    let mut plugin_state = PluginState::new();

    let mut line = String::new();
    loop {
        display_command_line_prompt(&plugin_state)?;

        line.clear();
        std::io::stdin().read_line(&mut line)?;

        if line.is_empty() {
            // EOF
            break;
        }

        let Some((command, args)) = parse_command(&line) else {
            continue;
        };

        match command {
            "help" => do_command_help(&plugin_state),
            "echo" => do_command_echo(&args),
            "exit" => break,
            "load-plugin" => do_command_load_plugin(&args, &mut plugin_state)?,
            "unload-all-plugins" => do_command_unload_all_plugins(&mut plugin_state)?,
            _ => {
                let handled = do_plugin_command(command, &args, &mut plugin_state)?;

                if !handled {
                    println!("unknown command `{command}`");
                }
            }
        }
    }

    println!("Good bye!");

    Ok(())
}

fn display_command_line_prompt(plugin_state: &PluginState) -> Result<()> {
    // SAFETY: static mut variable access is safe because we don't have any multithreading
    if let Some(function) = plugin_state.custom_prompt_function {
        // SAFETY: safety of this call depends on the implementation. If we
        // don't control who writes this code and have no way of making sure it
        // is safe, than we cannot guarantee that this call is safe.
        unsafe { function() };
    } else {
        print!("> ");
        std::io::stdout().flush()?;
    }

    Ok(())
}

fn do_command_help(plugin_state: &PluginState) {
    println!("supported commands:");
    println!("   help");
    println!("   echo");
    println!("   exit");
    println!("   load-plugin");
    println!("   unload-all-plugins");
    if !plugin_state.custom_commands.is_empty() {
        println!("commands from plugins:");
        for command in plugin_state.custom_commands.keys() {
            println!("   {command}");
        }
    }
}

fn do_command_echo(args: &[&str]) {
    let mut iter = args.iter();
    if let Some(first) = iter.next() { print!("{first}"); }
    for next in iter { print!(" {next}"); }
    println!("");
}

fn parse_command(s: &str) -> Option<(&str, Vec<&str>)> {
    let s = s.trim();
    let mut iter = s.split_whitespace();
    let Some(command) = iter.next() else {
        return None;
    };

    let args = iter.collect();
    Some((command, args))
}

////////////////////////////////////////////////////////////////////////////////
// plugins
////////////////////////////////////////////////////////////////////////////////

struct PluginState {
    module: *mut libc::c_void,
    path: Option<std::ffi::CString>,
    custom_prompt_function: Option<plugin_sdk::CustomPromptFn>,
    custom_commands: std::collections::HashMap<String, plugin_sdk::CommandHandler>,
}

impl PluginState {
    fn new() -> Self {
        Self {
            module: std::ptr::null_mut(),
            path: None,
            custom_prompt_function: None,
            custom_commands: Default::default(),
        }
    }
}

fn do_plugin_command(command: &str, args: &[&str], plugin_state: &mut PluginState) -> Result<bool> {
    let Some(handler) = plugin_state.custom_commands.get(command) else {
        return Ok(false);
    };

    handler.call(args);

    return Ok(true);
}

fn do_command_load_plugin(args: &[&str], plugin_state: &mut PluginState) -> Result<()> {
    if plugin_state.path.is_some() {
        println!("plugin already loaded, multiple plugins are not supported yet");
        return Ok(());
    }

    let [path] = args else {
        println!("expected a file path as first argument");
        return Ok(());
    };

    let path = std::ffi::CString::new(*path).map_err(Error::new)?;

    // SAFETY: this is safe because file path is a valid nul-terminated string pointer
    let module = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_LOCAL | libc::RTLD_NOW) };
    if module.is_null() {
        return Err(make_dlerror_error());
    }

    plugin_state.module = module;
    plugin_state.path = Some(path);

    // SAFETY: assumming the plugin defines the symbol with the correct signature
    let fn_ptr = unsafe { load_symbol(module, c"ffi_custom_prompt")? };
    plugin_state.custom_prompt_function = Some(fn_ptr);

    // SAFETY: assumming the plugin defines the symbol with the correct signature
    let register_commands: plugin_sdk::RegisterCommandsFn = unsafe {
        load_symbol(module, c"ffi_register_commands")?
    };

    let context = plugin_state as *mut _ as *mut ();
    unsafe { (register_commands)(context) }

    Ok(())
}

unsafe fn load_symbol<F>(module: *mut libc::c_void, name: &std::ffi::CStr) -> Result<F> {
    // SAFETY: this is safe because `module` is returned by `dlopen` and the
    // symbol name is a valid nul-terminated string pointer
    let symbol = libc::dlsym(module, name.as_ptr());
    if symbol.is_null() {
        return Err(make_dlerror_error());
    }

    // SAFETY: a cast from `*mut void` is only safe if the dynamic library exports the symbol with the correct type
    let func_ptr: F = std::mem::transmute_copy(&symbol);
    Ok(func_ptr)
}

fn do_command_unload_all_plugins(plugin_state: &mut PluginState) -> Result<()> {
    if plugin_state.path.is_none() {
        println!("no plugins loaded yet");
        return Ok(());
    }

    // SAFETY: this is safe because file path is a valid nul-terminated string pointer
    let rc = unsafe { libc::dlclose(plugin_state.module) };
    if rc != 0 {
        return Err(make_dlerror_error());
    }

    let plugin_path = plugin_state.path.take().expect("just made sure it's there");
    println!("unloaded plugin {plugin_path:?}");
    plugin_state.module = std::ptr::null_mut();
    plugin_state.custom_prompt_function = None;
    plugin_state.custom_commands.clear();

    Ok(())
}

fn make_dlerror_error() -> Error {
    // SAFETY: this call is always safe
    let error = unsafe { libc::dlerror() };
    assert!(!error.is_null());
    // SAFETY: dlerror returns only valid nul-terminated string pointers
    let message = unsafe { std::ffi::CStr::from_ptr(error) };
    let message = message.to_string_lossy();
    return Error::message(message);
}

#[no_mangle]
pub extern "C" fn ffi_register_command(context: *mut (), handler: plugin_sdk::CommandHandler) -> bool {
    let plugin_state_ptr = context as *mut PluginState;
    let plugin_state = unsafe { &mut *plugin_state_ptr };

    let command = handler.name().to_owned();
    let e = plugin_state.custom_commands.entry(command);
    match e {
        std::collections::hash_map::Entry::Occupied(e) => {
            let command = e.key();
            println!("ERROR: custom command '{command}' is already registerred");
            return false;
        }
        std::collections::hash_map::Entry::Vacant(e) => {
            e.insert(handler);
        }
    }

    return true;
}

////////////////////////////////////////////////////////////////////////////////
// Error
////////////////////////////////////////////////////////////////////////////////

struct Error {
    cause: Box<dyn std::error::Error>,
    backtrace: std::backtrace::Backtrace,
}

impl Error {
    fn new(e: impl std::error::Error + 'static) -> Self {
        Self { cause: Box::new(e), backtrace: std::backtrace::Backtrace::capture() }
    }

    fn message(m: impl Into<String>) -> Self {
        Self { cause: m.into().into(), backtrace: std::backtrace::Backtrace::capture() }
    }
}

impl From<std::io::Error> for Error {
    #[inline(always)]
    fn from(e: std::io::Error) -> Self {
        Self { cause: Box::new(e), backtrace: std::backtrace::Backtrace::capture() }
    }
}
