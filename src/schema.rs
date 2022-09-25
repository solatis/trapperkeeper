// @generated automatically by Diesel CLI.

diesel::table! {
    auth_tokens (id) {
        id -> Text,
        trapp_id -> Integer,
        name -> Text,
    }
}

diesel::table! {
    trapps (id) {
        id -> Nullable<Integer>,
        name -> Text,
    }
}

diesel::joinable!(auth_tokens -> trapps (trapp_id));

diesel::allow_tables_to_appear_in_same_query!(
    auth_tokens,
    trapps,
);
