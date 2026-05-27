// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

const SENTRY_DSN: &str = "https://1ef441fdd1202426505007899c0726c2@o4511455386599424.ingest.us.sentry.io/4511455416156160";

fn main() {
    let client = sentry::init((
        SENTRY_DSN,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            traces_sample_rate: 0.0,
            ..Default::default()
        },
    ));

    if client.is_enabled() {
        findr_desktop_lib::run_with_sentry(client);
    } else {
        findr_desktop_lib::run();
    }
}
