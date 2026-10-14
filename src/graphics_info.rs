//! Explicit graphics-only diagnostics for native platform qualification.
//! Ordinary launches stay silent; terminal data never passes through this logger.

struct GraphicsInfo;

impl log::Log for GraphicsInfo {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() == log::Level::Info && metadata.target() == "iced_wgpu::window::compositor"
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            let message = record.args().to_string();
            // This pinned iced_wgpu message contains the selected AdapterInfo.
            // Exclude all other graphics messages and every other log target.
            if message.starts_with("Selected: AdapterInfo {") {
                eprintln!("hoori-term graphics: {message}");
            }
        }
    }

    fn flush(&self) {}
}

pub fn enable_if_requested() {
    if std::env::var_os("HOORI_GRAPHICS_INFO").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return;
    }
    static LOGGER: GraphicsInfo = GraphicsInfo;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}
