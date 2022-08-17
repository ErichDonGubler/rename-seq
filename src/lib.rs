use arrayvec::ArrayVec;
use snafu::Snafu;
use std::{
    fmt,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

/// A limited specification of replacement.
///
/// Currently, only a single replacement group (`{}`)
#[derive(Clone, Debug)]
pub struct RenameSpec<'a> {
    delimited: ArrayVec<(&'a str, DynamicRenameContent), 1>,
    suffix: &'a str,
}

impl<'a> RenameSpec<'a> {
    pub fn new(s: &'a str) -> Result<Self, RenameSpecParseError> {
        let mut delimited = ArrayVec::new();
        let mut remaining = s;

        if let Some(idx) = s.find('{') {
            let before = &s[..idx];

            let after_brace_idx = idx + '{'.len_utf8();
            remaining = &s[after_brace_idx..];

            if let Some(after) = remaining.strip_prefix("padded_idx}") {
                delimited.push((before, DynamicRenameContent::PaddedInteger));
                remaining = after;
            } else {
                return Err(RenameSpecParseError {
                    idx: after_brace_idx,
                    source: RenameSpecParseErrorKind::UnexpectedAfterOpenCurlyBrace,
                });
            }
        }

        let suffix = remaining;

        Ok(Self { delimited, suffix })
    }

    pub fn has_dynamic_content(&self) -> bool {
        !self.delimited.is_empty()
    }

    fn write(&self, ctx: &RenameContext, mut f: impl fmt::Write) -> fmt::Result {
        let Self { delimited, suffix } = self;

        for (prefix, dyn_content) in delimited.iter() {
            write!(f, "{prefix}")?;
            match dyn_content {
                DynamicRenameContent::PaddedInteger => {
                    let RenameContext {
                        idx,
                        max_size_hint_digits,
                    } = ctx;
                    write!(f, "{idx:0padding$}", padding = max_size_hint_digits)?;
                }
            }
        }
        f.write_str(suffix)
    }
}

#[derive(Clone, Debug)]
pub enum DynamicRenameContent {
    PaddedInteger,
}

#[derive(Debug, Snafu)]
#[snafu(display("failed to parse rename spec beyond index {idx}"))]
pub struct RenameSpecParseError {
    idx: usize,
    source: RenameSpecParseErrorKind,
}

#[derive(Debug, Snafu)]
enum RenameSpecParseErrorKind {
    #[snafu(display(
        "expected replacement group `{{padded_idx}}`, but content after opening `{{` does not match"
    ))]
    UnexpectedAfterOpenCurlyBrace,
}

struct RenameContext {
    idx: usize,
    max_size_hint_digits: usize,
}

pub fn zip_single_side_scans<'a, V>(
    files: impl Iterator<Item = &'a Path>,
    rename_spec: RenameSpec,
    mut visitor: V,
) -> Result<(), V::Error>
where
    V: Visitor,
{
    let max_size_hint_digits = {
        let max_hinted_size = {
            let (min, max) = files.size_hint();
            max.unwrap_or(min)
        };
        match max_hinted_size {
            0 => 1,
            n => (f64::try_from(u32::try_from(n).unwrap()).unwrap().log10() as usize)
                .checked_add(1)
                .unwrap(),
        }
    };
    for (idx, from) in files.enumerate() {
        let to = {
            let mut to = String::new();
            rename_spec
                .write(
                    &RenameContext {
                        idx,
                        max_size_hint_digits,
                    },
                    &mut to,
                )
                .unwrap();
            PathBuf::from(to)
        };

        match visitor.visit(idx, from, to) {
            ControlFlow::Continue(()) => (),
            ControlFlow::Break(e) => return Err(e),
        }
    }
    Ok(())
}

pub trait Visitor {
    type Error;

    fn visit(&mut self, idx: usize, from: &Path, to: PathBuf) -> ControlFlow<Self::Error>;
}

#[test]
fn no_replacement() {
    todo!()
}

#[test]
fn correct_padding() {
    todo!()
}
