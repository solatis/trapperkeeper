use trapperkeeper::database;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

#[test]
fn can_establish_connection() {
    database::establish_connection().unwrap();
}
