fn main() {
    std::process::exit(workon::run_cli(std::env::args().skip(1)));
}
