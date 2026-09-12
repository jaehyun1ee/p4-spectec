use crate::{Result, snapshot};

pub fn run() -> Result<()> {
    snapshot::run(true)
}
