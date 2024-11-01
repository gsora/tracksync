fn main() {
    println!("cargo:rerun-if-changed=database/migrations/local");
    println!("cargo:rerun-if-changed=database/migrations/destination");
}
