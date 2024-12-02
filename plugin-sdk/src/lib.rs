pub type CustomPromptFn = extern "C" fn();
pub type PluginOnLoadFn = extern "C" fn(*mut ());

extern "C" {
    pub fn ffi_register_command(context: *mut (), callback: CommandHandler) -> bool;
}

#[repr(C)]
pub struct CommandHandler {
    pub name: FfiSafeString,

    pub closure_data: *mut (),
    pub closure_fn: CommandCallbackFn,

    pub drop_fn: unsafe extern "C" fn(handler: *mut CommandHandler),
}

impl CommandHandler {
    pub fn new<F>(name: String, callback: F) -> Self
    where
        F: Fn(&[&str]) -> Result<(), Box<dyn std::error::Error>>
    {
        let name = FfiSafeString::new(name);

        let closure = Box::new(callback);
        let closure_ptr = Box::into_raw(closure);

        Self {
            name,
            closure_data: closure_ptr as *mut (),

            closure_fn: Self::trampoline::<F>,
            drop_fn: Self::drop::<F>,
        }
    }

    pub fn name(&self) -> &str {
        // SAFETY: data in `name` is owned by `self`
        unsafe { self.name.as_str() }
    }

    pub fn call(&self, args: &[&str]) -> bool {
        let mut args_copy = Vec::with_capacity(args.len());
        for arg in args {
            args_copy.push(FfiSafeStr::new(arg));
        }

        (self.closure_fn)(self, FfiSafeSlice::new(&args_copy))
    }

    extern "C" fn trampoline<F>(handler: *const Self, args: FfiSafeSlice<FfiSafeStr>) -> bool
    where
        F: Fn(&[&str]) -> Result<(), Box<dyn std::error::Error>>,
    {
        // SAFETY: our API guarantees that args outlives this function call
        let args = unsafe { args.as_slice() };

        let mut args_copy = Vec::with_capacity(args.len());
        for arg in args {
            // SAFETY: our API guarantees that args outlives this function call
            let arg_copy = unsafe { arg.as_str() };
            args_copy.push(arg_copy);
        }

        let closure;
        // SAFETY: data is guaranteed to be valid for the duration of this call
        unsafe {
            let handler = &*handler;
            let closure_ptr = handler.closure_data as *const F;
            closure = unsafe { &*closure_ptr };
        }

        let res = closure(&args_copy);
        if let Err(e) = res {
            println!("ERROR: {e}");
            return false;
        }

        return true;
    }

    /// # Safety
    /// Data must be valid and nobody should have reference to it.
    unsafe extern "C" fn drop<F>(handler: *mut Self) {
        let handler = &mut *handler;
        let closure_ptr = handler.closure_data as *mut F;

        // Drop any data captured by the closure
        _ = Box::from(closure_ptr);
    }
}

impl Drop for CommandHandler {
    fn drop(&mut self) {
        // SAFETY: `&mut` guarantees we have the only reference.
        // The validity of memory is guranteed by our API.
        unsafe { (self.drop_fn)(self) }
    }
}

type CommandCallbackFn = extern "C" fn(
    handler: *const CommandHandler,
    args: FfiSafeSlice<FfiSafeStr>,
) -> bool;

#[repr(C)]
pub struct FfiSafeString {
    pub data: *mut u8,
    pub len: usize,
}

impl FfiSafeString {
    fn new(mut s: String) -> Self {
        // We don't want to store the capacity separate from length
        s.shrink_to_fit();

        // Convert to Vec because String::from_raw_parts is unstable
        let mut bytes = s.into_bytes();
        let data = bytes.as_mut_ptr();
        let len = bytes.len();

        // Now Self ownes the memory
        std::mem::forget(bytes);

        Self { data, len }
    }

    /// # Safety
    /// Data in `self` must be valid and must not be referenced by anybody else
    /// and this function must only be called once.
    unsafe fn into_string(self) -> String {
        let bytes = Vec::from_raw_parts(self.data, self.len, self.len);

        // bytes now ownes the data
        std::mem::forget(self);

        // SAFETY: data is guaranteed to be utf8 by construction
        String::from_utf8_unchecked(bytes)
    }

    /// # Safety
    /// The pointer must be valid for the lifetime of `self`.
    unsafe fn as_str(&self) -> &str {
        let bytes = std::slice::from_raw_parts(self.data, self.len);
        // SAFETY: data is guaranteed to be utf8 by construction
        std::str::from_utf8_unchecked(bytes)
    }
}

impl Drop for FfiSafeString {
    fn drop(&mut self) {
        // SAFETY: caller must make sure nobody references the data and it's valid
        _ = unsafe { Vec::from_raw_parts(self.data, self.len, self.len) };
    }
}


#[repr(C)]
pub struct FfiSafeStr {
    pub data: *const u8,
    pub len: usize,
}

impl FfiSafeStr {
    fn new(s: &str) -> FfiSafeStr {
        let data = s.as_ptr();
        let len = s.len();
        Self { data, len }
    }

    /// # Safety
    /// The pointer must be valid for the lifetime of `self`.
    unsafe fn as_str(&self) -> &str {
        let bytes = std::slice::from_raw_parts(self.data, self.len);
        // SAFETY: data is guaranteed to be utf8 by construction
        std::str::from_utf8_unchecked(bytes)
    }
}

#[repr(C)]
pub struct FfiSafeSlice<T> {
    pub data: *const T,
    pub len: usize,
}

impl<T> FfiSafeSlice<T> {
    fn new(s: &[T]) -> Self {
        let data = s.as_ptr();
        let len = s.len();

        Self { data, len }
    }

    /// # Safety
    /// The caller must make sure that the pointer is valid for the lifetime of `self`.
    unsafe fn as_slice(&self) -> &[T] {
        std::slice::from_raw_parts(self.data, self.len)
    }
}
