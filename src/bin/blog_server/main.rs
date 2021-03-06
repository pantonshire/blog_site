mod config;
mod context;
mod fs_watcher;
mod render;
mod service;
mod template;

use std::{
    env,
    error,
    fmt,
    fs,
    io,
    net::SocketAddr,
    path::PathBuf,
    process,
    sync::Arc,
    thread,
};

use hyper::Server;
use tokio::signal;
use tracing::{error, info};

use blog::{
    codeblock::CodeBlockRenderer,
    db::ConcurrentPostsStore,
};

use config::Config;
use context::Context;
use render::Renderer;

fn main() {
    if let Err(err) = run() {
        eprintln!("***** Fatal error *****");
        eprintln!("{}", err);
        process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    // Load the configuration from the TOML config file specified by the first command-line
    // argument.
    let config = {
        let config_path = env::args().nth(1)
            .ok_or(Error::NoConfig)?;

        info!(path = %config_path, "Loading config");

        let contents = fs::read_to_string(&config_path)
            .map_err(Error::ReadConfig)?;
            
        contents.parse::<Config>()
            .map_err(Error::BadConfig)?
    };

    // Create the global context that will be used and modified throughout the program.
    let context = Arc::new(Context::new(config, ConcurrentPostsStore::new()));

    let code_renderer = CodeBlockRenderer::new();

    // Create the post renderer and the mpsc channel that will be used to communicate with it.
    let (renderer, tx) = Renderer::new(
        context.clone(),
        code_renderer,
        context.config().content.posts_dir.clone()
    );

    // Dropping the watcher stops its thread, so keep it alive until the server has stopped.
    let watcher = fs_watcher::start_watching(
        tx,
        &context.config().content.posts_dir,
        context.config().fs_event_delay
    )?;

    let renderer_handle = thread::spawn(move || {
        renderer.handle_events();
    });

    info!("Started renderer thread");

    // To run the web server, we need to be in an async context, so create a new Tokio runtime and
    // pass control to it.
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(Error::TokioRuntime)?
        .block_on(run_server(context))?;

    info!("Stopped server");

    info!("Stopping filesystem watcher");
    drop(watcher);

    info!("Waiting for renderer thread to exit");
    if renderer_handle.join().is_err() {
        error!("Renderer thread panicked!");
    }

    info!("Goodbye!");

    Ok(())
}

async fn run_server(context: Arc<Context>) -> Result<(), Error> {
    let service = service::site_service(context.clone());

    info!(address = %context.config().bind, "Starting server");

    Server::try_bind(&context.config().bind)
        .map_err(|err| Error::Bind(context.config().bind, err))?
        .serve(service.into_make_service())
        .with_graceful_shutdown(handle_interrupt())
        .await
        .map_err(Error::Server)
}

async fn handle_interrupt() {
    info!("Installing interrupt handler");
    
    let sigint = async {
        signal::ctrl_c()
            .await
            .unwrap()
    };

    #[cfg(unix)] {
        use signal::unix::{signal, SignalKind};

        let sigterm = async {
            signal(SignalKind::terminate())
                .unwrap()
                .recv()
                .await;
        };

        tokio::select! {
            biased;
            _ = sigterm => {
                info!("Received SIGTERM");
            },
            _ = sigint => {
                info!("Received SIGINT");
            },
        };
    }

    #[cfg(not(unix))] {
        sigint.await;
    }

    info!("Shutdown signal received, stopping");
}

#[derive(Debug)]
enum Error {
    NoConfig,
    ReadConfig(io::Error),
    BadConfig(toml::de::Error),
    CreateWatcher(notify::Error),
    WatchDir(PathBuf, notify::Error),
    TokioRuntime(io::Error),
    Bind(SocketAddr, hyper::Error),
    Server(hyper::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoConfig => {
                write!(f, "no config file specified")
            },
            Self::ReadConfig(err) => {
                write!(f, "failed to read config file: {}", err)
            },
            Self::BadConfig(err) => {
                write!(f, "error in config: {}", err)
            },
            Self::CreateWatcher(err) => {
                write!(f, "failed to create filesystem watcher: {}", err)
            },
            Self::WatchDir(path, err) => {
                write!(f, "failed to watch directory {}: {}", path.to_string_lossy(), err)
            },
            Self::TokioRuntime(err) => {
                write!(f, "failed to create async runtime: {}", err)
            },
            Self::Bind(addr, err) => {
                write!(f, "failed to bind {}: {}", addr, err)
            },
            Self::Server(err) => {
                write!(f, "error while running server: {}", err)
            },
        }
    }
}

impl error::Error for Error {}
