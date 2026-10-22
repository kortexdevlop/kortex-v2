#[cfg(feature = "desktop")]
fn main() -> std::process::ExitCode {
    kortex_player::desktop::main()
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn main() {
    kortex_player::web::main();
}

#[cfg(not(any(feature = "desktop", feature = "web")))]
fn main() {}
