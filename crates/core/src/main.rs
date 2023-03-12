use anyhow::{anyhow, Result, bail};
use ::http::{request, HeaderName, HeaderValue};
use once_cell::sync::{OnceCell, Lazy};
use quickjs_wasm_rs::{Context, Value, Deserializer, Serializer};
use serde::{Serialize, Deserialize};
use serde_bytes::ByteBuf;
// use std::alloc::{alloc, dealloc, Layout};
use std::collections::HashMap;
use std::io::{self, Read};
use std::ops::Deref;
// use std::ptr::copy_nonoverlapping;
// use std::slice;
use std::str;
use std::sync::Mutex;
use send_wrapper::SendWrapper;

// mod globals;
mod outbound_http;

pub mod http {
    use anyhow::Result;

    /// The Spin HTTP request.
    pub type Request = http::Request<Option<bytes::Bytes>>;

    /// The Spin HTTP response.
    pub type Response = http::Response<Option<bytes::Bytes>>;

    pub use crate::outbound_http::send_request as send;

    /// Helper function to return a 404 Not Found response.
    pub fn not_found() -> Result<Response> {
        Ok(http::Response::builder()
            .status(404)
            .body(Some("Not Found".into()))?)
    }

    /// Helper function to return a 500 Internal Server Error response.
    pub fn internal_server_error() -> Result<Response> {
        Ok(http::Response::builder()
            .status(500)
            .body(Some("Internal Server Error".into()))?)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct HttpRequest {
    method: String,
    uri: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    body: Option<ByteBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
struct HttpResponse {
    status: u16,
    #[serde(default)]
    headers: HashMap<String, String>,
    body: Option<ByteBuf>,
}


// Unlike C's realloc, zero-length allocations need not have
// unique addresses, so a zero-length allocation may be passed
// in and also requested, but it's ok to return anything that's
// non-zero to indicate success.
// const ZERO_SIZE_ALLOCATION_PTR: *mut u8 = 1 as _;

// static mut COMPILE_SRC_RET_AREA: [u32; 2] = [0; 2];

static mut CONTEXT: OnceCell<SendWrapper<Context>> = OnceCell::new();
static mut BYTECODE: OnceCell<SendWrapper<Vec<u8>>> = OnceCell::new();
// static TASKS: Lazy<Mutex<Vec<SendWrapper<Value>>>> = Lazy::new(|| Mutex::new(Vec::new()));

// fn set_timeout(context: &Context, _this: &Value, args: &[Value]) -> Result<Value> {
//     match args {
//         [function] => {
//             TASKS
//                 .lock()
//                 .unwrap()
//                 .push(SendWrapper::new(function.clone()));

//             // TODO: If we ever add support for `clearTimeout`, we'll need to produce a unique ID here:
//             context.value_from_u32(0)
//         }
//         _ => bail!(
//             "expected one argument (function), got {} arguments",
//             args.len()
//         ),
//     }
// }

/// Used by Wizer to preinitialize the module
#[export_name = "wizer.initialize"]
pub extern "C" fn init() {
    // let context = Context::default();
    // globals::inject_javy_globals(&context, io::stderr(), io::stderr()).unwrap();
    // unsafe { CONTEXT.set(SendWrapper::new(context)).unwrap() };
    do_init().unwrap();
}

fn main() {
    println!("start to run wasm");
    let bytecode = unsafe { BYTECODE.take().unwrap() };
    let context = unsafe { CONTEXT.take().unwrap() };
    
    context.eval_binary(&bytecode).unwrap();
}

/// Compiles JS source code to QuickJS bytecode.
///
/// Returns a pointer to a buffer containing a 32-bit pointer to the bytecode byte array and the
/// u32 length of the bytecode byte array.
///
/// # Arguments
///
/// * `js_src_ptr` - A pointer to the start of a byte array containing UTF-8 JS source code
/// * `js_src_len` - The length of the byte array containing JS source code
///
/// # Safety
///
/// * `js_src_ptr` must reference a valid array of unsigned bytes of `js_src_len` length
// #[export_name = "compile_src"]
// pub unsafe extern "C" fn compile_src(js_src_ptr: *const u8, js_src_len: usize) -> *const u32 {
//     // Use fresh context to avoid depending on Wizened context
//     let context = Context::default();
//     let js_src = str::from_utf8(slice::from_raw_parts(js_src_ptr, js_src_len)).unwrap();
//     let bytecode = context.compile_global("function.mjs", js_src).unwrap();
//     let bytecode_len = bytecode.len();
//     // We need the bytecode buffer to live longer than this function so it can be read from memory
//     let bytecode_ptr = Box::leak(bytecode.into_boxed_slice()).as_ptr();
//     COMPILE_SRC_RET_AREA[0] = bytecode_ptr as u32;
//     COMPILE_SRC_RET_AREA[1] = bytecode_len.try_into().unwrap();
//     COMPILE_SRC_RET_AREA.as_ptr()
// }

// /// Evaluates QuickJS bytecode
// ///
// /// # Safety
// ///
// /// * `bytecode_ptr` must reference a valid array of unsigned bytes of `bytecode_len` length
// #[export_name = "eval_bytecode"]
// pub unsafe extern "C" fn eval_bytecode(bytecode_ptr: *const u8, bytecode_len: usize) {
//     let context = CONTEXT.get().unwrap();
//     let bytecode = slice::from_raw_parts(bytecode_ptr, bytecode_len);
//     context.eval_binary(bytecode).unwrap();
// }

// /// 1. Allocate memory of new_size with alignment.
// /// 2. If original_ptr != 0
// ///   a. copy min(new_size, original_size) bytes from original_ptr to new memory
// ///   b. de-allocate original_ptr
// /// 3. return new memory ptr
// ///
// /// # Safety
// ///
// /// * `original_ptr` must be 0 or a valid pointer
// /// * if `original_ptr` is not 0, it must be valid for reads of `original_size`
// ///   bytes
// /// * if `original_ptr` is not 0, it must be properly aligned
// /// * if `original_size` is not 0, it must match the `new_size` value provided
// ///   in the original `canonical_abi_realloc` call that returned `original_ptr`
// #[export_name = "canonical_abi_realloc"]
// pub unsafe extern "C" fn canonical_abi_realloc(
//     original_ptr: *mut u8,
//     original_size: usize,
//     alignment: usize,
//     new_size: usize,
// ) -> *mut std::ffi::c_void {
//     assert!(new_size >= original_size);

//     let new_mem = match new_size {
//         0 => ZERO_SIZE_ALLOCATION_PTR,
//         // this call to `alloc` is safe since `new_size` must be > 0
//         _ => alloc(Layout::from_size_align(new_size, alignment).unwrap()),
//     };

//     if !original_ptr.is_null() && original_size != 0 {
//         copy_nonoverlapping(original_ptr, new_mem, original_size);
//         canonical_abi_free(original_ptr, original_size, alignment);
//     }
//     new_mem as _
// }

// /// Frees memory
// ///
// /// # Safety
// ///
// /// * `ptr` must denote a block of memory allocated by `canonical_abi_realloc`
// /// * `size` and `alignment` must match the values provided in the original
// ///   `canonical_abi_realloc` call that returned `ptr`
// #[export_name = "canonical_abi_free"]
// pub unsafe extern "C" fn canonical_abi_free(ptr: *mut u8, size: usize, alignment: usize) {
//     if size > 0 {
//         dealloc(ptr, Layout::from_size_align(size, alignment).unwrap())
//     };
// }

fn do_init() -> Result<()> {
    let mut script = String::new();
    io::stdin().read_to_string(&mut script)?;
    // println!("{}", &script);

    let context = Context::default();
    let bytecode = context.compile_module("function.mjs", &script).unwrap();
    // context.eval_global("script.js", &script)?;

    let global = context.global_object()?;

    let console = context.object_value()?;
    console.set_property("log", context.wrap_callback(console_log)?)?;

    global.set_property("console", console)?;

    global.set_property("reqwest_get", context.wrap_callback(spin_send_http_request)?)?;

    // global.set_property("setTimeout", context.wrap_callback(set_timeout)?)?;
    println!("function ability inject complete");

    unsafe{
        CONTEXT.set(SendWrapper::new(context)).unwrap();
        BYTECODE.set(SendWrapper::new(bytecode)).unwrap();
        println!("CONTEXT and BYTECODE set complete");
    }
    Ok(())
}

fn console_log(context: &Context, _this: &Value, args: &[Value]) -> Result<Value> {
    let mut spaced = false;
    for arg in args {
        if spaced {
            print!(" ");
        } else {
            spaced = true;
        }
        print!("{}", arg.as_str()?);
    }
    println!();

    context.undefined_value()
}

fn spin_send_http_request(context: &Context, _this: &Value, args: &[Value]) -> Result<Value> {
    match args {
        [request] => {
            let deserializer = &mut Deserializer::from(request.clone());
            let request = HttpRequest::deserialize(deserializer)?;

            let mut builder = request::Builder::new()
                .method(request.method.deref())
                .uri(request.uri.deref());

            if let Some(headers) = builder.headers_mut() {
                for (key, value) in &request.headers {
                    headers.insert(
                        HeaderName::from_bytes(key.as_bytes())?,
                        HeaderValue::from_bytes(value.as_bytes())?,
                    );
                }
            }

            let response = outbound_http::send_request(
                builder.body(request.body.map(|buffer| buffer.into_vec().into()))?,
            )?;

            let response = HttpResponse {
                status: response.status().as_u16(),
                headers: response
                    .headers()
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            key.as_str().to_owned(),
                            str::from_utf8(value.as_bytes())?.to_owned(),
                        ))
                    })
                    .collect::<Result<_>>()?,
                body: response
                    .into_body()
                    .map(|bytes| ByteBuf::from(bytes.deref())),
            };

            let mut serializer = Serializer::from_context(context)?;
            response.serialize(&mut serializer)?;
            Ok(serializer.value)
        }

        _ => Err(anyhow!("expected 1 argument, got {}", args.len())),
    }
}