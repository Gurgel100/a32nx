macro_rules! circuit {
    ($($({ext})? $src:ident $(($src_busnum:literal))?: $($({ext})? $dest:ident $(($dest_busnum:literal))?),+;)+) => {};
}

pub struct Circuit {}

#[cfg(test)]
mod circuit_tests {
    use super::*;

    circuit!(Bus1: [Bus2, Bus3], {ext} Bus4: [Bus5]);
}
