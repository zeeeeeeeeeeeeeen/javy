use std::fs;

#[cfg(not(target_os = "windows"))]
use binaryen::{CodegenConfig, Module as ModuleB};
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};
use {
    anyhow::{bail, Result},
    std::{env, fs::File, path::PathBuf, process::Command},
    structopt::StructOpt,
    wizer::Wizer,
};

use crate::outbound_http::{wasi_outbound_http, OutboundHttp};

mod outbound_http;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "runjs",
    about = "A spin plugin to convert javascript files to Spin compatible modules"
)]
pub struct Options {
    #[structopt(parse(from_os_str))]
    pub input: PathBuf,

    #[structopt(short = "o", parse(from_os_str), default_value = "index.wasm")]
    pub output: PathBuf,
}

pub struct Context<T> {
    pub wasi: WasiCtx,
    pub runtime_data: Option<T>,
}

fn main() -> Result<()> {
    let opts = Options::from_args();

    if env::var("SPIN_JS_WIZEN").eq(&Ok("1".into())) {
        env::remove_var("SPIN_JS_WIZEN");

        println!("\nStarting to build Spin compatible module");

        let wasm: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/engine.wasm"));

        println!("Preinitiating using Wizer");

        let mut wasm = Wizer::new()
            .allow_wasi(true)?
            .inherit_stdio(true)
            .wasm_bulk_memory(true)
            .run(wasm)?;

        let codegen_cfg = CodegenConfig {
            optimization_level: 3,
            shrink_level: 0,
            debug_info: false,
        };

        println!("Optimizing wasm binary using wasm-opt");

        if let Ok(mut module) = ModuleB::read(&wasm) {
            module.optimize(&codegen_cfg);
            module
                .run_optimization_passes(vec!["strip"], &codegen_cfg)
                .unwrap();
            wasm = module.write();
        } else {
            bail!("Unable to read wasm binary for wasm-opt optimizations");
        }

        fs::write(&opts.output, &wasm)?;

        return Ok(());
    }

    let script = File::open(&opts.input)?;

    let self_cmd = env::args().next().unwrap();

    env::set_var("SPIN_JS_WIZEN", "1");
    let status = Command::new(self_cmd)
        .arg(&opts.input)
        .arg("-o")
        .arg(&opts.output)
        .stdin(script)
        .status()?;

    if !status.success() {
        bail!("Couldn't create wasm from input");
    }

    println!("Spin compatible module built successfully");

    let wasm: &[u8] = include_bytes!("../index.wasm");
    let wasi = WasiCtxBuilder::new()
        .inherit_stdin()
        .inherit_stdout()
        .inherit_stderr()
        .build();

    let runtime_data = Some(OutboundHttp::new(Some(vec![
        "https://example.com".to_string()
    ])));

    let ctx = Context { wasi, runtime_data };

    let mut config = Config::new();
    config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
    config.wasm_multi_memory(true);
    config.wasm_module_linking(true);

    let engine = Engine::new(&config)?;
    let module = Module::from_binary(&engine, &wasm)?;
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |cx: &mut Context<OutboundHttp>| &mut cx.wasi).unwrap();
    wasi_outbound_http::add_to_linker(&mut linker, |ctx| -> &mut OutboundHttp {
        ctx.runtime_data.as_mut().unwrap()
    }).unwrap();
    let mut store = Store::new(&engine, ctx);
    let instance = linker.instantiate(&mut store, &module)?;

    let main_fn = instance.get_typed_func::<(), (), _>(&mut store, "_start")?;

    main_fn.call(&mut store, ()).unwrap();

    Ok(())
}
