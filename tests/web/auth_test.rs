use trapperkeeper::database;
use trapperkeeper::web;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

#[test]
fn can_build_pool_from_env() {
    database::PoolBuilder::new().build();
}
