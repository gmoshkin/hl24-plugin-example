use std::io::Write;

static mut COUNTER: usize = 0;

#[no_mangle]
pub extern "C" fn ffi_custom_prompt() {
    let counter = unsafe { COUNTER };
    print!("{counter} $ ");
    _ = std::io::stdout().flush();

    unsafe { COUNTER += 1 };
}

const _CHECK1: plugin_sdk::CustomPromptFn = ffi_custom_prompt;

#[no_mangle]
pub extern "C" fn ffi_plugin_on_load(context: *mut ()) {

    let handler = plugin_sdk::CommandHandler::new("reset-counter".into(), |_| {
        unsafe { COUNTER = 0 };
        Ok(())
    });
    let ok = unsafe { plugin_sdk::ffi_register_command(context, handler) };
    if !ok {
        println!("couldn't register command");
    }

    let handler = plugin_sdk::CommandHandler::new("count".into(), |args| {
        println!("you provided {} arguments", args.len());
        Ok(())
    });
    unsafe { plugin_sdk::ffi_register_command(context, handler) };
}

const _CHECK2: plugin_sdk::PluginOnLoadFn = ffi_plugin_on_load;
