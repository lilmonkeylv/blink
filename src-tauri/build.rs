const BAKED: [&str; 2] = ["BLINK_GITHUB_CLIENT_ID", "BLINK_GITHUB_CLIENT_SECRET"];

fn bake_oauth_credentials() {
    let dotenv = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.env");
    println!("cargo:rerun-if-changed={}", dotenv.display());
    let file = std::fs::read_to_string(&dotenv).unwrap_or_default();
    for name in BAKED {
        println!("cargo:rerun-if-env-changed={name}");
        let from_env = std::env::var(name).ok().filter(|v| !v.is_empty());
        let from_file = file
            .lines()
            .filter_map(|line| line.trim().split_once('='))
            .find(|(key, _)| key.trim() == name)
            .map(|(_, value)| value.trim().trim_matches('"').to_string())
            .filter(|v| !v.is_empty());
        match from_env.or(from_file) {
            Some(value) => println!("cargo:rustc-env={name}={value}"),
            None if std::env::var("PROFILE").as_deref() == Ok("release") => {
                println!("cargo:warning={name} is not set, GitHub sign-in will be disabled in this build")
            }
            None => {}
        }
    }
}

fn main() {
    bake_oauth_credentials();
    tauri_build::build()
}
