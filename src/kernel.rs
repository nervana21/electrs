use env_logger::Builder;
use bitcoinkernel::{ChainType, Context, ContextBuilder, KernelError, Log, Logger};
use log::LevelFilter;

pub struct MainLog {}

impl Log for MainLog {
    fn log(&self, message: &str) {
        log::info!(
            target: "libbitcoinkernel",
            "{}", message.strip_suffix("\r\n").or_else(|| message.strip_suffix('\n')).unwrap_or(message));
    }
}

pub fn setup_logging() -> Result<Logger, KernelError> {
    let mut builder = Builder::from_default_env();
    builder.filter(None, LevelFilter::Info).init();
    Logger::new(MainLog {})
}

pub fn create_context(network: ChainType) -> Context {
    ContextBuilder::new()
        .chain_type(network)
        .notifications(|registry| {
            registry.register_block_tip(|_state, _hash, _progress| {});
            registry.register_header_tip(|_state, _height, _timestamp, _presync| {});
            registry.register_progress(|_title, _progress, _resume_possible| {});
            registry.register_warning_set(|_warning, _message| {});
            registry.register_warning_unset(|_warning| {});
            registry.register_flush_error(|_message| {});
            registry.register_fatal_error(|_message| {});
        })
        .build()
        .unwrap()
}
