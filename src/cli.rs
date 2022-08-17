use clap::{Parser, ValueEnum};
use color_eyre::{
    eyre::{self, eyre},
    Section,
};
use itertools::{Either, Itertools};
use snafu::Snafu;
use std::{ops::Not, path::PathBuf};
use wax::{FilterTarget, Glob, IteratorExt};

/// Command-line arguments parsed by [`main`].
#[derive(Debug, Parser)]
pub struct MainArgs {
    /// Actually rename files, instead of performing a dry run.
    #[clap(long)]
    pub go: bool,
    /// Execute renaming even if there are warnings of likely unintended behavior.
    #[clap(long)]
    pub allow_warnings: bool,
    /// The order in which selected files should be renamed.
    #[clap(long, default_value_t = Order::Sequential, value_enum)]
    pub order: Order,
    /// A pattern describing renamed path names, of the form `[<prefix>{padded_idx}]<suffix>`.
    ///
    /// Further examples:
    ///
    /// - `photo-{padded_idx}.jpg` # `photo-1.jpg`, `photo-2.jpg`, etc.
    ///
    /// - `asdf.txt` # Renames all files, in succession, to `asdf.txt`. You probably don't want
    /// this.
    pub rename_spec: String,
    #[clap(subcommand)]
    pub selection: Selection,
}

/// Represents a selection of files in [`MainArgs`], according to rules that differ between variants.
#[derive(Clone, Debug, Parser)]
pub enum Selection {
    /// Select files by shell arguments, in the order provided.
    FromFiles { files: Vec<PathBuf> },
    /// Select files via cross-platform globbing.
    FromGlob {
        /// A `wax` glob pattern to use to list files to be renamed.
        glob: String,
        /// The way that paths matching `glob` should be sorted.
        #[clap(long, default_value_t, value_enum)]
        sort_by: SortBy,
    },
}

impl Selection {
    pub fn files(self) -> eyre::Result<Vec<PathBuf>> {
        let files = match self {
            Selection::FromFiles { files } => files,
            Selection::FromGlob { glob, sort_by } => {
                let glob = Glob::new(&glob).map_err(|source| CliGlobParseError {
                    source: source.into_owned(),
                })?;

                let (mut files, fs_errs): (Vec<_>, Vec<_>) = glob
                    .walk(".")
                    .filter_tree(|entry| {
                        entry
                            .file_type()
                            .is_file()
                            .not()
                            .then_some(FilterTarget::File)
                    })
                    .partition_map(|res| match res {
                        Ok(entry) => Either::Left(entry.into_path()),
                        Err(e) => Either::Right(e),
                    });

                if !fs_errs.is_empty() {
                    fs_errs.into_iter().fold(
                        Err(eyre!("encountered one or more file system errors")),
                        |report, e| report.error(e),
                    )?;
                }

                match sort_by {
                    SortBy::Discovered => (),
                    SortBy::Lexicographical => files.sort(),
                };

                files
            }
        };

        Ok(files)
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Order {
    Sequential,
    SingleSidedScans,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SortBy {
    Discovered,
    Lexicographical,
    // TODO: add natural sort; probably use <https://docs.rs/lexical-sort/>
}

impl Default for SortBy {
    fn default() -> Self {
        Self::Lexicographical
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("failed to parse image pattern"))]
pub struct CliGlobParseError {
    source: wax::BuildError<'static>,
}
