// @generated automatically by Diesel CLI.

diesel::table! {
    apps (id) {
        id -> Nullable<Integer>,
        name -> Text,
    }
}

diesel::table! {
    auth_tokens (id) {
        id -> Text,
        app_id -> Integer,
        name -> Text,
    }
}

diesel::joinable!(auth_tokens -> apps (app_id));

diesel::allow_tables_to_appear_in_same_query!(apps, auth_tokens,);
