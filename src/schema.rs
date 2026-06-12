// @generated automatically by Diesel CLI.

diesel::table! {
    bets (id) {
        id -> Uuid,
        job_id -> Nullable<Uuid>,
        market_id -> Text,
        question -> Text,
        side -> Text,
        size_usdc -> Float8,
        price -> Float8,
        paper -> Bool,
        confidence -> Float8,
        yes_price -> Float8,
        no_price -> Float8,
        volume_24h -> Float8,
        attestation_hash -> Nullable<Text>,
        tx_hash -> Nullable<Text>,
        outcome -> Nullable<Bool>,
        pnl_usdc -> Nullable<Float8>,
        placed_at -> Timestamptz,
        resolved_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    jobs (id) {
        id -> Uuid,
        model_id -> Nullable<Text>,
        status -> Nullable<Text>,
    }
}

diesel::table! {
    market_snapshots (id) {
        id -> Uuid,
        market_id -> Text,
        question -> Text,
        yes_price -> Float8,
        no_price -> Float8,
        volume_24h -> Float8,
        end_date -> Timestamptz,
        captured_at -> Timestamptz,
    }
}

diesel::table! {
    models (id) {
        id -> Text,
        name -> Nullable<Text>,
    }
}

diesel::table! {
    outcomes (id) {
        id -> Uuid,
        market_id -> Text,
        question -> Text,
        resolved_at -> Timestamptz,
        outcome -> Bool,
    }
}

diesel::table! {
    training_samples (id) {
        id -> Uuid,
        snapshot_id -> Uuid,
        outcome_id -> Uuid,
        market_id -> Text,
        yes_price -> Float8,
        no_price -> Float8,
        volume_24h -> Float8,
        time_to_expiry -> Float8,
        outcome -> Bool,
        created_at -> Timestamptz,
    }
}

diesel::joinable!(bets -> jobs (job_id));
diesel::joinable!(training_samples -> market_snapshots (snapshot_id));
diesel::joinable!(training_samples -> outcomes (outcome_id));

diesel::allow_tables_to_appear_in_same_query!(
    bets,
    jobs,
    market_snapshots,
    models,
    outcomes,
    training_samples,
);
