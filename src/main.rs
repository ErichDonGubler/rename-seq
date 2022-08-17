use self::cli::MainArgs;
use clap::Parser;
use color_eyre::eyre::{self, bail, WrapErr};
use rename_seq::{zip_single_side_scans, RenameSpec, Visitor};
use std::{
    convert::Infallible,
    fs,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

mod cli;

fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    let MainArgs {
        go,
        allow_warnings,
        order,
        rename_spec: rename_spec_str,
        selection,
    } = MainArgs::parse();

    color_eyre::install().unwrap();

    let rename_spec = RenameSpec::new(&rename_spec_str).wrap_err("failed to parse rename spec")?;

    if !rename_spec.has_dynamic_content() {
        tracing::warn!("rename spec {rename_spec_str:?} does not have any dynamic content; this probably isn't what you want!");
        if !allow_warnings {
            bail!("warning(s) emitted, and `--allow-warnings` was not specified; bailing");
        }
    }

    let files = selection.files()?;

    let dry_run = !go;
    if dry_run {
        tracing::info!("doing a dry run of all moves");
    }

    let files_iter: Box<dyn Iterator<Item = &Path>> = match order {
        cli::Order::Sequential => Box::new(files.iter().map(|p| p.as_ref())),
        cli::Order::SingleSidedScans => Box::new(ZigZag::new(files.iter()).map(|p| p.as_ref())),
    };

    zip_single_side_scans(files_iter, rename_spec, ZipVisitor { dry_run })
        .wrap_err("failed to execute zipping operation")?;

    if dry_run {
        tracing::info!("dry run complete; use the `--go` flag to actually rename files");
    }

    Ok(())
}

struct ZipVisitor {
    dry_run: bool,
}

// [Workaround] for an upstream `tracing` issue where `tracing::event!(...)` only permits a constant
// `$level` parameter.
//
// [Workaround]: https://github.com/tokio-rs/tracing/issues/372#issuecomment-762529515
macro_rules! event {
    ($level:expr, $($args:tt)*) => {{
        use ::tracing::Level;

        match $level {
            Level::ERROR => ::tracing::event!(Level::ERROR, $($args)*),
            Level::WARN => ::tracing::event!(Level::WARN, $($args)*),
            Level::INFO => ::tracing::event!(Level::INFO, $($args)*),
            Level::DEBUG => ::tracing::event!(Level::DEBUG, $($args)*),
            Level::TRACE => ::tracing::event!(Level::TRACE, $($args)*),
        }
    }};
}

impl Visitor for ZipVisitor {
    type Error = Infallible;

    fn visit(&mut self, idx: usize, from: &Path, to: PathBuf) -> ControlFlow<Self::Error> {
        let &mut Self { dry_run } = self;

        let _span = tracing::debug_span!("renaming file", rename_idx = idx,).entered();

        let tracing_level = if dry_run {
            tracing::Level::INFO
        } else {
            tracing::Level::DEBUG
        };
        event!(tracing_level, "renaming {from:?} to {to:?}",);

        if !dry_run {
            if let Err(e) = fs::rename(from, &to)
                .wrap_err_with(|| format!("failed to rename file {from:?} to {to:?}"))
            {
                tracing::error!("{e:#}");
            }
        }

        ControlFlow::Continue(())
    }
}

struct ZigZag<T, I>
where
    I: Iterator<Item = T> + DoubleEndedIterator,
{
    next: ForwardOrBackward,
    inner: I,
}

impl<T, I> ZigZag<T, I>
where
    I: Iterator<Item = T> + DoubleEndedIterator,
{
    pub fn new(iter: I) -> Self {
        Self {
            next: ForwardOrBackward::Forward,
            inner: iter,
        }
    }
}

impl<T, I> Iterator for ZigZag<T, I>
where
    I: Iterator<Item = T> + DoubleEndedIterator,
{
    type Item = T;

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn next(&mut self) -> Option<Self::Item> {
        let Self { next, inner } = self;

        let item;
        match *next {
            ForwardOrBackward::Forward => {
                item = inner.next()?;
                *next = ForwardOrBackward::Backward;
            }
            ForwardOrBackward::Backward => {
                item = inner.next_back()?;
                *next = ForwardOrBackward::Forward;
            }
        }
        Some(item)
    }
}

#[derive(Clone, Copy, Debug)]
enum ForwardOrBackward {
    Forward,
    Backward,
}
