use crate::stats::GLOBAL_STATS;

pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!();
        eprintln!("================================");
        eprintln!("BitLog Panic Captured!");
        eprintln!("================================");

        if let Some(location) = panic_info.location() {
            eprintln!(
                "panic occurred in file '{}' at line {}",
                location.file(),
                location.line(),
            );
        }

        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("panic payload: {}", s);
        }

        GLOBAL_STATS.print_report();

        eprintln!("================================");
    }));
}
