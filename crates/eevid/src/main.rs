fn main() {
    if let Err(err) = eevid::main_entry(std::env::args_os()) {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}
