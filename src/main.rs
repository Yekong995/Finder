// Build handler for user didn't choose any features situation
#[cfg(not(any(feature = "cli", feature = "ui")))]
compile_error!("❌ You must enable at least one feature, such as --features cli or --features ui");

// Handle the case where both features are enabled
#[cfg(all(feature = "cli", feature = "ui"))]
fn main() {
    compile_error!("Both CLI and UI features enabled");
}

#[cfg(all(feature = "cli", not(feature = "ui")))]
mod cli_main;
#[cfg(all(feature = "cli", not(feature = "ui")))]
fn main() {
    let _ = cli_main::main();
}

#[cfg(all(feature = "ui", not(feature = "cli")))]
mod ui_main;
#[cfg(all(feature = "ui", not(feature = "cli")))]
fn main() {
    let _ = ui_main::main();
}
