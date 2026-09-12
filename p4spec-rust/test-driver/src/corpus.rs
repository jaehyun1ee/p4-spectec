use crate::{Error, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    Exclude,
}

pub fn parse_expected(text: &str) -> Result<BTreeMap<PathBuf, Outcome>> {
    let mut records = BTreeMap::new();
    for (idx, line) in text.lines().enumerate() {
        let (status, path) = line
            .split_once('\t')
            .ok_or_else(|| Error::Invalid(format!("expected line {}: missing tab", idx + 1)))?;
        let outcome = match status {
            "pass" => Outcome::Pass,
            "fail" => Outcome::Fail,
            "exclude" => Outcome::Exclude,
            _ => {
                return Err(Error::Invalid(format!(
                    "expected line {}: invalid status {status}",
                    idx + 1
                )));
            }
        };
        if !path.ends_with(".p4")
            || path.contains(['\t', '\\'])
            || path.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(Error::Invalid(format!(
                "expected line {}: invalid relative path {path:?}",
                idx + 1
            )));
        }
        if records.insert(PathBuf::from(path), outcome).is_some() {
            return Err(Error::Invalid(format!("duplicate expected path: {path}")));
        }
    }
    if records.is_empty() {
        return Err(Error::Invalid("empty expected file".to_owned()));
    }
    Ok(records)
}

pub fn collect(dir: &Path, suffix: &str) -> Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    let mut files = Vec::new();
    for path in paths {
        if fs::metadata(&path)?.is_dir() {
            if path.file_name().is_some_and(|name| name != "include") {
                files.extend(collect(&path, suffix)?);
            }
        } else if path.to_str().is_some_and(|path| path.ends_with(suffix)) {
            files.push(path);
        }
    }
    Ok(files)
}

pub fn collect_excludes(dir: &Path) -> Result<BTreeSet<String>> {
    let mut excludes = BTreeSet::new();
    for path in collect(dir, ".exclude")? {
        let text = fs::read_to_string(path)?;
        // OCaml input_line strips only LF; do not trim whitespace or CR
        excludes.extend(
            text.split_terminator('\n')
                .filter(|line| !line.starts_with('#'))
                .map(str::to_owned),
        );
    }
    Ok(excludes)
}

pub fn validate_inventory(
    expected: &BTreeMap<PathBuf, Outcome>,
    paths: &[PathBuf],
    excludes: &BTreeSet<String>,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for path in paths {
        if !seen.insert(path) {
            return Err(Error::Invalid(format!(
                "duplicate input: {}",
                path.display()
            )));
        }
        let outcome = expected
            .get(path)
            .ok_or_else(|| Error::Invalid(format!("unexpected input: {}", path.display())))?;
        let excluded = excludes.contains(
            path.to_str()
                .ok_or_else(|| Error::Invalid("non-UTF-8 corpus path".to_owned()))?,
        );
        if (*outcome == Outcome::Exclude) != excluded {
            return Err(Error::Invalid(format!(
                "exclusion mismatch: {}",
                path.display()
            )));
        }
    }
    let missing: Vec<_> = expected
        .keys()
        .filter(|path| !seen.contains(path))
        .collect();
    if !missing.is_empty() {
        return Err(Error::Invalid(format!("missing inputs: {missing:?}")));
    }
    Ok(())
}

pub struct Results<'a> {
    expected: &'a BTreeMap<PathBuf, Outcome>,
    seen: BTreeSet<PathBuf>,
    pub matched: usize,
    pub mismatched: usize,
}

impl<'a> Results<'a> {
    pub fn new(expected: &'a BTreeMap<PathBuf, Outcome>) -> Self {
        Self {
            expected,
            seen: BTreeSet::new(),
            matched: 0,
            mismatched: 0,
        }
    }
    pub fn record(&mut self, path: &Path, outcome: Outcome) -> Result<bool> {
        let expected = self
            .expected
            .get(path)
            .ok_or_else(|| Error::Invalid(format!("unexpected result: {}", path.display())))?;
        if !self.seen.insert(path.to_owned()) {
            return Err(Error::Invalid(format!(
                "duplicate result: {}",
                path.display()
            )));
        }
        let matched = *expected == outcome;
        if matched {
            self.matched += 1;
        } else {
            self.mismatched += 1;
        }
        Ok(matched)
    }
    pub fn finish(&self) -> Result<()> {
        let missing: Vec<_> = self
            .expected
            .keys()
            .filter(|path| !self.seen.contains(*path))
            .collect();
        if !missing.is_empty() || self.mismatched != 0 {
            return Err(Error::Invalid(format!(
                "{} mismatches; missing results: {missing:?}",
                self.mismatched
            )));
        }
        Ok(())
    }
}
