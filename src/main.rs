use std::env;

fn main() {
    let mut args = env::args().skip(1);
    if args.any(|arg| arg == "--version") {
        println!("our-tool {}", env!("CARGO_PKG_VERSION"));
    } else {
        println!("hello from our-tool");
    }
}
