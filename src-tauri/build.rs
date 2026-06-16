fn main() {
    // Skip Tauri build pipeline for unit-test / check-only invocations.
    // The embedded `tauri-winres` build helper spawns `rustc -V` via
    // `std::process::Command`; on some Windows sandboxes this fails with
    // `Os { code: 0, kind: Uncategorized, message: "操作成功完成。" }`,
    // which the upstream `rustc_version` crate unwraps and panics on.
    // Skipping the Tauri build (which is only needed for the final app
    // binary, not the lib's unit tests) lets `cargo test --lib` work
    // without code changes. The app build path remains unaffected
    // because that command is not used here.
    let skip_tauri_build = std::env::var("PAPER_VAULT_SKIP_TAURI_BUILD").is_ok();
    if skip_tauri_build {
        println!("cargo:warning=PAPER_VAULT_SKIP_TAURI_BUILD set; skipping tauri_build::build()");
        return;
    }
    tauri_build::build()
}
