// Test utils for unit tests, integration tests and doc tests
//
// In this file, you can only use public items from spdlog-rs, as this file will
// be used from integration tests and doc tests.
//
// This file will be handled in `build.rs` for code generation to workaround the
// rustdoc bug https://github.com/rust-lang/rust/issues/67295

use std::{
    fmt::Write,
    marker::PhantomData,
    sync::{atomic::*, Arc, Mutex},
    thread::sleep,
    time::Duration,
};

use atomic::Atomic;
use spdlog::{
    formatter::{FmtExtraInfo, Formatter, Pattern, PatternFormatter},
    sink::{Sink, WriteSink, WriteSinkBuilder},
    Error, ErrorHandler, LevelFilter, Logger, LoggerBuilder, Record, Result, StringBuf,
};

//////////////////////////////////////////////////

pub struct CounterSink {
    level_filter: Atomic<LevelFilter>,
    log_counter: AtomicUsize,
    flush_counter: AtomicUsize,
    payloads: Mutex<Vec<String>>,
    delay_duration: Option<Duration>,
}

impl CounterSink {
    #[must_use]
    pub fn new() -> Self {
        Self::with_delay(None)
    }

    #[must_use]
    pub fn with_delay(duration: Option<Duration>) -> Self {
        Self {
            level_filter: Atomic::new(LevelFilter::All),
            log_counter: AtomicUsize::new(0),
            flush_counter: AtomicUsize::new(0),
            payloads: Mutex::new(vec![]),
            delay_duration: duration,
        }
    }

    #[must_use]
    pub fn log_count(&self) -> usize {
        self.log_counter.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn flush_count(&self) -> usize {
        self.flush_counter.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn payloads(&self) -> Vec<String> {
        self.payloads.lock().unwrap().clone()
    }

    pub fn reset(&self) {
        self.log_counter.store(0, Ordering::Relaxed);
        self.flush_counter.store(0, Ordering::Relaxed);
        self.payloads.lock().unwrap().clear();
    }
}

impl Sink for CounterSink {
    fn log(&self, record: &Record) -> Result<()> {
        if let Some(delay) = self.delay_duration {
            sleep(delay);
        }

        self.log_counter.fetch_add(1, Ordering::Relaxed);

        self.payloads
            .lock()
            .unwrap()
            .push(record.payload().to_string());

        Ok(())
    }

    fn flush(&self) -> Result<()> {
        if let Some(delay) = self.delay_duration {
            sleep(delay);
        }

        self.flush_counter.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn level_filter(&self) -> LevelFilter {
        self.level_filter.load(Ordering::Relaxed)
    }

    fn set_level_filter(&self, level_filter: LevelFilter) {
        self.level_filter.store(level_filter, Ordering::Relaxed);
    }

    fn set_formatter(&self, _formatter: Box<dyn Formatter>) {
        unimplemented!("no-op")
    }

    fn set_error_handler(&self, _handler: Option<ErrorHandler>) {
        unimplemented!("no-op")
    }
}

impl Default for CounterSink {
    fn default() -> Self {
        Self::new()
    }
}

//////////////////////////////////////////////////

pub struct StringSink {
    underlying: WriteSink<Vec<u8>>,
}

impl StringSink {
    pub fn new() -> Self {
        Self {
            underlying: WriteSink::builder().target(vec![]).build().unwrap(),
        }
    }

    pub fn with(
        cb: impl FnOnce(
            WriteSinkBuilder<Vec<u8>, PhantomData<Vec<u8>>>,
        ) -> WriteSinkBuilder<Vec<u8>, PhantomData<Vec<u8>>>,
    ) -> Self {
        Self {
            underlying: cb(WriteSink::builder().target(vec![])).build().unwrap(),
        }
    }

    pub fn clone_string(&self) -> String {
        String::from_utf8(self.underlying.clone_target()).unwrap()
    }
}

impl Sink for StringSink {
    fn log(&self, record: &Record) -> Result<()> {
        self.underlying.log(record)
    }

    fn flush(&self) -> Result<()> {
        self.underlying.flush()
    }

    fn level_filter(&self) -> LevelFilter {
        self.underlying.level_filter()
    }

    fn set_level_filter(&self, level_filter: LevelFilter) {
        self.underlying.set_level_filter(level_filter)
    }

    fn set_formatter(&self, formatter: Box<dyn Formatter>) {
        self.underlying.set_formatter(formatter)
    }

    fn set_error_handler(&self, handler: Option<ErrorHandler>) {
        self.underlying.set_error_handler(handler)
    }
}

//////////////////////////////////////////////////

// no modifications formatter, it will write `record` to `dest` as is.
#[derive(Clone)]
pub struct NoModFormatter {}

impl NoModFormatter {
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Formatter for NoModFormatter {
    fn format(&self, record: &Record, dest: &mut StringBuf) -> Result<FmtExtraInfo> {
        dest.write_str(record.payload())
            .map_err(Error::FormatRecord)?;

        Ok(FmtExtraInfo::new())
    }

    fn clone_box(&self) -> Box<dyn Formatter> {
        Box::new(self.clone())
    }
}

impl Default for NoModFormatter {
    fn default() -> Self {
        Self::new()
    }
}

//////////////////////////////////////////////////

#[must_use]
pub fn build_test_logger(cb: impl FnOnce(&mut LoggerBuilder) -> &mut LoggerBuilder) -> Logger {
    let mut builder = Logger::builder();
    cb(builder.error_handler(|err| panic!("{}", err)));
    builder.build().unwrap()
}

pub fn assert_send<T: Send>() {}

pub fn assert_sync<T: Sync>() {}

#[must_use]
pub fn echo_logger_from_pattern(
    pattern: impl Pattern + Clone + 'static,
    name: Option<&'static str>,
) -> (Logger, Arc<StringSink>) {
    echo_logger_from_formatter(Box::new(PatternFormatter::new(pattern)), name)
}

#[must_use]
pub fn echo_logger_from_formatter(
    formatter: Box<dyn Formatter>,
    name: Option<&'static str>,
) -> (Logger, Arc<StringSink>) {
    let sink = Arc::new(StringSink::with(|b| b.formatter(formatter)));

    let mut builder = Logger::builder();

    builder.sink(sink.clone());
    if let Some(name) = name {
        builder.name(name);
    }

    (builder.build().unwrap(), sink)
}
