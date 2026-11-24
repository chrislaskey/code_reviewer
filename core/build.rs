// The `migrate!` macro reads the migration files at compile time, and a new
// file alone would not make Cargo compile again, so this tells it to.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
