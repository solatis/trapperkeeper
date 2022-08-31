use trapperkeeper::database;

#[test]
fn can_establish_connection() {
    database::establish_connection();
}
