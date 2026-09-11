#[path = "intern/canon.rs"]
mod canon;
#[path = "intern/rc.rs"]
mod rc;
#[path = "intern/simple.rs"]
mod simple;

use std::hash::{Hash, Hasher};

#[derive(Debug, Eq, PartialEq)]
struct Collision(u32);

impl Hash for Collision {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        0_u32.hash(hasher);
    }
}
