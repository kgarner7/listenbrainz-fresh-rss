// @generated automatically by Diesel CLI.

diesel::table! {
    releases (id) {
        id -> Text,
        has_front -> Bool,
        urls -> Text,
    }
}
